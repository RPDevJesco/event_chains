use crate::core::event_context::EventContext;
use crate::core::event_result::EventResult;
use crate::events::chainable_event::ChainableEvent;
use crate::events::event_middleware::EventMiddleware;
use std::time::Duration;

/// Backoff strategy for retry attempts
#[derive(Debug, Clone, Copy)]
pub enum BackoffStrategy {
    /// No delay between retries
    None,
    /// Fixed delay between retries
    Fixed(Duration),
    /// Exponential backoff: delay doubles after each retry
    Exponential {
        initial: Duration,
        max: Duration,
    },
    /// Linear backoff: delay increases by a fixed amount
    Linear {
        initial: Duration,
        increment: Duration,
    },
}

/// Middleware that retries failed events with configurable strategies
///
/// # Middleware Failures
///
/// This middleware does not produce its own infrastructure failures.
/// It transparently passes through `MiddlewareFailure` from downstream
/// without retrying them, since middleware failures indicate infrastructure
/// problems that retrying won't fix.
///
/// # Retry Behavior
///
/// - **Event failures** (`EventResult::Failure`): Retried according to strategy
/// - **Middleware failures** (`EventResult::MiddlewareFailure`): NOT retried, passed through immediately
/// - **Success**: Returned immediately
///
/// # Example
///
/// ```ignore
/// use event_chains::middleware::retry::{RetryMiddleware, BackoffStrategy};
/// use std::time::Duration;
///
/// // Simple retry with no delay
/// let chain = EventChain::new()
///     .middleware(RetryMiddleware::new(3))
///     .event(MyEvent);
///
/// // Exponential backoff
/// let chain = EventChain::new()
///     .middleware(
///         RetryMiddleware::new(5)
///             .with_backoff(BackoffStrategy::Exponential {
///                 initial: Duration::from_millis(100),
///                 max: Duration::from_secs(5),
///             })
///     )
///     .event(MyEvent);
/// ```
pub struct RetryMiddleware {
    max_retries: usize,
    backoff: BackoffStrategy,
    log_retries: bool,
}

impl RetryMiddleware {
    /// Create a new retry middleware with the specified maximum number of retries
    pub fn new(max_retries: usize) -> Self {
        Self {
            max_retries,
            backoff: BackoffStrategy::None,
            log_retries: true,
        }
    }

    /// Set the backoff strategy
    pub fn with_backoff(mut self, backoff: BackoffStrategy) -> Self {
        self.backoff = backoff;
        self
    }

    /// Configure whether to log retry attempts
    pub fn with_logging(mut self, enabled: bool) -> Self {
        self.log_retries = enabled;
        self
    }

    /// Create retry middleware with exponential backoff
    pub fn exponential(max_retries: usize, initial: Duration, max: Duration) -> Self {
        Self::new(max_retries).with_backoff(BackoffStrategy::Exponential { initial, max })
    }

    /// Create retry middleware with fixed delay
    pub fn fixed(max_retries: usize, delay: Duration) -> Self {
        Self::new(max_retries).with_backoff(BackoffStrategy::Fixed(delay))
    }

    fn calculate_delay(&self, attempt: usize) -> Duration {
        match self.backoff {
            BackoffStrategy::None => Duration::from_millis(0),
            BackoffStrategy::Fixed(delay) => delay,
            BackoffStrategy::Exponential { initial, max } => {
                let multiplier = 2u32.pow(attempt as u32 - 1);
                let delay = initial * multiplier;
                delay.min(max)
            }
            BackoffStrategy::Linear { initial, increment } => {
                initial + increment * (attempt as u32 - 1)
            }
        }
    }
}

impl EventMiddleware for RetryMiddleware {
    fn execute(
        &self,
        event: &dyn ChainableEvent,
        context: &mut EventContext,
        next: &mut dyn FnMut(&mut EventContext) -> EventResult<()>,
    ) -> EventResult<()> {
        let mut attempts = 0;

        loop {
            attempts += 1;
            let result = next(context);

            match &result {
                EventResult::Success(_) => {
                    if attempts > 1 && self.log_retries {
                        println!(
                            " {} succeeded after {} attempts",
                            event.name(),
                            attempts
                        );
                    }
                    return result;
                }
                EventResult::MiddlewareFailure(_) => {
                    // DO NOT retry middleware failures - they indicate infrastructure problems
                    // Pass them through immediately
                    if self.log_retries {
                        println!(
                            " {} middleware failure - not retrying (infrastructure issue)",
                            event.name()
                        );
                    }
                    return result;
                }
                EventResult::Failure(err) => {
                    if attempts >= self.max_retries {
                        if self.log_retries {
                            println!(
                                " {} failed after {} attempts: {}",
                                event.name(),
                                attempts,
                                err
                            );
                        }
                        return result;
                    }

                    let delay = self.calculate_delay(attempts);

                    if self.log_retries {
                        if delay.is_zero() {
                            println!(
                                " {} attempt {}/{} failed, retrying immediately...",
                                event.name(),
                                attempts,
                                self.max_retries
                            );
                        } else {
                            println!(
                                " {} attempt {}/{} failed, retrying in {:?}...",
                                event.name(),
                                attempts,
                                self.max_retries,
                                delay
                            );
                        }
                    }

                    if !delay.is_zero() {
                        std::thread::sleep(delay);
                    }
                }
            }
        }
    }
}
