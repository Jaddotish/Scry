# Scry
Scry is a small Linux sandbox written in Rust for running programs with resource limits, namespace isolation, and basic filesystem activity tracking.

### Features
- CPU, memory, filesize, process, file descriptor limits
- Wall-clock timeout with process-group cleanup
- Bounded `stdout`/`stderr` capture
- UTS, network, mount namespaces
- Isolated filesystem root using `pivot_root`
- Filesystem activity tracking with `strace`
- Human readable and JSON output

### Example
```
cargo build --bins

./target/debug/scry python3 test_scripts/hello.py
```
```
=== RUN RESULT ===
command: python3
status: succeeded
exit code: 0
duration: 0.02 seconds

--- stdout --- 
hello
```
To run JSON output, simply add the `--json` flag before the command.
You can also change the wall-clock timeout with `--timeout` followed by a number (seconds).

### How it works
Scry starts the target program inside a set of Linux namespaces and applies resource limits before execution.

The sandbox currently uses UTS, network, and mount namespaces, as well as Linux `rlimit`s for resource restrictions, process groups, and `strace` for filesystem activity.

A helper process sets up the sandbox filesystem and uses `pivot_root` before executing the target command.

### Tests
```
cargo test
```
The test suite covers normal execution, timeouts, resource limits, output truncation, filesystem tracking, JSON output, namespace isolation.

### Requirements
Scry is currently Linux only.

Requirements include:
- Rust/Cargo
- `strace`
- `unshare`
- user namespaces enabled

### Limitations
Scry is a learning project, not a fully developed security boundary yet.

There is currently no seccomp syscall filtering or PID namespace isolation yet, some host system directories are bind-mounted so normal executables can run, filesystem tracing only covers selected operations, and the sandbox hasn't been security-audited.

I plan to continue extending Scry as I learn more about Linux sandboxing.