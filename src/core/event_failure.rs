/// Event failure information
#[derive(Debug, Clone)]
pub struct EventFailure {
    pub event_name: String,
    pub error_message: String,
    pub timestamp: u64,
    pub is_middleware_failure: bool,
}

impl EventFailure {
    pub fn new(event_name: String, error_message: String) -> Self {
        Self {
            event_name,
            error_message,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            is_middleware_failure: false,
        }
    }

    pub fn middleware_failure(event_name: String, error_message: String) -> Self {
        Self {
            event_name,
            error_message,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            is_middleware_failure: true,
        }
    }
}
