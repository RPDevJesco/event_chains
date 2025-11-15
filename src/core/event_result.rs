/// Result of an event execution
#[derive(Debug, Clone)]
pub enum EventResult<T> {
    Success(T),
    Failure(String),
}

impl<T> EventResult<T> {
    pub fn is_success(&self) -> bool {
        matches!(self, EventResult::Success(_))
    }

    pub fn is_failure(&self) -> bool {
        matches!(self, EventResult::Failure(_))
    }

    pub fn get_data(self) -> Option<T> {
        match self {
            EventResult::Success(data) => Some(data),
            EventResult::Failure(_) => None,
        }
    }

    pub fn get_error(&self) -> Option<&str> {
        match self {
            EventResult::Success(_) => None,
            EventResult::Failure(msg) => Some(msg),
        }
    }
}
