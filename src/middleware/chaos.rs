use crate::core::event_context::EventContext;
use crate::core::event_result::EventResult;
use crate::events::chainable_event::ChainableEvent;
use crate::events::event_middleware::EventMiddleware;
use std::sync::{Arc, Mutex};

/// Types of chaos that can be injected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChaosType {
    /// Inject random event failures
    RandomFailure,
    /// Inject random middleware (infrastructure) failures
    InfrastructureFailure,
    /// Inject random latency/delays
    Latency,
    /// Randomly skip event execution entirely
    Skip,
}

/// Configuration for chaos injection
#[derive(Debug, Clone)]
pub struct ChaosConfig {
    /// Probability of chaos occurring (0.0 to 1.0)
    pub probability: f64,
    /// Types of chaos to potentially inject
    pub chaos_types: Vec<ChaosType>,
    /// Minimum latency in milliseconds (for Latency chaos)
    pub min_latency_ms: u64,
    /// Maximum latency in milliseconds (for Latency chaos)
    pub max_latency_ms: u64,
}

impl Default for ChaosConfig {
    fn default() -> Self {
        Self {
            probability: 0.1, // 10% chance
            chaos_types: vec![ChaosType::RandomFailure],
            min_latency_ms: 50,
            max_latency_ms: 500,
        }
    }
}

/// Statistics about chaos injection
#[derive(Debug, Clone, Default)]
pub struct ChaosStats {
    pub total_events: u64,
    pub chaos_injected: u64,
    pub failures_injected: u64,
    pub infrastructure_failures_injected: u64,
    pub latency_injected: u64,
    pub skips_injected: u64,
}

/// Middleware that randomly injects failures for testing resilience
///
/// # Purpose
///
/// This middleware is designed for **testing only**. It helps validate:
/// - Fault tolerance modes work correctly
/// - Retry middleware handles transient failures
/// - Circuit breakers open appropriately
/// - System gracefully degrades under chaos
///
/// # ⚠️ WARNING
///
/// **NEVER USE IN PRODUCTION!** This middleware intentionally breaks things.
///
/// # Chaos Types
///
/// * `RandomFailure` - Returns `EventResult::Failure` (business logic failure)
/// * `InfrastructureFailure` - Returns `EventResult::MiddlewareFailure` (infrastructure problem)
/// * `Latency` - Injects random delays before executing the event
/// * `Skip` - Skips event execution entirely (returns success without running)
///
/// # BestEffort Mode Interaction
///
/// This middleware is perfect for testing BestEffort mode's differentiation:
/// - `RandomFailure`: Chain continues (business failure)
/// - `InfrastructureFailure`: Chain stops (infrastructure failure)
///
/// # Example
///
/// ```ignore
/// use event_chains::middleware::chaos::{ChaosMiddleware, ChaosConfig, ChaosType};
///
/// // Basic chaos: 20% chance of random failures
/// let chain = EventChain::new()
///     .middleware(ChaosMiddleware::new(0.2))
///     .event(MyEvent)
///     .with_fault_tolerance(FaultToleranceMode::BestEffort);
///
/// // Advanced chaos: multiple types
/// let config = ChaosConfig {
///     probability: 0.3,
///     chaos_types: vec![
///         ChaosType::RandomFailure,
///         ChaosType::Latency,
///         ChaosType::Skip,
///     ],
///     min_latency_ms: 100,
///     max_latency_ms: 1000,
/// };
///
/// let chaos = ChaosMiddleware::with_config(config);
/// let chain = EventChain::new()
///     .middleware(chaos.clone())
///     .event(Event1)
///     .event(Event2);
///
/// // Check stats after execution
/// chaos.print_stats();
/// ```
#[derive(Clone)]
pub struct ChaosMiddleware {
    config: ChaosConfig,
    stats: Arc<Mutex<ChaosStats>>,
    enabled: Arc<Mutex<bool>>,
    log_chaos: bool,
}

impl ChaosMiddleware {
    /// Create chaos middleware with simple failure probability
    pub fn new(probability: f64) -> Self {
        Self::with_config(ChaosConfig {
            probability: probability.clamp(0.0, 1.0),
            ..Default::default()
        })
    }

    /// Create chaos middleware with full configuration
    pub fn with_config(config: ChaosConfig) -> Self {
        Self {
            config,
            stats: Arc::new(Mutex::new(ChaosStats::default())),
            enabled: Arc::new(Mutex::new(true)),
            log_chaos: true,
        }
    }

    /// Enable or disable chaos injection at runtime
    pub fn set_enabled(&self, enabled: bool) {
        if let Ok(mut e) = self.enabled.lock() {
            *e = enabled;
        }
    }

    /// Check if chaos is currently enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.lock().map(|e| *e).unwrap_or(false)
    }

    /// Configure whether to log when chaos is injected
    pub fn with_logging(mut self, enabled: bool) -> Self {
        self.log_chaos = enabled;
        self
    }

    /// Get current chaos statistics
    pub fn get_stats(&self) -> Option<ChaosStats> {
        self.stats.lock().ok().map(|s| s.clone())
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        if let Ok(mut stats) = self.stats.lock() {
            *stats = ChaosStats::default();
        }
    }

    /// Print statistics to stdout
    pub fn print_stats(&self) {
        if let Ok(stats) = self.stats.lock() {
            println!("\n=== Chaos Injection Statistics ===");
            println!("Total events:                    {}", stats.total_events);
            println!("Chaos injected:                  {} ({:.1}%)",
                     stats.chaos_injected,
                     if stats.total_events > 0 {
                         (stats.chaos_injected as f64 / stats.total_events as f64) * 100.0
                     } else {
                         0.0
                     }
            );
            println!("  - Business failures:           {}", stats.failures_injected);
            println!("  - Infrastructure failures:     {}", stats.infrastructure_failures_injected);
            println!("  - Latency injections:          {}", stats.latency_injected);
            println!("  - Skipped events:              {}", stats.skips_injected);
            println!();
        }
    }

    fn should_inject_chaos(&self) -> bool {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hash, Hasher};

        // Use a simple random approach based on system time + a hash
        let mut hasher = RandomState::new().build_hasher();
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);

        let random_value = (hasher.finish() % 10000) as f64 / 10000.0;
        random_value < self.config.probability
    }

    fn select_chaos_type(&self) -> ChaosType {
        if self.config.chaos_types.is_empty() {
            return ChaosType::RandomFailure;
        }

        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hash, Hasher};

        let mut hasher = RandomState::new().build_hasher();
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);

        let idx = (hasher.finish() as usize) % self.config.chaos_types.len();
        self.config.chaos_types[idx]
    }

    fn random_latency_ms(&self) -> u64 {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hash, Hasher};

        let mut hasher = RandomState::new().build_hasher();
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);

        let range = self.config.max_latency_ms - self.config.min_latency_ms;
        if range == 0 {
            return self.config.min_latency_ms;
        }

        self.config.min_latency_ms + (hasher.finish() % range)
    }
}

impl EventMiddleware for ChaosMiddleware {
    fn execute(
        &self,
        event: &dyn ChainableEvent,
        context: &mut EventContext,
        next: &mut dyn FnMut(&mut EventContext) -> EventResult<()>,
    ) -> EventResult<()> {
        // Update stats
        if let Ok(mut stats) = self.stats.lock() {
            stats.total_events += 1;
        }

        // Check if enabled
        if !self.is_enabled() {
            return next(context);
        }

        // Decide whether to inject chaos
        if !self.should_inject_chaos() {
            return next(context);
        }

        // Record chaos injection
        if let Ok(mut stats) = self.stats.lock() {
            stats.chaos_injected += 1;
        }

        // Select and execute chaos type
        let chaos_type = self.select_chaos_type();

        match chaos_type {
            ChaosType::RandomFailure => {
                if self.log_chaos {
                    println!("    [CHAOS] Injecting random failure in {}", event.name());
                }
                if let Ok(mut stats) = self.stats.lock() {
                    stats.failures_injected += 1;
                }
                EventResult::Failure(format!("Chaos monkey struck: random failure in {}", event.name()))
            }

            ChaosType::InfrastructureFailure => {
                if self.log_chaos {
                    println!("    [CHAOS] Injecting infrastructure failure in {}", event.name());
                }
                if let Ok(mut stats) = self.stats.lock() {
                    stats.infrastructure_failures_injected += 1;
                }
                EventResult::MiddlewareFailure(format!(
                    "Chaos monkey struck: infrastructure failure in {}",
                    event.name()
                ))
            }

            ChaosType::Latency => {
                let latency_ms = self.random_latency_ms();
                if self.log_chaos {
                    println!("    [CHAOS] Injecting {}ms latency in {}", latency_ms, event.name());
                }
                if let Ok(mut stats) = self.stats.lock() {
                    stats.latency_injected += 1;
                }
                std::thread::sleep(std::time::Duration::from_millis(latency_ms));
                next(context)
            }

            ChaosType::Skip => {
                if self.log_chaos {
                    println!("   ️  [CHAOS] Skipping execution of {}", event.name());
                }
                if let Ok(mut stats) = self.stats.lock() {
                    stats.skips_injected += 1;
                }
                // Return success without executing the event
                EventResult::Success(())
            }
        }
    }
}

impl Default for ChaosMiddleware {
    fn default() -> Self {
        Self::new(0.1)
    }
}
