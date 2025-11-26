[![Crates.io](https://img.shields.io/crates/v/event_chains.svg)](https://crates.io/crates/event_chains)
[![Documentation](https://docs.rs/event_chains/badge.svg)](https://docs.rs/event_chains)
# Event Chains

A flexible and robust event processing pipeline library for Rust with middleware support and configurable fault tolerance.

## Features

-  **Chainable Events** - Build event pipelines with a fluent API
-  **Middleware Support** - Wrap events with reusable middleware (logging, timing, auth, etc.)
-  **Fault Tolerance** - Configure how your chain handles failures (strict, lenient, best-effort)
-  **Flexible Context** - Pass typed data between events using a type-safe context
-  **Zero Cost Abstractions** - Efficient execution with minimal overhead

## Installation

Add this to your `Cargo.toml`:
```toml
[dependencies]
event_chains = "0.2.1"
```

## Quick Start
```rust
use event_chains::core::event_chain::EventChain;
use event_chains::core::event_context::EventContext;
use event_chains::core::event_result::EventResult;
use event_chains::core::fault_tolerance_mode::FaultToleranceMode;
use event_chains::events::chainable_event::ChainableEvent;

// Define your event
struct ValidateUserEvent;

impl ChainableEvent for ValidateUserEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        // Your validation logic here
        context.set("user_valid", true);
        EventResult::Success(())
    }

    fn name(&self) -> &str {
        "ValidateUser"
    }
}

// Build and execute the chain
fn main() {
    let chain = EventChain::new()
        .event(ValidateUserEvent)
        .with_fault_tolerance(FaultToleranceMode::Lenient);

    let mut context = EventContext::new();
    let result = chain.execute(&mut context);

    match result.status {
        ChainStatus::Completed => println!("✓ Success!"),
        ChainStatus::CompletedWithWarnings => println!("⚠ Partial success"),
        ChainStatus::Failed => println!("✗ Failed"),
    }
}
```

## Core Concepts

### Events

Events are the building blocks of your pipeline. They execute in FIFO order (first added → first executed).
```rust
struct ProcessPaymentEvent;

impl ChainableEvent for ProcessPaymentEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        let amount: f64 = context.get("amount").unwrap_or(0.0);
        
        if amount > 0.0 {
            // Process payment
            EventResult::Success(())
        } else {
            EventResult::Failure("Invalid amount".to_string())
        }
    }

    fn name(&self) -> &str {
        "ProcessPayment"
    }
}
```

### Middleware

Middleware wraps around events to add cross-cutting concerns. They execute in LIFO order (last added → first executed).
```rust
use event_chains::events::event_middleware::EventMiddleware;

struct LoggingMiddleware;

impl EventMiddleware for LoggingMiddleware {
    fn execute(
        &self,
        event: &dyn ChainableEvent,
        context: &mut EventContext,
        next: &mut dyn FnMut(&mut EventContext) -> EventResult<()>,
    ) -> EventResult<()> {
        println!("→ Executing: {}", event.name());
        let result = next(context);
        println!("← Finished: {}", event.name());
        result
    }
}
```

### Execution Order
```rust
let chain = EventChain::new()
    .middleware(TimingMiddleware)    // Executes 1st (outermost)
    .middleware(LoggingMiddleware)   // Executes 2nd
    .event(ValidateEvent)            // Executes 3rd
    .event(ProcessEvent);            // Executes 4th
```

**Flow:**
```
TimingMiddleware (before)
  → LoggingMiddleware (before)
    → ValidateEvent.execute()
  ← LoggingMiddleware (after)
← TimingMiddleware (after)
TimingMiddleware (before)
  → LoggingMiddleware (before)
    → ProcessEvent.execute()
  ← LoggingMiddleware (after)
← TimingMiddleware (after)
```

### Context

Share data between events using the type-safe context:
```rust
let mut context = EventContext::new();

// Set values
context.set("user_id", 42u64);
context.set("email", "user@examples.com".to_string());

// Get values
let user_id: u64 = context.get("user_id").unwrap();
let email: String = context.get("email").unwrap();

// Check existence
if context.has("user_id") {
    // ...
}
```

### Fault Tolerance

Configure how your chain handles failures:
```rust
// Strict: Stop execution immediately on first failure (default)
// Semantic Intent: Ensure critical tasks halt when any failure appears.
let chain = EventChain::new()
    .event(Event1)
    .event(Event2)
    .with_fault_tolerance(FaultToleranceMode::Strict);

// Lenient: Continue execution, collect failures for later review
// Semantic Intent: Handle non-critical failures where some operations can fail without compromising the overall goal
let chain = EventChain::new()
    .event(Event1)
    .event(Event2)
    .with_fault_tolerance(FaultToleranceMode::Lenient);

// BestEffort: Attempt all events regardless of failures, maximum resilience
// Semantic Intent: Ensure all cleanup/recovery operations are attempted, even if some fail
let chain = EventChain::new()
    .event(Event1)
    .event(Event2)
    .with_fault_tolerance(FaultToleranceMode::BestEffort);
```

## Complete Example
```rust
use event_chains::core::event_chain::EventChain;
use event_chains::core::event_context::EventContext;
use event_chains::core::event_result::EventResult;
use event_chains::core::fault_tolerance_mode::FaultToleranceMode;
use event_chains::events::chainable_event::ChainableEvent;
use event_chains::events::event_middleware::EventMiddleware;

// Events
struct ValidateOrderEvent;
struct CalculateTotalEvent;
struct ProcessPaymentEvent;

impl ChainableEvent for ValidateOrderEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        let items: Vec<String> = context.get("items").unwrap_or_default();
        if items.is_empty() {
            return EventResult::Failure("No items in order".to_string());
        }
        EventResult::Success(())
    }
    fn name(&self) -> &str { "ValidateOrder" }
}

impl ChainableEvent for CalculateTotalEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        context.set("total", 99.99);
        EventResult::Success(())
    }
    fn name(&self) -> &str { "CalculateTotal" }
}

impl ChainableEvent for ProcessPaymentEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        let total: f64 = context.get("total").unwrap_or(0.0);
        println!("Processing payment: ${}", total);
        EventResult::Success(())
    }
    fn name(&self) -> &str { "ProcessPayment" }
}

// Middleware
struct TimingMiddleware;

impl EventMiddleware for TimingMiddleware {
    fn execute(
        &self,
        event: &dyn ChainableEvent,
        context: &mut EventContext,
        next: &mut dyn FnMut(&mut EventContext) -> EventResult<()>,
    ) -> EventResult<()> {
        let start = std::time::Instant::now();
        let result = next(context);
        println!("{} took {:?}", event.name(), start.elapsed());
        result
    }
}

fn main() {
    let chain = EventChain::new()
        .middleware(TimingMiddleware)
        .event(ValidateOrderEvent)
        .event(CalculateTotalEvent)
        .event(ProcessPaymentEvent)
        .with_fault_tolerance(FaultToleranceMode::Strict);

    let mut context = EventContext::new();
    context.set("items", vec!["Widget".to_string(), "Gadget".to_string()]);

    let result = chain.execute(&mut context);
    println!("Result: {:?}", result.status);
}
```

# Built-in Middleware
- **LoggingMiddleware** - Logs event execution with configurable log levels.
- **TimingMiddleware** - Measures and logs event execution time.
- **RetryMiddleware** - Retries failed events with configurable backoff strategies.
- **MetricsMiddleware** - Collects execution statistics for events.
- **RateLimitMiddleware** - Enforces rate limits on event execution using token bucket algorithm.
- **CircuitBreakerMiddleware** - Implements the circuit breaker pattern to prevent cascading failures.

## Combining Middleware

Middleware can be stacked together. Remember: **LIFO execution order** (last added executes first).

```rust
use event_chains::middleware::*;

let metrics = metrics::MetricsMiddleware::new();

let chain = EventChain::new()
    .middleware(metrics.clone())                           // 1st: Collect metrics
    .middleware(timing::TimingMiddleware::new())           // 2nd: Measure time
    .middleware(
        circuit_breaker::CircuitBreakerMiddleware::new()   // 3rd: Circuit breaker
            .with_failure_threshold(5)
    )
    .middleware(
        retry::RetryMiddleware::exponential(              // 4th: Retry logic
            3,
            Duration::from_millis(100),
            Duration::from_secs(2),
        )
    )
    .middleware(logging::LoggingMiddleware::info())        // 5th: Logging (innermost)
    .event(MyEvent);

// Execution flow:
// Metrics → Timing → CircuitBreaker → Retry → Logging → Event
```

## Thread Safety

All middleware is thread-safe and can be shared across threads using `Arc`:

## Use Cases (Non-exhaustive list)

- **Request Processing Pipelines** - Validation → Authentication → Business Logic → Response
- **Data Processing** - Extract → Transform → Validate → Load
- **Workflow Orchestration** - Multi-step business processes with rollback support
- **Event Sourcing** - Chain of domain events with middleware for logging/persistence
- **Plugin Systems** - Extensible processing with middleware hooks

## License

Licensed under:

- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
