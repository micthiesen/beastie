use std::io;
use std::ops::{Deref, DerefMut};
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

const TERMINATION_GRACE: Duration = Duration::from_millis(500);

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

#[cfg(all(test, windows))]
mod tests {
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
