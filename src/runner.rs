use std::io::Read;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::result::{RunResult, RunStatus};

fn read_with_limit<R: Read>(
    mut reader: R,
    max_bytes: usize,
) -> (Vec<u8>, bool) {
    let mut captured = Vec::new();
    let mut chunk = [0u8; 8192];
    let mut truncated = false;

    loop {
        let bytes_read = reader
            .read(&mut chunk)
            .expect("Could not read child output");

        if bytes_read == 0 {
            break;
        }

        let remaining = max_bytes.saturating_sub(captured.len());
        let bytes_to_store = remaining.min(bytes_read);

        captured.extend_from_slice(&chunk[..bytes_to_store]);

        if bytes_to_store < bytes_read {
            truncated = true;
        }
    }

    (captured, truncated)
}

pub fn run_command(
    command: &str,
    args: &[&str],
    timeout_secs: u64,
    max_output_bytes: usize,
) -> RunResult {
    let start = Instant::now();

    let mut child = match Command::new(command)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            return RunResult {
                command: command.to_string(),
                args: args.iter().map(|s| s.to_string()).collect(),
                status: RunStatus::FailedToStart,
                exit_code: None,
                signal: None,
                duration: start.elapsed().as_secs_f64(),
                stdout: String::new(),
                stderr: format!("Failed to start process: {err}"),
                stdout_truncated: false,
                stderr_truncated: false,
            };
        }
    };

    let stdout = child
        .stdout
        .take()
        .expect("stdout should be piped");

    let stderr = child
        .stderr
        .take()
        .expect("stderr should be piped");

    let stdout_thread = thread::spawn(move || {
        read_with_limit(stdout, max_output_bytes)
    });

    let stderr_thread = thread::spawn(move || {
        read_with_limit(stderr, max_output_bytes)
    });

    let timeout = Duration::from_secs(timeout_secs);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let (stdout_bytes, stdout_truncated) = stdout_thread
                    .join()
                    .expect("stdout reader thread panicked");

                let (stderr_bytes, stderr_truncated) = stderr_thread
                    .join()
                    .expect("stderr reader thread panicked");

                let run_status = if status.signal().is_some() {
                    RunStatus::Signaled
                } else if status.success() {
                    RunStatus::Succeeded
                } else {
                    RunStatus::Failed
                };

                return RunResult {
                    command: command.to_string(),
                    args: args.iter().map(|s| s.to_string()).collect(),
                    status: run_status,
                    exit_code: status.code(),
                    signal: status.signal(),
                    duration: start.elapsed().as_secs_f64(),
                    stdout: String::from_utf8_lossy(&stdout_bytes).to_string(),
                    stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
                    stdout_truncated,
                    stderr_truncated,
                };
            }

            Ok(None) => {
                if start.elapsed() >= timeout {
                    let process_group_id = child.id() as i32;

                    unsafe {
                        libc::kill(-process_group_id, libc::SIGKILL);
                    }

                    child
                        .wait()
                        .expect("Timed out process could not be reaped");

                    let (stdout_bytes, stdout_truncated) = stdout_thread
                        .join()
                        .expect("stdout reader thread panicked");

                    let (stderr_bytes, stderr_truncated) = stderr_thread
                        .join()
                        .expect("stderr reader thread panicked");

                    return RunResult {
                        command: command.to_string(),
                        args: args.iter().map(|s| s.to_string()).collect(),
                        status: RunStatus::TimedOut,
                        exit_code: None,
                        signal: None,
                        duration: start.elapsed().as_secs_f64(),
                        stdout: String::from_utf8_lossy(&stdout_bytes).to_string(),
                        stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
                        stdout_truncated,
                        stderr_truncated,
                    };
                }

                thread::sleep(Duration::from_millis(10));
            }

            Err(err) => {
                return RunResult {
                    command: command.to_string(),
                    args: args.iter().map(|s| s.to_string()).collect(),
                    status: RunStatus::FailedToStart,
                    exit_code: None,
                    signal: None,
                    duration: start.elapsed().as_secs_f64(),
                    stdout: String::new(),
                    stderr: format!("Error while waiting on process: {err}"),
                    stdout_truncated: false,
                    stderr_truncated: false,
                };
            }
        }
    }
}