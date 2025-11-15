use crate::core::event_context::EventContext;
use crate::core::event_result::EventResult;
use crate::events::chainable_event::ChainableEvent;
use crate::events::event_middleware::EventMiddleware;
use std::time::{Duration, Instant};

/// Middleware that measures and logs event execution time
///
/// # Middleware Failures
///
/// This middleware does not produce infrastructure failures - timing measurement
/// is always available and never returns `MiddlewareFailure`.
///
/// # Example
///
/// ```ignore
/// use event_chains::middleware::timing::TimingMiddleware;
/// use std::time::Duration;
///
/// // Log all events
/// let chain = EventChain::new()
///     .middleware(TimingMiddleware::new())
///     .event(MyEvent);
///
/// // Only log slow events (> 100ms)
/// let chain = EventChain::new()
///     .middleware(TimingMiddleware::new().with_threshold(Duration::from_millis(100)))
///     .event(MyEvent);
/// ```
pub struct TimingMiddleware {
    threshold: Option<Duration>,
    store_in_context: bool,
}

impl TimingMiddleware {
    /// Create a new timing middleware that logs all event durations
    pub fn new() -> Self {
        Self {
            threshold: None,
            store_in_context: false,
        }
    }

    /// Only log events that take longer than the specified threshold
    pub fn with_threshold(mut self, threshold: Duration) -> Self {
        self.threshold = Some(threshold);
        self
    }

    /// Store timing information in the context for later retrieval
    ///
    /// Timing data will be stored with key: `"timing:{event_name}"`
    pub fn store_in_context(mut self) -> Self {
        self.store_in_context = true;
        self
    }

    fn should_log(&self, duration: Duration) -> bool {
        match self.threshold {
            Some(threshold) => duration >= threshold,
            None => true,
        }
    }

    fn format_duration(duration: Duration) -> String {
        let micros = duration.as_micros();
        if micros < 1_000 {
            format!("{}µs", micros)
        } else if micros < 1_000_000 {
            format!("{:.2}ms", micros as f64 / 1_000.0)
        } else {
            format!("{:.2}s", duration.as_secs_f64())
        }
    }
}

impl EventMiddleware for TimingMiddleware {
    fn execute(
        &self,
        event: &dyn ChainableEvent,
        context: &mut EventContext,
        next: &mut dyn FnMut(&mut EventContext) -> EventResult<()>,
    ) -> EventResult<()> {
        let start = Instant::now();
        let result = next(context);
        let duration = start.elapsed();

        if self.should_log(duration) {
            println!(
                "  {} took {}",
                event.name(),
                Self::format_duration(duration)
            );
        }

        if self.store_in_context {
            let key = format!("timing:{}", event.name());
            context.set(&key, duration.as_micros() as u64);
        }

        result
    }
}

impl Default for TimingMiddleware {
    fn default() -> Self {
        Self::new()
    }
}
