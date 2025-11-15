use crate::core::event_failure::EventFailure;

/// Chain execution result
#[derive(Debug)]
pub struct ChainResult {
    pub success: bool,
    pub failures: Vec<EventFailure>,
    pub status: ChainStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainStatus {
    Completed,
    CompletedWithWarnings,
    Failed,
}

impl ChainResult {
    pub fn success() -> Self {
        Self {
            success: true,
            failures: Vec::new(),
            status: ChainStatus::Completed,
        }
    }

    pub fn partial_success(failures: Vec<EventFailure>) -> Self {
        Self {
            success: true,
            failures,
            status: ChainStatus::CompletedWithWarnings,
        }
    }

    pub fn failure(failures: Vec<EventFailure>) -> Self {
        Self {
            success: false,
            failures,
            status: ChainStatus::Failed,
        }
    }
}
