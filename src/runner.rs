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

fn is_successful_write_open(line: &str) -> bool {
    line.starts_with("openat(")
        && (line.contains("O_WRONLY") || line.contains("O_RDWR"))
        && !line.contains("= -1")
}

fn is_successful_unlink(line: &str) -> bool {
    line.starts_with("unlink(")
        && !line.contains("= -1")
}

fn is_successful_mkdir(line: &str) -> bool {
    line.starts_with("mkdir(")
        && !line.contains("= -1")
}

fn is_successful_rmdir(line: &str) -> bool {
    line.starts_with("rmdir(")
        && !line.contains("= -1")
}

fn is_successful_rename(line: &str) -> bool {
    line.starts_with("rename(")
        && !line.contains("= -1")
}

fn extract_quoted_paths(line: &str) -> Vec<&str> {
    let mut paths = Vec::new();
    let mut remaining = line;

    while let Some(first_quote) = remaining.find('"') {
        remaining = &remaining[first_quote + 1..];

        let Some(second_quote) = remaining.find('"') else {
            break;
        };

        paths.push(&remaining[..second_quote]);
        remaining = &remaining[second_quote + 1..];
    }

    paths
}

fn extract_quoted_path(line: &str) -> Option<&str> {
    let first_quote = line.find('"')?;
    let rest = &line[first_quote + 1..];
    let second_quote = rest.find('"')?;

    Some(&rest[..second_quote])
}


pub fn run_command(
    command: &str,
    args: &[&str],
    timeout_secs: u64,
    cpu_limit_secs: u64,
    max_output_bytes: usize,
    memory_limit_bytes: u64,
    file_size_limit_bytes: u64,
    open_file_limit: u64,
    process_limit: u64,
) -> RunResult {
    let start = Instant::now();

    let trace_file = tempfile::NamedTempFile::new()
        .expect("Could not create trace file");
    let trace_path = trace_file.path().to_path_buf();

    let mut cmd = Command::new("strace");

    cmd.arg("-o")
        .arg(&trace_path)
        .arg("-e")
        .arg("trace=openat,unlink,rename,mkdir,rmdir")
        .arg(command)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);

    unsafe {
        cmd.pre_exec(move || {
            let mem_rlimit = libc::rlimit {
                rlim_cur: memory_limit_bytes,
                rlim_max: memory_limit_bytes,
            };

            let cpu_rlimit = libc::rlimit {
                rlim_cur: cpu_limit_secs,
                rlim_max: cpu_limit_secs,
            };

            let file_size_rlimit = libc::rlimit {
                rlim_cur: file_size_limit_bytes,
                rlim_max: file_size_limit_bytes,
            };

            let open_file_rlimit = libc::rlimit {
                rlim_cur: open_file_limit,
                rlim_max: open_file_limit,
            };

            let process_rlimit = libc::rlimit {
                rlim_cur: process_limit,
                rlim_max: process_limit,
            };


            let mem_result = libc::setrlimit(libc::RLIMIT_AS, &mem_rlimit);
            let cpu_result = libc::setrlimit(libc::RLIMIT_CPU, &cpu_rlimit);
            let file_size_result = libc::setrlimit(libc::RLIMIT_FSIZE, &file_size_rlimit);
            let open_file_result = libc::setrlimit(libc::RLIMIT_NOFILE, &open_file_rlimit);
            let process_result = libc::setrlimit(libc::RLIMIT_NPROC, &process_rlimit);

            
            if mem_result != 0 {
                return Err(std::io::Error::last_os_error());
            }

            if cpu_result != 0 {
                return Err(std::io::Error::last_os_error());
            }

            if file_size_result != 0 {
                return Err(std::io::Error::last_os_error());
            }

            if open_file_result != 0 {
                return Err(std::io::Error::last_os_error());
            }

            if process_result != 0 {
                return Err(std::io::Error::last_os_error());
            }

            Ok(())
        });
    }

    let mut child = match cmd.spawn()
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
                files_opened_for_writing: Vec::new(),
                files_deleted: Vec::new(),
                directories_created: Vec::new(),
                directories_deleted: Vec::new(),
                files_renamed: Vec::new(),
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

                let stderr_text = 
                    String::from_utf8_lossy(&stderr_bytes).to_string();

                let command_failed_to_start =
                    stderr_text.starts_with("strace: Cannot find executable");

                let trace_contents = std::fs::read_to_string(&trace_path)
                    .expect("Could not read trace file");

                let mut files_opened_for_writing = Vec::new();
                let mut files_deleted = Vec::new();
                let mut directories_created = Vec::new();
                let mut directories_deleted = Vec::new();
                let mut files_renamed = Vec::new();

                for line in trace_contents.lines() {
                    if is_successful_write_open(line) {
                        if let Some(path) = extract_quoted_path(line) {
                            files_opened_for_writing.push(path.to_string());
                        }
                    }

                    if is_successful_unlink(line) {
                        if let Some(path) = extract_quoted_path(line) {
                            files_deleted.push(path.to_string());
                        }
                    }

                    if is_successful_mkdir(line) {
                        if let Some(path) = extract_quoted_path(line) {
                            directories_created.push(path.to_string());
                        }
                    }

                    if is_successful_rmdir(line) {
                        if let Some(path) = extract_quoted_path(line) {
                            directories_deleted.push(path.to_string());
                        }
                    }

                    if is_successful_rename(line) {
                        let paths = extract_quoted_paths(line);

                        if paths.len() == 2 {
                            files_renamed.push((
                                paths[0].to_string(),
                                paths[1].to_string(),
                            ));
                        }
                    }
                }

                let run_status = if command_failed_to_start {
                    RunStatus::FailedToStart
                } else if status.signal().is_some() {
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
                    exit_code: if command_failed_to_start {
                        None
                    } else {
                        status.code()
                    },
                    signal: status.signal(),
                    duration: start.elapsed().as_secs_f64(),
                    stdout: String::from_utf8_lossy(&stdout_bytes).to_string(),
                    stderr: if command_failed_to_start {
                        format!("Failed to start process: {stderr_text}")
                    } else {
                        stderr_text
                    },
                    stdout_truncated,
                    stderr_truncated,
                    files_opened_for_writing,
                    files_deleted,
                    directories_created,
                    directories_deleted,
                    files_renamed,
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
                        files_opened_for_writing: Vec::new(),
                        files_deleted: Vec::new(),
                        directories_created: Vec::new(),
                        directories_deleted: Vec::new(),
                        files_renamed: Vec::new(),
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
                    files_opened_for_writing: Vec::new(),
                    files_deleted: Vec::new(),
                    directories_created: Vec::new(),
                    directories_deleted: Vec::new(),
                    files_renamed: Vec::new(),
                };
            }
        }
    }
}