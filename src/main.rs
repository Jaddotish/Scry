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
        max_output_bytes: 1_000_000,
        memory_limit_bytes: 256_000_000,
    };

    let result = run_command(
        command,
        &command_args,
        config.timeout_secs,
        config.max_output_bytes,
        config.memory_limit_bytes,
    );

    print_result(&result);
}