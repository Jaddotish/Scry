pub struct RunConfig {
    pub timeout_secs: u64,
    pub cpu_limit_secs: u64,
    pub max_output_bytes: usize,
    pub memory_limit_bytes: u64,
}