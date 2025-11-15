use crate::core::event_context::EventContext;
use crate::core::event_result::EventResult;
use crate::events::chainable_event::ChainableEvent;
use crate::events::event_middleware::EventMiddleware;

/// Logging levels for the middleware
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// Middleware that logs event execution with configurable log levels
///
/// # Middleware Failures
///
/// In BestEffort mode, if logging infrastructure fails (e.g., can't write to log file),
/// this middleware returns `EventResult::MiddlewareFailure`, which will stop execution
/// even in BestEffort mode, since logging infrastructure must be reliable.
///
/// # Example
///
/// ```ignore
/// use event_chains::middleware::logging::{LoggingMiddleware, LogLevel};
///
/// let chain = EventChain::new()
///     .middleware(LoggingMiddleware::new(LogLevel::Info))
///     .event(MyEvent);
/// ```
pub struct LoggingMiddleware {
    level: LogLevel,
    log_success: bool,
    log_failure: bool,
    fail_on_error: bool,  // For testing middleware failures
}

impl LoggingMiddleware {
    /// Create a new logging middleware with the specified log level
    pub fn new(level: LogLevel) -> Self {
        Self {
            level,
            log_success: true,
            log_failure: true,
            fail_on_error: false,
        }
    }

    /// Create a logging middleware that only logs errors
    pub fn errors_only() -> Self {
        Self {
            level: LogLevel::Error,
            log_success: false,
            log_failure: true,
            fail_on_error: false,
        }
    }

    /// Create a logging middleware with info level (default)
    pub fn info() -> Self {
        Self::new(LogLevel::Info)
    }

    /// Create a logging middleware with debug level
    pub fn debug() -> Self {
        Self::new(LogLevel::Debug)
    }

    /// Configure whether to log successful executions
    pub fn with_success_logging(mut self, enabled: bool) -> Self {
        self.log_success = enabled;
        self
    }

    /// Configure whether to log failures
    pub fn with_failure_logging(mut self, enabled: bool) -> Self {
        self.log_failure = enabled;
        self
    }

    /// For testing: simulate a middleware failure
    #[doc(hidden)]
    pub fn with_simulated_failure(mut self) -> Self {
        self.fail_on_error = true;
        self
    }

    fn log(&self, level: LogLevel, message: &str) -> Result<(), String> {
        if !self.should_log(level) {
            return Ok(());
        }

        // Simulate logging infrastructure failure for testing
        if self.fail_on_error && level == LogLevel::Error {
            return Err("Logging infrastructure failure: unable to write to log".to_string());
        }

        let prefix = match level {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        };
        println!("[{}] {}", prefix, message);
        Ok(())
    }

    fn should_log(&self, level: LogLevel) -> bool {
        level as u8 >= self.level as u8
    }
}

impl EventMiddleware for LoggingMiddleware {
    fn execute(
        &self,
        event: &dyn ChainableEvent,
        context: &mut EventContext,
        next: &mut dyn FnMut(&mut EventContext) -> EventResult<()>,
    ) -> EventResult<()> {
        // Try to log start
        if let Err(e) = self.log(LogLevel::Debug, &format!(" Starting event: {}", event.name())) {
            return EventResult::MiddlewareFailure(e);
        }

        let result = next(context);

        match &result {
            EventResult::Success(_) => {
                if self.log_success {
                    if let Err(e) = self.log(LogLevel::Info, &format!(" Completed event: {}", event.name())) {
                        return EventResult::MiddlewareFailure(e);
                    }
                }
            }
            EventResult::Failure(err) | EventResult::MiddlewareFailure(err) => {
                if self.log_failure {
                    if let Err(e) = self.log(
                        LogLevel::Error,
                        &format!(" Failed event: {} - {}", event.name(), err),
                    ) {
                        return EventResult::MiddlewareFailure(e);
                    }
                }
            }
        }

        result
    }
}

impl Default for LoggingMiddleware {
    fn default() -> Self {
        Self::info()
    }
}
