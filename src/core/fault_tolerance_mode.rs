/// Fault tolerance mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultToleranceMode {
    Strict,
    Lenient,
    BestEffort,
}
