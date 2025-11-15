use crate::core::event_context::EventContext;
use crate::core::event_result::EventResult;
use crate::events::chainable_event::ChainableEvent;
use crate::events::event_middleware::EventMiddleware;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Rate limiting strategy
#[derive(Debug, Clone, Copy)]
pub enum RateLimitStrategy {
    /// Block execution if rate limit is exceeded
    Block,
    /// Wait until the rate limit window resets
    Wait,
}

/// Simple token bucket rate limiter
#[derive(Clone)]
pub struct RateLimiter {
    tokens: Arc<Mutex<f64>>,
    max_tokens: f64,
    refill_rate: f64, // tokens per second
    last_refill: Arc<Mutex<Instant>>,
}

impl RateLimiter {
    fn new(max_tokens: f64, refill_rate: f64) -> Self {
        Self {
            tokens: Arc::new(Mutex::new(max_tokens)),
            max_tokens,
            refill_rate,
            last_refill: Arc::new(Mutex::new(Instant::now())),
        }
    }

    fn try_consume(&self, strategy: RateLimitStrategy) -> Result<(), Duration> {
        loop {
            self.refill();

            let mut tokens = self.tokens.lock().unwrap();

            if *tokens >= 1.0 {
                *tokens -= 1.0;
                return Ok(());
            }

            match strategy {
                RateLimitStrategy::Block => {
                    let wait_time = Duration::from_secs_f64(1.0 / self.refill_rate);
                    return Err(wait_time);
                }
                RateLimitStrategy::Wait => {
                    drop(tokens);
                    let wait_time = Duration::from_secs_f64(1.0 / self.refill_rate);
                    std::thread::sleep(wait_time);
                    continue;
                }
            }
        }
    }

    fn refill(&self) {
        let mut last_refill = self.last_refill.lock().unwrap();
        let now = Instant::now();
        let elapsed = now.duration_since(*last_refill).as_secs_f64();

        if elapsed > 0.0 {
            let mut tokens = self.tokens.lock().unwrap();
            let new_tokens = elapsed * self.refill_rate;
            *tokens = (*tokens + new_tokens).min(self.max_tokens);
            *last_refill = now;
        }
    }
}

/// Middleware that enforces rate limiting on event execution
///
/// # Middleware Failures
///
/// This middleware does NOT produce `MiddlewareFailure`. Rate limiting is a
/// business rule, not an infrastructure failure. When rate limit is exceeded,
/// it returns `EventResult::Failure`.
///
/// # Rationale
///
/// Rate limiting represents a policy decision (too many requests), not an
/// infrastructure problem. In BestEffort mode, the chain will continue if
/// rate limited, attempting subsequent events.
///
/// # Example
///
/// ```ignore
/// use event_chains::middleware::rate_limit::{RateLimitMiddleware, RateLimitStrategy};
///
/// // Allow 10 events per second, block if exceeded
/// let chain = EventChain::new()
///     .middleware(
///         RateLimitMiddleware::new(10, RateLimitStrategy::Block)
///     )
///     .event(MyEvent);
///
/// // Allow 5 events per second, wait if exceeded
/// let chain = EventChain::new()
///     .middleware(
///         RateLimitMiddleware::new(5, RateLimitStrategy::Wait)
///     )
///     .event(MyEvent);
/// ```
pub struct RateLimitMiddleware {
    limiter: RateLimiter,
    strategy: RateLimitStrategy,
    log_limits: bool,
}

impl RateLimitMiddleware {
    /// Create a new rate limit middleware
    ///
    /// # Arguments
    ///
    /// * `requests_per_second` - Maximum number of events allowed per second
    /// * `strategy` - How to handle rate limit violations
    pub fn new(requests_per_second: u32, strategy: RateLimitStrategy) -> Self {
        let rps = requests_per_second as f64;
        Self {
            limiter: RateLimiter::new(rps, rps),
            strategy,
            log_limits: true,
        }
    }

    /// Create a rate limiter with custom burst capacity
    ///
    /// # Arguments
    ///
    /// * `burst_capacity` - Maximum number of events that can be executed in a burst
    /// * `requests_per_second` - Rate at which capacity is refilled
    /// * `strategy` - How to handle rate limit violations
    pub fn with_burst(
        burst_capacity: u32,
        requests_per_second: u32,
        strategy: RateLimitStrategy,
    ) -> Self {
        Self {
            limiter: RateLimiter::new(burst_capacity as f64, requests_per_second as f64),
            strategy,
            log_limits: true,
        }
    }

    /// Configure whether to log rate limit violations
    pub fn with_logging(mut self, enabled: bool) -> Self {
        self.log_limits = enabled;
        self
    }
}

impl EventMiddleware for RateLimitMiddleware {
    fn execute(
        &self,
        event: &dyn ChainableEvent,
        context: &mut EventContext,
        next: &mut dyn FnMut(&mut EventContext) -> EventResult<()>,
    ) -> EventResult<()> {
        match self.limiter.try_consume(self.strategy) {
            Ok(()) => next(context),
            Err(wait_time) => {
                if self.log_limits {
                    println!(
                        " Rate limit exceeded for {}. Try again in {:?}",
                        event.name(),
                        wait_time
                    );
                }
                // Rate limiting is a policy/business rule, not infrastructure failure
                // Use Failure, not MiddlewareFailure
                EventResult::Failure("Rate limit exceeded".to_string())
            }
        }
    }
}
