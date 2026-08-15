use std::io;
use std::ops::{Deref, DerefMut};
use std::process::{Child, Command};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnixProcessGroup {
    Inherit,
    New,
}

/// A child whose Windows process tree is owned by a kill-on-close Job Object.
///
/// Unix callers choose whether to preserve the inherited process group or start a new one,
/// matching the pre-existing behavior of each backend.
#[derive(Debug)]
pub(crate) struct ContainedChild {
    child: Child,
    #[cfg(unix)]
    unix_process_group: UnixProcessGroup,
    #[cfg(windows)]
    job: Option<windows_job::Job>,
}

impl ContainedChild {
    pub(crate) fn spawn(
        command: &mut Command,
        unix_process_group: UnixProcessGroup,
    ) -> io::Result<Self> {
        #[cfg(unix)]
        if unix_process_group == UnixProcessGroup::New {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }

        #[cfg(windows)]
        let job = windows_job::Job::new_kill_on_close()?;

        #[cfg(not(unix))]
        let _ = unix_process_group;

        let child = command.spawn()?;

        #[cfg(windows)]
        if let Err(error) = job.assign(&child) {
            let mut failed_child = child;
            let _ = failed_child.kill();
            let _ = failed_child.wait();
            return Err(error);
        }

        Ok(Self {
            child,
            #[cfg(unix)]
            unix_process_group,
            #[cfg(windows)]
            job: Some(job),
        })
    }

    pub(crate) fn terminate_tree(&mut self) {
        self.close_descendants();

        #[cfg(unix)]
        if self.unix_process_group == UnixProcessGroup::New
            && let Ok(process_group) = i32::try_from(self.child.id())
        {
            unix_process_group::kill(process_group);
        }

        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// Releases process-tree containment after the direct child exits, killing descendants on
    /// Windows before inherited pipes are joined.
    pub(crate) fn close_descendants(&mut self) {
        #[cfg(windows)]
        {
            self.job.take();
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

impl Drop for ContainedChild {
    fn drop(&mut self) {
        self.terminate_tree();
    }
}

#[cfg(all(test, windows))]
mod tests {
    use std::process::Command;

    use super::{ContainedChild, UnixProcessGroup};

    #[test]
    fn spawned_child_is_assigned_before_it_is_returned() {
        let mut command = Command::new("cmd.exe");
        command.args(["/D", "/C", "exit", "0"]);
        let mut child = ContainedChild::spawn(&mut command, UnixProcessGroup::Inherit)
            .expect("child should be assigned to a kill-on-close job");
        assert!(child.wait().expect("child should exit").success());
        child.close_descendants();
    }
}

#[cfg(unix)]
mod unix_process_group {
    pub(super) fn kill(process_group: i32) {
        use nix::sys::signal::{Signal, killpg};
        use nix::unistd::Pid;

        let _ = killpg(Pid::from_raw(process_group), Signal::SIGKILL);
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
            // SAFETY: Null security attributes and name request an unnamed job with default
            // security. The returned handle is checked before being wrapped for RAII cleanup.
            let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            let handle = NonNull::new(handle).ok_or_else(io::Error::last_os_error)?;
            let job = Self(handle);

            let mut information = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            information.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let information_size = u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
                .expect("Windows job information size fits in u32");
            // SAFETY: `job` owns a valid job handle. `information` has the exact structure and
            // byte length required by JobObjectExtendedLimitInformation and lives for the call.
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
            // SAFETY: Both handles are live for this call. The child handle is borrowed from
            // `Child`, and the job handle remains owned by `self`.
            if unsafe { AssignProcessToJobObject(self.0.as_ptr(), process) } == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: `self.0` is a uniquely owned, non-null job handle closed exactly once.
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
