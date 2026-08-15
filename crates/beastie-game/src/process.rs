use std::ffi::OsString;
use std::io::{self, BufRead, BufReader, Write};
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;
use std::time::{Duration, Instant};

const TERMINATION_GRACE: Duration = Duration::from_millis(500);
const CANCELLATION_POLL: Duration = Duration::from_millis(10);

/// A direct child plus Windows Job Object ownership for its complete descendant tree.
#[derive(Debug)]
pub(crate) struct ContainedChild {
    child: Child,
    #[cfg(windows)]
    job: Option<windows_job::Job>,
}

impl ContainedChild {
    pub(crate) fn spawn(command: &mut Command) -> io::Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }

        #[cfg(windows)]
        let job = windows_job::Job::new_kill_on_close()?;

        let child = command.spawn()?;

        #[cfg(windows)]
        if let Err(error) = job.assign(&child) {
            let mut failed_child = child;
            bounded_kill_and_reap(&mut failed_child);
            return Err(error);
        }

        Ok(Self {
            child,
            #[cfg(windows)]
            job: Some(job),
        })
    }

    pub(crate) fn terminate_tree(&mut self) {
        #[cfg(windows)]
        {
            // Kill-on-close terminates the child and all processes it created.
            self.job.take();
        }

        #[cfg(unix)]
        if let Ok(process_group) = i32::try_from(self.child.id()) {
            use nix::sys::signal::{Signal, killpg};
            use nix::unistd::Pid;

            let _ = killpg(Pid::from_raw(process_group), Signal::SIGKILL);
        }

        bounded_kill_and_reap(&mut self.child);
    }
}

fn bounded_kill_and_reap(child: &mut Child) {
    let _ = child.kill();
    let deadline = Instant::now() + TERMINATION_GRACE;
    loop {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => return,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            Ok(None) => return,
        }
    }
}

impl Deref for ContainedChild {
    type Target = Child;

    fn deref(&self) -> &Self::Target {
        &self.child
    }
}

impl DerefMut for ContainedChild {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.child
    }
}

/// Persistent, bounded JSONL transport for local worker processes.
///
/// It owns only process lifetime and one-request/one-line reply exchange. Callers retain their
/// request/reply types, validation, fallback, correlation, and recovery policy.
pub(crate) struct JsonlWorkerSession {
    child: ContainedChild,
    stdin: ChildStdin,
    lines: Receiver<io::Result<String>>,
    exchanged: bool,
}

impl JsonlWorkerSession {
    pub(crate) fn spawn(
        executable: &Path,
        arguments: &[OsString],
        maximum_reply_bytes: usize,
    ) -> io::Result<Self> {
        let mut command = Command::new(executable);
        command
            .args(arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = ContainedChild::spawn(&mut command)?;
        let Some(stdin) = child.stdin.take() else {
            child.terminate_tree();
            return Err(io::Error::other("JSONL worker stdin unavailable"));
        };
        let Some(stdout) = child.stdout.take() else {
            child.terminate_tree();
            return Err(io::Error::other("JSONL worker stdout unavailable"));
        };
        let (sender, lines) = mpsc::sync_channel(1);
        thread::spawn(move || {
            read_jsonl_lines(BufReader::new(stdout), sender, maximum_reply_bytes);
        });
        Ok(Self {
            child,
            stdin,
            lines,
            exchanged: false,
        })
    }

    pub(crate) fn exchange<Request: serde::Serialize>(
        &mut self,
        request: &Request,
        timeout: Duration,
        cancelled: &AtomicBool,
    ) -> io::Result<String> {
        if self.exchanged {
            match self.lines.try_recv() {
                Ok(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "JSONL worker emitted unsolicited output",
                    ));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "JSONL worker reply reader disconnected",
                    ));
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        serde_json::to_writer(&mut self.stdin, request)?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        self.exchanged = true;
        let started = Instant::now();
        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "JSONL worker transport cancelled",
                ));
            }
            let remaining = timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "JSONL worker reply timed out",
                ));
            }
            match self.lines.recv_timeout(remaining.min(CANCELLATION_POLL)) {
                Ok(line) => return line,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(error @ mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(io::Error::new(io::ErrorKind::BrokenPipe, error));
                }
            }
        }
    }

    pub(crate) fn terminate(&mut self) {
        self.child.terminate_tree();
    }
}

fn read_jsonl_lines(
    mut reader: impl BufRead,
    sender: SyncSender<io::Result<String>>,
    maximum_reply_bytes: usize,
) {
    loop {
        let line = read_bounded_jsonl_line(&mut reader, maximum_reply_bytes);
        let finished = matches!(&line, Ok(None) | Err(_));
        let result = line.and_then(|line| {
            line.ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "JSONL worker exited"))
        });
        if sender.send(result).is_err() || finished {
            break;
        }
    }
}

pub(crate) fn read_bounded_jsonl_line(
    reader: &mut impl BufRead,
    maximum: usize,
) -> io::Result<Option<String>> {
    let mut bytes = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return if bytes.is_empty() {
                Ok(None)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "unterminated JSONL worker reply",
                ))
            };
        }
        let consumed = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |index| index + 1);
        if bytes.len().saturating_add(consumed) > maximum {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "JSONL worker reply exceeds limit",
            ));
        }
        bytes.extend_from_slice(&available[..consumed]);
        reader.consume(consumed);
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
            if bytes.last() == Some(&b'\r') {
                bytes.pop();
            }
            return String::from_utf8(bytes)
                .map(Some)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error));
        }
    }
}

#[cfg(test)]
mod jsonl_tests {
    use std::io::Cursor;

    use super::read_bounded_jsonl_line;

    #[test]
    fn bounded_jsonl_reader_accepts_crlf_and_rejects_unterminated_or_oversized_lines() {
        let mut valid = Cursor::new(b"ok\r\nnext\n".to_vec());
        assert_eq!(
            read_bounded_jsonl_line(&mut valid, 8).unwrap().as_deref(),
            Some("ok")
        );
        assert_eq!(
            read_bounded_jsonl_line(&mut valid, 8).unwrap().as_deref(),
            Some("next")
        );
        let mut oversized = Cursor::new(b"too-long\n".to_vec());
        assert!(read_bounded_jsonl_line(&mut oversized, 8).is_err());
        let mut unterminated = Cursor::new(b"partial".to_vec());
        assert!(read_bounded_jsonl_line(&mut unterminated, 8).is_err());
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use std::process::Command;

    use super::ContainedChild;

    #[test]
    fn spawned_child_is_assigned_before_it_is_returned() {
        let mut command = Command::new("cmd.exe");
        command.args(["/D", "/C", "exit", "0"]);
        let mut child = ContainedChild::spawn(&mut command)
            .expect("child should be assigned to a kill-on-close job");
        assert!(child.wait().expect("child should exit").success());
    }
}

#[cfg(windows)]
mod windows_job {
    use std::io;
    use std::mem::size_of;
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;
    use std::ptr::NonNull;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };

    #[derive(Debug)]
    pub(super) struct Job(NonNull<std::ffi::c_void>);

    impl Job {
        pub(super) fn new_kill_on_close() -> io::Result<Self> {
            // SAFETY: Null attributes/name request an unnamed job with default security. The
            // returned handle is checked before RAII ownership begins.
            let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            let handle = NonNull::new(handle).ok_or_else(io::Error::last_os_error)?;
            let job = Self(handle);

            let mut information = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            information.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let information_size = u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
                .expect("Windows job information size fits in u32");
            // SAFETY: The job handle is valid and the structure pointer/length exactly match the
            // requested JobObjectExtendedLimitInformation class for the duration of this call.
            let configured = unsafe {
                SetInformationJobObject(
                    job.0.as_ptr(),
                    JobObjectExtendedLimitInformation,
                    std::ptr::from_ref(&information).cast(),
                    information_size,
                )
            };
            if configured == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(job)
        }

        pub(super) fn assign(&self, child: &Child) -> io::Result<()> {
            let process: HANDLE = child.as_raw_handle();
            // SAFETY: Both borrowed handles are live for the duration of the assignment call.
            if unsafe { AssignProcessToJobObject(self.0.as_ptr(), process) } == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: This is the unique owner of a non-null job handle, closed exactly once.
            unsafe {
                CloseHandle(self.0.as_ptr());
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn kill_on_close_job_can_be_created() {
            Job::new_kill_on_close().expect("kill-on-close job should be available");
        }
    }
}
