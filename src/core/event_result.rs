/// Result of an event execution
#[derive(Debug, Clone)]
pub enum EventResult<T> {
    Success(T),
    Failure(String),
    MiddlewareFailure(String),
}

impl<T> EventResult<T> {
    pub fn is_success(&self) -> bool {
        matches!(self, EventResult::Success(_))
    }

    pub fn is_failure(&self) -> bool {
        matches!(self, EventResult::Failure(_) | EventResult::MiddlewareFailure(_))
    }

    pub fn is_event_failure(&self) -> bool {
        matches!(self, EventResult::Failure(_))
    }

    pub fn is_middleware_failure(&self) -> bool {
        matches!(self, EventResult::MiddlewareFailure(_))
    }

    pub fn get_data(self) -> Option<T> {
        match self {
            EventResult::Success(data) => Some(data),
            EventResult::Failure(_) | EventResult::MiddlewareFailure(_) => None,
        }
    }

    pub fn get_error(&self) -> Option<&str> {
        match self {
            EventResult::Success(_) => None,
            EventResult::Failure(msg) | EventResult::MiddlewareFailure(msg) => Some(msg),
        }
    }

    /// Get the error type and message if this is a failure
    pub fn get_failure_info(&self) -> Option<(bool, &str)> {
        match self {
            EventResult::Success(_) => None,
            EventResult::Failure(msg) => Some((false, msg)),  // (is_middleware_failure, message)
            EventResult::MiddlewareFailure(msg) => Some((true, msg)),
        }
    }
}
