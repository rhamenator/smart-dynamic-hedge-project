use std::io::Read;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::CoreError;

pub struct TimedOutput {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

/// `std::process` has no built-in timeout (unlike Python's
/// `subprocess.run(timeout=...)`), and reaching for a crate just for that
/// would violate this project's dependency-minimization policy. This
/// hand-rolls one: stdout/stderr are read concurrently on dedicated
/// threads (so a chatty child can't deadlock by filling a pipe buffer
/// while the main thread polls `try_wait`), and the child is killed if it
/// outlives `timeout`. Fully cross-platform via `std` alone —
/// `Child::kill()` already does the right thing on both Windows
/// (`TerminateProcess`) and Unix (`SIGKILL`).
pub fn run_command_with_timeout(command: &mut Command, timeout: Duration) -> Result<TimedOutput, CoreError> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let mut stdout_pipe = child.stdout.take().expect("stdout was piped");
    let mut stderr_pipe = child.stderr.take().expect("stderr was piped");

    let stdout_handle = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout_pipe.read_to_end(&mut buf);
        buf
    });
    let stderr_handle = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr_pipe.read_to_end(&mut buf);
        buf
    });

    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            // The reader threads exit once the (now-dead) child's pipes
            // close; join them so this function never leaks threads, but
            // their content is irrelevant once we're reporting a timeout.
            let _ = stdout_handle.join();
            let _ = stderr_handle.join();
            return Err(CoreError::Timeout(timeout));
        }
        thread::sleep(Duration::from_millis(10));
    };

    let stdout_bytes = stdout_handle.join().unwrap_or_default();
    let stderr_bytes = stderr_handle.join().unwrap_or_default();
    Ok(TimedOutput {
        status,
        stdout: String::from_utf8_lossy(&stdout_bytes).into_owned(),
        stderr: String::from_utf8_lossy(&stderr_bytes).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn echo_command(text: &str) -> Command {
        if cfg!(windows) {
            let mut c = Command::new("cmd");
            c.args(["/C", "echo", text]);
            c
        } else {
            let mut c = Command::new("echo");
            c.arg(text);
            c
        }
    }

    #[test]
    fn captures_stdout_of_a_fast_command() {
        let result = run_command_with_timeout(&mut echo_command("hello"), Duration::from_secs(5)).unwrap();
        assert!(result.status.success());
        assert!(result.stdout.contains("hello"));
    }

    #[test]
    fn times_out_a_command_that_sleeps_too_long() {
        let mut command = if cfg!(windows) {
            let mut c = Command::new("cmd");
            c.args(["/C", "ping", "-n", "20", "127.0.0.1", ">", "NUL"]);
            c
        } else {
            let mut c = Command::new("sleep");
            c.arg("20");
            c
        };
        let result = run_command_with_timeout(&mut command, Duration::from_millis(200));
        assert!(matches!(result, Err(CoreError::Timeout(_))));
    }
}
