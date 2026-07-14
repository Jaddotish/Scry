use std::io::Read;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug)]
enum RunStatus {
    Succeeded,
    Failed,
    TimedOut,
    FailedToStart,
    Signaled,
}

#[derive(Debug)]
struct RunResult {
    command: String,
    args: Vec<String>,
    status: RunStatus,
    exit_code: Option<i32>,
    signal: Option<i32>,
    duration: f64,
    stdout: String,
    stderr: String,
    stdout_truncated: bool,
    stderr_truncated: bool,
}

struct RunConfig {
    timeout_secs: u64,
    max_output_bytes: usize,
}

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

fn run_command(
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
                    let _ = child.kill();

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

fn print_result(result: &RunResult) {
    println!("=== RUN RESULT ===");
    println!("command: {}", result.command);
    println!("args: {:?}", result.args);

    match result.status {
        RunStatus::Succeeded => println!("status: succeeded"),
        RunStatus::Failed => println!("status: failed"),
        RunStatus::TimedOut => println!("status: timed out"),
        RunStatus::FailedToStart => println!("status: failed to start"),
        RunStatus::Signaled => println!("status: terminated by signal"),
    }

    match result.exit_code {
        Some(code) => println!("exit code: {}", code),
        None => println!("exit code: None"),
    }

    match result.signal {
        Some(sig) => println!("signal: {}", sig),
        None => println!("signal: None"),
    }

    println!("duration: {:.4} seconds", result.duration);

    println!("--- stdout ---");
    if result.stdout.is_empty() {
        println!("(empty)");
    } else {
        println!("{}", result.stdout);
    }

    if result.stdout_truncated {
        println!("[stdout truncated]");
    }

    println!("--- stderr ---");
    if result.stderr.is_empty() {
        println!("(empty)");
    } else {
        println!("{}", result.stderr);
    }

    if result.stderr_truncated {
        println!("[stderr truncated]");
    }
}

fn print_usage() {
    eprintln!("Usage: scry [--timeout SECONDS] <command> [arguments...]");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Error: missing command.");
        print_usage();
        return;
    }

    let mut timeout_secs = 3;
    let command_index;

    if args[1] == "--timeout" {
        if args.len() < 3 {
            eprintln!("Error: --timeout requires a number.");
            print_usage();
            return;
        }

        timeout_secs = match args[2].parse::<u64>() {
            Ok(value) => value,
            Err(_) => {
                eprintln!(
                    "Error: timeout must be a nonnegative integer, not {:?}.",
                    args[2]
                );
                print_usage();
                return;
            }
        };

        if args.len() < 4 {
            eprintln!("Error: missing command after timeout value.");
            print_usage();
            return;
        }

        command_index = 3;
    } else {
        command_index = 1;
    }

    let command = &args[command_index];

    let command_args: Vec<&str> = args[command_index + 1..]
        .iter()
        .map(|arg| arg.as_str())
        .collect();

    let config = RunConfig {
        timeout_secs,
        max_output_bytes: 1_000_000,
    };

    let result = run_command(
        command,
        &command_args,
        config.timeout_secs,
        config.max_output_bytes,
    );

    print_result(&result);
}