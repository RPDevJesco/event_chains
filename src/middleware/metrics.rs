use crate::core::event_context::EventContext;
use crate::core::event_result::EventResult;
use crate::events::chainable_event::ChainableEvent;
use crate::events::event_middleware::EventMiddleware;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Statistics for a single event
#[derive(Debug, Clone)]
pub struct EventMetrics {
    pub event_name: String,
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub total_duration_micros: u64,
    pub min_duration_micros: u64,
    pub max_duration_micros: u64,
}

impl EventMetrics {
    fn new(event_name: String) -> Self {
        Self {
            event_name,
            total_executions: 0,
            successful_executions: 0,
            failed_executions: 0,
            total_duration_micros: 0,
            min_duration_micros: u64::MAX,
            max_duration_micros: 0,
        }
    }

    fn record(&mut self, duration_micros: u64, success: bool) {
        self.total_executions += 1;
        if success {
            self.successful_executions += 1;
        } else {
            self.failed_executions += 1;
        }

        self.total_duration_micros += duration_micros;
        self.min_duration_micros = self.min_duration_micros.min(duration_micros);
        self.max_duration_micros = self.max_duration_micros.max(duration_micros);
    }

    /// Get the average execution time in microseconds
    pub fn avg_duration_micros(&self) -> u64 {
        if self.total_executions == 0 {
            0
        } else {
            self.total_duration_micros / self.total_executions
        }
    }

    /// Get the success rate as a percentage (0.0 - 100.0)
    pub fn success_rate(&self) -> f64 {
        if self.total_executions == 0 {
            0.0
        } else {
            (self.successful_executions as f64 / self.total_executions as f64) * 100.0
        }
    }
}

/// Middleware that collects execution metrics for events
///
/// # Middleware Failures
///
/// In BestEffort mode, if metrics collection infrastructure fails (e.g., cannot
/// acquire lock on metrics storage), this middleware returns `EventResult::MiddlewareFailure`,
/// which will stop execution since metrics infrastructure must be reliable.
///
/// # Example
///
/// ```ignore
/// use event_chains::middleware::metrics::MetricsMiddleware;
/// use std::sync::Arc;
///
/// let metrics = MetricsMiddleware::new();
/// let metrics_clone = metrics.clone();
///
/// let chain = EventChain::new()
///     .middleware(metrics)
///     .event(MyEvent);
///
/// chain.execute(&mut context);
///
/// // Later, retrieve metrics
/// metrics_clone.print_summary();
/// let event_stats = metrics_clone.get_metrics("MyEvent");
/// ```
#[derive(Clone)]
pub struct MetricsMiddleware {
    metrics: Arc<Mutex<std::collections::HashMap<String, EventMetrics>>>,
    fail_on_lock_error: bool,  // For BestEffort mode: fail if can't record metrics
}

impl MetricsMiddleware {
    /// Create a new metrics middleware
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(Mutex::new(std::collections::HashMap::new())),
            fail_on_lock_error: true,  // Default: fail if metrics infrastructure broken
        }
    }

    /// Configure whether to fail on lock errors (default: true)
    ///
    /// When true (default), returns MiddlewareFailure if metrics cannot be recorded.
    /// When false, silently continues if metrics recording fails.
    pub fn with_fail_on_error(mut self, fail: bool) -> Self {
        self.fail_on_lock_error = fail;
        self
    }

    /// Get metrics for a specific event
    pub fn get_metrics(&self, event_name: &str) -> Option<EventMetrics> {
        self.metrics
            .lock()
            .ok()?
            .get(event_name)
            .cloned()
    }

    /// Get all collected metrics
    pub fn get_all_metrics(&self) -> Vec<EventMetrics> {
        self.metrics
            .lock()
            .ok()
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Reset all metrics
    pub fn reset(&self) {
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.clear();
        }
    }

    /// Print a summary of all metrics to stdout
    pub fn print_summary(&self) {
        let Ok(metrics) = self.metrics.lock() else {
            eprintln!("Warning: Could not acquire metrics lock for printing");
            return;
        };

        println!("\n=== Event Metrics Summary ===");
        println!("{:<25} {:>10} {:>10} {:>10} {:>12} {:>12} {:>12} {:>10}",
                 "Event", "Total", "Success", "Failed", "Avg (µs)", "Min (µs)", "Max (µs)", "Success %");
        println!("{}", "-".repeat(115));

        let mut sorted: Vec<_> = metrics.values().collect();
        sorted.sort_by(|a, b| a.event_name.cmp(&b.event_name));

        for metric in sorted {
            println!(
                "{:<25} {:>10} {:>10} {:>10} {:>12} {:>12} {:>12} {:>9.1}%",
                metric.event_name,
                metric.total_executions,
                metric.successful_executions,
                metric.failed_executions,
                metric.avg_duration_micros(),
                metric.min_duration_micros,
                metric.max_duration_micros,
                metric.success_rate()
            );
        }
        println!();
    }
}

impl EventMiddleware for MetricsMiddleware {
    fn execute(
        &self,
        event: &dyn ChainableEvent,
        context: &mut EventContext,
        next: &mut dyn FnMut(&mut EventContext) -> EventResult<()>,
    ) -> EventResult<()> {
        let start = Instant::now();
        let result = next(context);
        let duration = start.elapsed();

        // Try to record metrics
        let record_result = self.metrics.lock().map(|mut metrics| {
            let event_metrics = metrics
                .entry(event.name().to_string())
                .or_insert_with(|| EventMetrics::new(event.name().to_string()));

            event_metrics.record(duration.as_micros() as u64, result.is_success());
        });

        // If we failed to record metrics and fail_on_lock_error is true, return middleware failure
        if record_result.is_err() && self.fail_on_lock_error {
            return EventResult::MiddlewareFailure(
                format!("Metrics infrastructure failure: could not record metrics for {}", event.name())
            );
        }

        result
    }
}

impl Default for MetricsMiddleware {
    fn default() -> Self {
        Self::new()
    }
}
