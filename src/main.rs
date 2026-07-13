use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use std::thread;
use std::os::unix::process::ExitStatusExt;

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
}

struct RunConfig {
    timeout_secs: u64,
}

fn run_command(command: &str, args: &[&str], timeout_secs: u64) -> RunResult {
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
            };
        }
    };

    let timeout = Duration::from_secs(timeout_secs);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child
                    .wait_with_output()
                    .expect("Process ended, but output could not be collected");

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
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                };
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();

                    let output = child
                        .wait_with_output()
                        .expect("Timed out process, but output could not be collected");

                    return RunResult {
                        command: command.to_string(),
                        args: args.iter().map(|s| s.to_string()).collect(),
                        status: RunStatus::TimedOut,
                        exit_code: None,
                        signal: None,
                        duration: start.elapsed().as_secs_f64(),
                        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
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

    println!("--- stderr ---");
    if result.stderr.is_empty() {
        println!("(empty)");
    } else {
        println!("{}", result.stderr);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: scry [--timeout SECONDS] <command> [arguments...]");
        return;
    }

    let mut timeout_secs = 3;
    let command_index;

    if args[1] == "--timeout" {
        if args.len() < 4 {
            eprintln!("Usage: scry --timeout SECONDS <command> [arguments...]");
            return;
        }

        timeout_secs = args[2].parse().unwrap();
        command_index = 3;
    } else {
        command_index = 1;
    }

    let command = &args[command_index];

    let command_args: Vec<&str> = args[command_index + 1..]
        .iter()
        .map(|arg| arg.as_str())
        .collect();

    let config = RunConfig { timeout_secs };

    let result = run_command(
        command,
        &command_args,
        config.timeout_secs,
    );

    print_result(&result);
}