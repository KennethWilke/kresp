pub struct RespConfig {
    pub max_resp_size: usize,
    pub max_buffer_size: usize,
}

const DEFAULT_MAX: usize = 512 * 1024 * 1024;

impl Default for RespConfig {
    fn default() -> Self {
        Self::new(DEFAULT_MAX, DEFAULT_MAX)
    }
}

impl RespConfig {
    pub fn new(max_resp_size: usize, max_buffer_size: usize) -> Self {
        RespConfig {
            max_resp_size,
            max_buffer_size,
        }
    }
}
