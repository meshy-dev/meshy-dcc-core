//! Run the just-updated executable in place of the current process.
//!
//! A staged update is applied before a binary does its work, so the
//! process that applied it must hand the same invocation to the new file.
//! The caller has to see that invocation's output and exit status: a
//! parent that returned early would report success with no output while
//! the new process carried on unsupervised, and a launcher waiting for a
//! gateway would see its process exit before it became healthy.

use std::ffi::OsString;
use std::process::Command;

/// Re-run this invocation from the executable now at `current_exe`.
///
/// On Unix the process image is replaced, so the PID, standard streams and
/// parent relationship stay the same. On Windows the new process inherits
/// the standard streams, is ended together with this one, and this process
/// exits with its status. Returns only if the new process could not start.
pub fn relaunch_current_exe(env: &[(&str, &str)]) -> std::io::Error {
    let executable = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => return error,
    };
    let mut command = Command::new(executable);
    command.args(std::env::args_os().skip(1).collect::<Vec<OsString>>());
    command.envs(env.iter().copied());
    run(command)
}

#[cfg(unix)]
fn run(mut command: Command) -> std::io::Error {
    use std::os::unix::process::CommandExt as _;
    command.exec()
}

#[cfg(windows)]
fn run(mut command: Command) -> std::io::Error {
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => return error,
    };
    // Best effort: without the job the new process still runs, it just
    // outlives this one if this one is killed.
    let _job = windows_job::end_with_this_process(&child);
    match child.wait() {
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(error) => error,
    }
}

#[cfg(windows)]
mod windows_job {
    use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle};
    use std::process::Child;

    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectExtendedLimitInformation, SetInformationJobObject,
    };

    /// Put `child` in a job that is closed, ending `child`, when this process
    /// ends. Processes `child` starts are not in the job (silent breakaway),
    /// so gateways and applications it launches keep running as before.
    pub(super) fn end_with_this_process(child: &Child) -> Option<OwnedHandle> {
        // SAFETY: plain Win32 calls; the job handle is owned below and the
        // child handle stays valid while `child` is alive.
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return None;
            }
            let job_handle = OwnedHandle::from_raw_handle(job as _);
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags =
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK;
            let configured = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ) != 0;
            let assigned =
                configured && AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE) != 0;
            assigned.then_some(job_handle)
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::process::Command;

    const CHILD_ENV: &str = "MESHY_DCC_RELAUNCH_TEST_CHILD";

    /// Runs only inside the process the next test starts: it hands itself
    /// over to a program whose output and exit status the test checks.
    #[test]
    fn relaunch_child() {
        if std::env::var_os(CHILD_ENV).is_none() {
            return;
        }
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "echo relaunched; exit 7"]);
        let error = super::run(command);
        panic!("relaunch returned: {error}");
    }

    /// The caller sees the relaunched invocation's output and exit status,
    /// not an early, empty success from the process that applied the update.
    #[test]
    fn caller_sees_relaunched_output_and_exit_status() {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "relaunch::tests::relaunch_child", "--nocapture"])
            .env(CHILD_ENV, "1")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("relaunched"), "{stdout}");
        assert_eq!(output.status.code(), Some(7));
    }

    #[test]
    fn a_missing_executable_is_reported() {
        let error = super::run(Command::new("/nonexistent/meshy-dcc-relaunch-test"));
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }
}
