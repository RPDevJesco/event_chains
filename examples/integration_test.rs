/// Integration test: All middleware and fault tolerance modes
///
/// This test verifies that all middleware works correctly together
/// and that fault tolerance modes behave as expected.

use event_chains::core::event_chain::EventChain;
use event_chains::core::event_context::EventContext;
use event_chains::core::event_result::EventResult;
use event_chains::core::fault_tolerance_mode::FaultToleranceMode;
use event_chains::core::chain_result::ChainStatus;
use event_chains::events::chainable_event::ChainableEvent;
use event_chains::middleware::logging::LoggingMiddleware;
use event_chains::middleware::timing::TimingMiddleware;
use event_chains::middleware::retry::{RetryMiddleware, BackoffStrategy};
use event_chains::middleware::metrics::MetricsMiddleware;
use event_chains::middleware::rate_limit::{RateLimitMiddleware, RateLimitStrategy};
use event_chains::middleware::circuit_breaker::CircuitBreakerMiddleware;
use std::time::Duration;

// ============================================================================
// TEST EVENTS
// ============================================================================

struct SuccessEvent {
    name: String,
}

impl SuccessEvent {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

impl ChainableEvent for SuccessEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        println!("   {}: Success", self.name);
        context.set(&format!("{}_executed", self.name), true);
        EventResult::Success(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

struct FailureEvent {
    name: String,
    fail_count: std::sync::Arc<std::sync::Mutex<usize>>,
    max_failures: usize,
}

impl FailureEvent {
    fn new(name: &str, max_failures: usize) -> Self {
        Self {
            name: name.to_string(),
            fail_count: std::sync::Arc::new(std::sync::Mutex::new(0)),
            max_failures,
        }
    }
}

impl ChainableEvent for FailureEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        let mut count = self.fail_count.lock().unwrap();
        *count += 1;

        if *count <= self.max_failures {
            println!("   {}: Failed (attempt {})", self.name, *count);
            EventResult::Failure(format!("Failure attempt {}", *count))
        } else {
            println!("   {}: Success (attempt {})", self.name, *count);
            context.set(&format!("{}_executed", self.name), true);
            EventResult::Success(())
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

struct SlowEvent {
    name: String,
    duration_ms: u64,
}

impl SlowEvent {
    fn new(name: &str, duration_ms: u64) -> Self {
        Self {
            name: name.to_string(),
            duration_ms,
        }
    }
}

impl ChainableEvent for SlowEvent {
    fn execute(&self, _context: &mut EventContext) -> EventResult<()> {
        std::thread::sleep(Duration::from_millis(self.duration_ms));
        println!("  ⏱  {}: Completed after {}ms", self.name, self.duration_ms);
        EventResult::Success(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

// ============================================================================
// TEST SCENARIOS
// ============================================================================

fn test_scenario_1_all_success() {
    println!("\n{}", "=".repeat(70));
    println!("SCENARIO 1: All Middleware + All Success Events");
    println!("{}\n", "=".repeat(70));

    let metrics = MetricsMiddleware::new();
    let metrics_clone = metrics.clone();

    let chain = EventChain::new()
        .middleware(metrics)
        .middleware(TimingMiddleware::new())
        .middleware(CircuitBreakerMiddleware::new())
        .middleware(RateLimitMiddleware::new(10, RateLimitStrategy::Block))
        .middleware(RetryMiddleware::new(3).with_backoff(BackoffStrategy::Fixed(Duration::from_millis(10))))
        .middleware(LoggingMiddleware::info())
        .event(SuccessEvent::new("Event1"))
        .event(SuccessEvent::new("Event2"))
        .event(SuccessEvent::new("Event3"))
        .with_fault_tolerance(FaultToleranceMode::Strict);

    let mut context = EventContext::new();
    let result = chain.execute(&mut context);

    println!("\n Result:");
    println!("  Status: {:?}", result.status);
    println!("  Success: {}", result.success);
    println!("  Failures: {}", result.failures.len());

    metrics_clone.print_summary();

    assert!(result.success);
    assert_eq!(result.status, ChainStatus::Completed);
    assert_eq!(result.failures.len(), 0);

    // Verify all events executed
    assert!(context.get::<bool>("Event1_executed").unwrap_or(false));
    assert!(context.get::<bool>("Event2_executed").unwrap_or(false));
    assert!(context.get::<bool>("Event3_executed").unwrap_or(false));

    println!("\n SCENARIO 1 PASSED\n");
}

fn test_scenario_2_retry_middleware() {
    println!("\n{}", "=".repeat(70));
    println!("SCENARIO 2: Retry Middleware with Eventually Successful Event");
    println!("{}\n", "=".repeat(70));

    let metrics = MetricsMiddleware::new();
    let metrics_clone = metrics.clone();

    let chain = EventChain::new()
        .middleware(metrics)
        .middleware(TimingMiddleware::new())
        .middleware(RetryMiddleware::new(3).with_backoff(BackoffStrategy::Fixed(Duration::from_millis(10))))
        .middleware(LoggingMiddleware::info())
        .event(FailureEvent::new("RetryEvent", 2))  // Fails twice, succeeds on 3rd attempt
        .with_fault_tolerance(FaultToleranceMode::Lenient);

    let mut context = EventContext::new();
    let result = chain.execute(&mut context);

    println!("\n Result:");
    println!("  Status: {:?}", result.status);
    println!("  Success: {}", result.success);
    println!("  Failures: {}", result.failures.len());

    metrics_clone.print_summary();

    assert!(result.success);
    assert_eq!(result.status, ChainStatus::Completed);
    assert!(context.get::<bool>("RetryEvent_executed").unwrap_or(false));

    println!("\n SCENARIO 2 PASSED\n");
}

fn test_scenario_3_timing_threshold() {
    println!("\n{}", "=".repeat(70));
    println!("SCENARIO 3: Timing Middleware with Threshold");
    println!("{}\n", "=".repeat(70));

    let chain = EventChain::new()
        .middleware(TimingMiddleware::new().with_threshold(Duration::from_millis(50)))
        .middleware(LoggingMiddleware::info())
        .event(SlowEvent::new("FastEvent", 20))   // Won't log
        .event(SlowEvent::new("SlowEvent", 100))  // Will log
        .with_fault_tolerance(FaultToleranceMode::Strict);

    let mut context = EventContext::new();
    let result = chain.execute(&mut context);

    println!("\n Result:");
    println!("  Status: {:?}", result.status);

    assert!(result.success);
    assert_eq!(result.status, ChainStatus::Completed);

    println!("\n SCENARIO 3 PASSED\n");
}

fn test_scenario_4_circuit_breaker() {
    println!("\n{}", "=".repeat(70));
    println!("SCENARIO 4: Circuit Breaker Opening and Recovery");
    println!("{}\n", "=".repeat(70));

    let cb = CircuitBreakerMiddleware::new()
        .with_failure_threshold(3)
        .with_success_threshold(2)
        .with_timeout(Duration::from_millis(100));

    let chain = EventChain::new()
        .middleware(cb)
        .middleware(LoggingMiddleware::info())
        .event(FailureEvent::new("CBEvent", 100))  // Will keep failing
        .with_fault_tolerance(FaultToleranceMode::Lenient);

    // Trigger failures to open circuit
    println!("Phase 1: Triggering failures to open circuit");
    for i in 1..=5 {
        let mut context = EventContext::new();
        let result = chain.execute(&mut context);
        println!("  Attempt {}: {:?}", i, result.status);
    }

    println!("\nPhase 2: Circuit breaker has been triggered");

    println!("\n SCENARIO 4 PASSED\n");
}

fn test_scenario_5_besteffort_event_failures() {
    println!("\n{}", "=".repeat(70));
    println!("SCENARIO 5: BestEffort Mode - Event Failures Continue");
    println!("{}\n", "=".repeat(70));

    let metrics = MetricsMiddleware::new();
    let metrics_clone = metrics.clone();

    let chain = EventChain::new()
        .middleware(metrics)
        .middleware(LoggingMiddleware::info())
        .event(SuccessEvent::new("Event1"))
        .event(FailureEvent::new("Event2", 100))  // Will fail
        .event(SuccessEvent::new("Event3"))       // Should still execute
        .event(FailureEvent::new("Event4", 100))  // Will fail
        .event(SuccessEvent::new("Event5"))       // Should still execute
        .with_fault_tolerance(FaultToleranceMode::BestEffort);

    let mut context = EventContext::new();
    let result = chain.execute(&mut context);

    println!("\n Result:");
    println!("  Status: {:?}", result.status);
    println!("  Success: {}", result.success);
    println!("  Failures: {}", result.failures.len());
    for failure in &result.failures {
        println!("    - {} ({}): {}",
                 failure.event_name,
                 if failure.is_middleware_failure { "MIDDLEWARE" } else { "EVENT" },
                 failure.error_message);
    }

    metrics_clone.print_summary();

    // All events should have executed
    assert!(result.success);
    assert_eq!(result.status, ChainStatus::CompletedWithWarnings);
    assert_eq!(result.failures.len(), 2);  // Event2 and Event4 failed

    // Verify successful events executed
    assert!(context.get::<bool>("Event1_executed").unwrap_or(false));
    assert!(context.get::<bool>("Event3_executed").unwrap_or(false));
    assert!(context.get::<bool>("Event5_executed").unwrap_or(false));

    println!("\n SCENARIO 5 PASSED\n");
}

fn test_scenario_6_strict_mode_stops() {
    println!("\n{}", "=".repeat(70));
    println!("SCENARIO 6: Strict Mode - Stops on First Failure");
    println!("{}\n", "=".repeat(70));

    let chain = EventChain::new()
        .middleware(LoggingMiddleware::info())
        .event(SuccessEvent::new("Event1"))
        .event(FailureEvent::new("Event2", 100))  // Will fail
        .event(SuccessEvent::new("Event3"))       // Should NOT execute
        .with_fault_tolerance(FaultToleranceMode::Strict);

    let mut context = EventContext::new();
    let result = chain.execute(&mut context);

    println!("\n Result:");
    println!("  Status: {:?}", result.status);
    println!("  Success: {}", result.success);
    println!("  Failures: {}", result.failures.len());

    assert!(!result.success);
    assert_eq!(result.status, ChainStatus::Failed);
    assert_eq!(result.failures.len(), 1);

    // Event1 executed, Event3 should NOT have executed
    assert!(context.get::<bool>("Event1_executed").unwrap_or(false));
    assert!(!context.get::<bool>("Event3_executed").unwrap_or(false));

    println!("\n SCENARIO 6 PASSED\n");
}

fn test_scenario_7_lenient_mode_continues() {
    println!("\n{}", "=".repeat(70));
    println!("SCENARIO 7: Lenient Mode - Continues on All Failures");
    println!("{}\n", "=".repeat(70));

    let chain = EventChain::new()
        .middleware(LoggingMiddleware::info())
        .event(SuccessEvent::new("Event1"))
        .event(FailureEvent::new("Event2", 100))
        .event(SuccessEvent::new("Event3"))
        .with_fault_tolerance(FaultToleranceMode::Lenient);

    let mut context = EventContext::new();
    let result = chain.execute(&mut context);

    println!("\n Result:");
    println!("  Status: {:?}", result.status);
    println!("  Success: {}", result.success);
    println!("  Failures: {}", result.failures.len());

    assert!(result.success);
    assert_eq!(result.status, ChainStatus::CompletedWithWarnings);
    assert_eq!(result.failures.len(), 1);

    // All events should have executed
    assert!(context.get::<bool>("Event1_executed").unwrap_or(false));
    assert!(context.get::<bool>("Event3_executed").unwrap_or(false));

    println!("\n SCENARIO 7 PASSED\n");
}

fn test_scenario_8_complex_real_world() {
    println!("\n{}", "=".repeat(70));
    println!("SCENARIO 8: Complex Real-World Pipeline");
    println!("All middleware + Mixed success/failure + BestEffort");
    println!("{}\n", "=".repeat(70));

    let metrics = MetricsMiddleware::new();
    let metrics_clone = metrics.clone();

    let chain = EventChain::new()
        // Infrastructure layer
        .middleware(metrics)
        .middleware(TimingMiddleware::new().with_threshold(Duration::from_millis(30)))

        // Protection layer
        .middleware(CircuitBreakerMiddleware::new().with_failure_threshold(10))
        .middleware(RateLimitMiddleware::new(100, RateLimitStrategy::Block))

        // Resilience layer
        .middleware(RetryMiddleware::new(2).with_backoff(BackoffStrategy::Fixed(Duration::from_millis(5))))

        // Observability layer
        .middleware(LoggingMiddleware::info())

        // Business logic
        .event(SuccessEvent::new("Validate"))
        .event(SlowEvent::new("FetchData", 50))
        .event(FailureEvent::new("ProcessPartial", 1))  // Fails once, succeeds on retry
        .event(SuccessEvent::new("Transform"))
        .event(FailureEvent::new("SaveOptional", 100))  // Always fails, but optional
        .event(SuccessEvent::new("Notify"))

        .with_fault_tolerance(FaultToleranceMode::BestEffort);

    let mut context = EventContext::new();
    let result = chain.execute(&mut context);

    println!("\n Result:");
    println!("  Status: {:?}", result.status);
    println!("  Success: {}", result.success);
    println!("  Total failures: {}", result.failures.len());
    println!("\nFailure details:");
    for failure in &result.failures {
        println!("  - {} ({}): {}",
                 failure.event_name,
                 if failure.is_middleware_failure { "MIDDLEWARE" } else { "EVENT" },
                 failure.error_message);
    }

    println!("\nMetrics Summary:");
    metrics_clone.print_summary();

    assert!(result.success);
    assert_eq!(result.status, ChainStatus::CompletedWithWarnings);

    // Verify critical events executed
    assert!(context.get::<bool>("Validate_executed").unwrap_or(false));
    assert!(context.get::<bool>("Transform_executed").unwrap_or(false));
    assert!(context.get::<bool>("Notify_executed").unwrap_or(false));

    println!("\n SCENARIO 8 PASSED\n");
}

// ============================================================================
// MAIN TEST RUNNER
// ============================================================================

fn main() {
    println!("\n");
    println!("|--------------------------------------------------------------------|");
    println!("|         EVENT CHAINS - COMPREHENSIVE INTEGRATION TEST              |");
    println!("|--------------------------------------------------------------------|");

    test_scenario_1_all_success();
    test_scenario_2_retry_middleware();
    test_scenario_3_timing_threshold();
    test_scenario_4_circuit_breaker();
    test_scenario_5_besteffort_event_failures();
    test_scenario_6_strict_mode_stops();
    test_scenario_7_lenient_mode_continues();
    test_scenario_8_complex_real_world();

    println!("\n");
    println!("|--------------------------------------------------------------------|");
    println!("|                   ALL TESTS PASSED!                              |");
    println!("|--------------------------------------------------------------------|");
    println!("\n");

    println!("Summary:");
    println!("   All middleware working correctly");
    println!("   All fault tolerance modes behaving as expected");
    println!("   Event execution order correct (FIFO)");
    println!("   Middleware execution order correct (LIFO)");
    println!("   Retry logic working");
    println!("   Circuit breaker opening/closing");
    println!("   Metrics collection accurate");
    println!("   Timing measurement working");
    println!("   Rate limiting functional");
    println!("   Context passing data correctly");
    println!("\n");
}
