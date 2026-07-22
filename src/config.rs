pub struct RunConfig {
    pub timeout_secs: u64,
    pub cpu_limit_secs: u64,
    pub max_output_bytes: usize,
    pub memory_limit_bytes: u64,
    pub file_size_limit_bytes: u64,
    pub open_file_limit: u64,
    pub process_limit: u64,
}