use crate::core::event_context::EventContext;
use crate::core::event_result::EventResult;
use crate::events::chainable_event::ChainableEvent;
use crate::events::event_middleware::EventMiddleware;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed, requests flow normally
    Closed,
    /// Circuit is open, requests are blocked
    Open,
    /// Circuit is half-open, testing if service recovered
    HalfOpen,
}

struct CircuitBreakerState {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<Instant>,
    opened_at: Option<Instant>,
}

impl CircuitBreakerState {
    fn new() -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure_time: None,
            opened_at: None,
        }
    }
}

/// Middleware that implements the circuit breaker pattern
///
/// Prevents cascading failures by temporarily blocking requests
/// to a failing service, allowing it time to recover.
///
/// # Middleware Failures
///
/// This middleware does NOT produce `MiddlewareFailure`. Circuit breaker state
/// (open/closed) is a protection mechanism, not an infrastructure failure.
/// When the circuit is open, it returns `EventResult::Failure`.
///
/// # Rationale
///
/// The circuit breaker pattern is a business logic protection mechanism.
/// An open circuit means "the downstream service is failing, don't call it",
/// not "the circuit breaker infrastructure is broken". Therefore it returns
/// `Failure` (business logic) not `MiddlewareFailure` (infrastructure).
///
/// In BestEffort mode, an open circuit will allow the chain to continue with
/// other events, which is the desired behavior for resilient cleanup/recovery.
///
/// # Example
///
/// ```ignore
/// use event_chains::middleware::circuit_breaker::CircuitBreakerMiddleware;
/// use std::time::Duration;
///
/// let chain = EventChain::new()
///     .middleware(
///         CircuitBreakerMiddleware::new()
///             .with_failure_threshold(5)
///             .with_timeout(Duration::from_secs(30))
///     )
///     .event(ExternalApiEvent);
/// ```
pub struct CircuitBreakerMiddleware {
    state: Arc<Mutex<CircuitBreakerState>>,
    failure_threshold: u32,
    success_threshold: u32,
    timeout: Duration,
    log_state_changes: bool,
}

impl CircuitBreakerMiddleware {
    /// Create a new circuit breaker with default settings
    ///
    /// Defaults:
    /// - Failure threshold: 5
    /// - Success threshold: 2
    /// - Timeout: 60 seconds
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(CircuitBreakerState::new())),
            failure_threshold: 5,
            success_threshold: 2,
            timeout: Duration::from_secs(60),
            log_state_changes: true,
        }
    }

    /// Set the number of consecutive failures before opening the circuit
    pub fn with_failure_threshold(mut self, threshold: u32) -> Self {
        self.failure_threshold = threshold;
        self
    }

    /// Set the number of consecutive successes needed to close the circuit from half-open
    pub fn with_success_threshold(mut self, threshold: u32) -> Self {
        self.success_threshold = threshold;
        self
    }

    /// Set the timeout duration before attempting to close the circuit
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Configure whether to log state changes
    pub fn with_logging(mut self, enabled: bool) -> Self {
        self.log_state_changes = enabled;
        self
    }

    /// Get the current circuit state
    pub fn get_state(&self) -> CircuitState {
        self.state.lock().unwrap().state
    }

    /// Manually reset the circuit breaker to closed state
    pub fn reset(&self) {
        let mut state = self.state.lock().unwrap();
        state.state = CircuitState::Closed;
        state.failure_count = 0;
        state.success_count = 0;
        state.last_failure_time = None;
        state.opened_at = None;

        if self.log_state_changes {
            println!(" Circuit breaker manually reset to CLOSED");
        }
    }

    fn should_attempt_reset(&self, state: &CircuitBreakerState) -> bool {
        if state.state != CircuitState::Open {
            return false;
        }

        if let Some(opened_at) = state.opened_at {
            Instant::now().duration_since(opened_at) >= self.timeout
        } else {
            false
        }
    }

    fn record_success(&self, event_name: &str) {
        let mut state = self.state.lock().unwrap();
        state.failure_count = 0;

        match state.state {
            CircuitState::Closed => {
                // Already closed, nothing to do
            }
            CircuitState::HalfOpen => {
                state.success_count += 1;
                if state.success_count >= self.success_threshold {
                    state.state = CircuitState::Closed;
                    state.success_count = 0;
                    state.opened_at = None;

                    if self.log_state_changes {
                        println!(" Circuit breaker CLOSED for {}", event_name);
                    }
                }
            }
            CircuitState::Open => {
                // Shouldn't happen, but handle it
                state.state = CircuitState::Closed;
                state.success_count = 0;
                state.opened_at = None;
            }
        }
    }

    fn record_failure(&self, event_name: &str) {
        let mut state = self.state.lock().unwrap();
        state.last_failure_time = Some(Instant::now());

        match state.state {
            CircuitState::Closed => {
                state.failure_count += 1;
                if state.failure_count >= self.failure_threshold {
                    state.state = CircuitState::Open;
                    state.opened_at = Some(Instant::now());

                    if self.log_state_changes {
                        println!(
                            " Circuit breaker OPENED for {} (failures: {})",
                            event_name, state.failure_count
                        );
                    }
                }
            }
            CircuitState::HalfOpen => {
                state.state = CircuitState::Open;
                state.success_count = 0;
                state.opened_at = Some(Instant::now());

                if self.log_state_changes {
                    println!(" Circuit breaker re-OPENED for {}", event_name);
                }
            }
            CircuitState::Open => {
                // Already open, just update the time
            }
        }
    }
}

impl EventMiddleware for CircuitBreakerMiddleware {
    fn execute(
        &self,
        event: &dyn ChainableEvent,
        context: &mut EventContext,
        next: &mut dyn FnMut(&mut EventContext) -> EventResult<()>,
    ) -> EventResult<()> {
        // Check if we should attempt to close the circuit
        {
            let mut state = self.state.lock().unwrap();
            if self.should_attempt_reset(&state) {
                state.state = CircuitState::HalfOpen;
                state.success_count = 0;

                if self.log_state_changes {
                    println!(" Circuit breaker HALF-OPEN for {}", event.name());
                }
            }
        }

        // Check current state
        let current_state = self.state.lock().unwrap().state;

        match current_state {
            CircuitState::Open => {
                // Circuit breaker open is a protection mechanism, not infrastructure failure
                // Use Failure, not MiddlewareFailure
                return EventResult::Failure(format!(
                    "Circuit breaker is OPEN for {}",
                    event.name()
                ));
            }
            CircuitState::Closed | CircuitState::HalfOpen => {
                let result = next(context);

                match &result {
                    EventResult::Success(_) => {
                        self.record_success(event.name());
                    }
                    EventResult::Failure(_) | EventResult::MiddlewareFailure(_) => {
                        // Record both types of failures in circuit breaker
                        self.record_failure(event.name());
                    }
                }

                result
            }
        }
    }
}

impl Default for CircuitBreakerMiddleware {
    fn default() -> Self {
        Self::new()
    }
}
