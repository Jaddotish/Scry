#[derive(Debug)]
pub enum RunStatus {
    Succeeded,
    Failed,
    TimedOut,
    FailedToStart,
    Signaled,
}

#[derive(Debug)]
pub struct RunResult {
    pub command: String,
    pub args: Vec<String>,
    pub status: RunStatus,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub duration: f64,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}