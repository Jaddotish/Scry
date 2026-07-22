mod config;
mod result;
mod runner;

use config::RunConfig;
use result::{RunResult, RunStatus};
use runner::run_command;

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
        cpu_limit_secs: 2,
        max_output_bytes: 1_000_000,
        memory_limit_bytes: 256_000_000,
        file_size_limit_bytes: 10_000_000,
    };

    let result = run_command(
        command,
        &command_args,
        config.timeout_secs,
        config.cpu_limit_secs,
        config.max_output_bytes,
        config.memory_limit_bytes,
        config.file_size_limit_bytes,
    );

    print_result(&result);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_command_returns_succeeded() {
        let result = run_command(
            "python3",
            &["-c", "print('hello')"],
            2,
            5,
            1_000_000,
            1_000_000_000,
            10_000_000,
        );

        assert!(matches!(result.status, RunStatus::Succeeded));
        assert_eq!(result.exit_code, Some(0));
        assert!(result.stdout.contains("hello"));
        assert!(result.stderr.is_empty());
    }

    #[test]
    fn nonzero_exit_returns_failed() {
        let result = run_command(
            "python3",
            &["-c", "raise RuntimeError('test')"],
            2,
            5,
            1_000_000,
            1_000_000_000,
            10_000_000,
        );

        assert!(matches!(result.status, RunStatus::Failed));
        assert_eq!(result.exit_code, Some(1));
        assert!(result.stdout.is_empty());
        assert!(result.stderr.contains("RuntimeError"));
    }

    #[test]
    fn long_running_command_times_out() {
        let result = run_command(
            "python3",
            &["-c", "import time; time.sleep(10)"],
            5,
            5,
            1_000_000,
            1_000_000_000,
            10_000_000,
        );

        assert!(matches!(result.status, RunStatus::TimedOut));
        assert_eq!(result.exit_code, None);
    }

    #[test]
    fn stdout_and_stderr_are_separate() {
        let result = run_command(
            "python3",
            &[
                "-c",
                "import sys; print('normal'); print('error', file=sys.stderr)",
            ],
            2,
            5,
            1_000_000,
            1_000_000_000,
            10_000_000,
        );

        assert!(matches!(result.status, RunStatus::Succeeded));
        assert!(result.stdout.contains("normal"));
        assert!(!result.stdout.contains("error"));
        assert!(result.stderr.contains("error"));
        assert!(!result.stderr.contains("normal"));
    }

    #[test]
    fn invalid_executable_returns_failed_to_start() {
        let result = run_command(
            "goldfish-tastes-great",
            &[],
            2,
            5,
            1_000_000,
            1_000_000_000,
            10_000_000,
        );

        assert!(matches!(result.status, RunStatus::FailedToStart));
        assert_eq!(result.exit_code, None);
        assert!(result.stdout.is_empty());
        assert!(result.stderr.contains("Failed to start process"));
    }

    #[test]
    fn large_output_is_truncated() {
        let result = run_command(
            "python3",
            &["-c", "print('x' * 100_000)"],
            2,
            5,
            1_000,
            1_000_000_000,
            10_000_000,
        );

        assert!(matches!(result.status, RunStatus::Succeeded));
        assert!(result.stdout_truncated);
        assert!(!result.stderr_truncated);
        assert!(result.stdout.len() <= 1_000);
    }

    #[test]
    fn large_output_does_not_block() {
        let result = run_command(
            "python3",
            &[
                "-c",
                "for i in range(1_000_000): print(i)",
            ],
            10,
            5,
            1_000,
            1_000_000_000,
            10_000_000,
        );

        assert!(matches!(result.status, RunStatus::Succeeded));
        assert!(result.stdout_truncated);
    }

    #[test]
    fn memory_limit_prevents_unbounded_allocation() {
        let result = run_command(
            "python3",
            &[
                "-c",
                "items = []\nwhile True:\n    items.append('x' * 10_000_000)",
            ],
            5,
            5,
            1_000_000,
            256_000_000,
            10_000_000,
        );

        assert!(matches!(result.status, RunStatus::Failed));
        assert!(result.stderr.contains("MemoryError"));
    }

    #[test]
    fn cpu_limit_stops_busy_loop() {
        let result = run_command(
            "python3",
            &["-c", "while True: pass"],
            5,
            1,
            1_000_000,
            256_000_000,
            10_000_000,
        );

        assert!(matches!(result.status, RunStatus::Signaled));
        assert_eq!(result.signal, Some(libc::SIGKILL));
    }

    #[test]
    fn file_size_limit_stops_large_file_write() {
        let result = run_command(
            "python3",
            &[
                "-c",
                "with open('test_output.bin', 'wb') as f:\n    while True:\n        f.write(b'x' * 1_000_000)",
            ],
            5,
            2,
            1_000_000,
            256_000_000,
            10_000_000,
        );

        assert!(matches!(result.status, RunStatus::Failed));
        assert!(result.stderr.contains("File too large"));

        let _ = std::fs::remove_file("test_output.bin");
    }
}