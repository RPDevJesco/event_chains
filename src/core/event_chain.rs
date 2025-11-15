use std::fmt;
use crate::core::chain_result::{ChainResult, ChainStatus};
use crate::core::event_context::EventContext;
use crate::core::event_failure::EventFailure;
use crate::core::event_result::EventResult;
use crate::core::fault_tolerance_mode::FaultToleranceMode;
use crate::events::chainable_event::ChainableEvent;
use crate::events::event_middleware::EventMiddleware;

/// Main EventChain orchestrator
///
/// Manages and executes a pipeline of events with optional middleware.
///
/// # Event Execution Order
///
/// * **Events**: Execute in FIFO order (first added → first executed)
/// * **Middleware**: Execute in LIFO order (last added → first executed)
///
/// # Fault Tolerance Modes
///
/// * **Strict**: Stop on any failure (event or middleware)
/// * **Lenient**: Continue on all failures, collect for review
/// * **BestEffort**: Continue on event failures, but stop on middleware failures
///
/// # Example
///
/// ```ignore
/// use event_chains_project::core::EventChain::EventChain;
/// use event_chains_project::core::EventContext::EventContext;
/// use event_chains_project::core::FaultToleranceMode::FaultToleranceMode;
///
/// let chain = EventChain::new()
///     .middleware(LoggingMiddleware)    // Outer layer
///     .middleware(TimingMiddleware)     // Inner layer
///     .event(ValidateEvent)             // Runs 1st
///     .event(ProcessEvent)              // Runs 2nd
///     .with_fault_tolerance(FaultToleranceMode::BestEffort);
///
/// let mut context = EventContext::new();
/// let result = chain.execute(&mut context);
/// ```
pub struct EventChain {
    events: Vec<Box<dyn ChainableEvent>>,
    middlewares: Vec<Box<dyn EventMiddleware>>,
    fault_tolerance: FaultToleranceMode,
}

impl EventChain {
    /// Create a new empty event chain with strict fault tolerance
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            middlewares: Vec::new(),
            fault_tolerance: FaultToleranceMode::Strict,
        }
    }

    /// Set the fault tolerance mode for this chain
    ///
    /// # Modes
    ///
    /// * [`FaultToleranceMode::Strict`] - Stop on any failure (default)
    /// * [`FaultToleranceMode::Lenient`] - Continue on all failures, collect for review
    /// * [`FaultToleranceMode::BestEffort`] - Continue on event failures, stop on middleware failures
    ///
    /// # BestEffort Mode Details
    ///
    /// BestEffort treats event and middleware failures differently:
    /// - **Event failures**: Continue execution (best effort to complete all operations)
    /// - **Middleware failures**: Stop immediately (infrastructure must be reliable)
    ///
    /// This is useful for cleanup operations where you want to attempt all cleanup steps,
    /// but require infrastructure (logging, auditing, transactions) to work correctly.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Cleanup with BestEffort: Try all cleanup steps, but audit must work
    /// let chain = EventChain::new()
    ///     .middleware(AuditMiddleware)      // Must succeed
    ///     .event(CloseConnection)           // Best effort
    ///     .event(DeleteTempFiles)           // Best effort
    ///     .with_fault_tolerance(FaultToleranceMode::BestEffort);
    /// ```
    pub fn with_fault_tolerance(mut self, mode: FaultToleranceMode) -> Self {
        self.fault_tolerance = mode;
        self
    }

    /// Add an event to the chain (fluent API - consumes self)
    ///
    /// Events execute in the order they are added (FIFO).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let chain = EventChain::new()
    ///     .event(ValidateEvent)   // Executes 1st
    ///     .event(ProcessEvent)    // Executes 2nd
    ///     .event(NotifyEvent);    // Executes 3rd
    /// ```
    ///
    /// # Type Parameters
    ///
    /// * `E` - Any type implementing [`ChainableEvent`] + `'static`
    pub fn event<E: ChainableEvent + 'static>(mut self, event: E) -> Self {
        self.events.push(Box::new(event));
        self
    }

    /// Add a middleware to the chain (fluent API - consumes self)
    ///
    /// #  Execution Order: LIFO (Last In, First Out)
    ///
    /// Middlewares execute in **reverse order** of registration - the last middleware
    /// added will be the **first** to execute (outermost layer).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let chain = EventChain::new()
    ///     .middleware(AuthMiddleware)      // Executes 3rd (innermost)
    ///     .middleware(LoggingMiddleware)   // Executes 2nd
    ///     .middleware(TimingMiddleware)    // Executes 1st (outermost)
    ///     .event(MyEvent);
    /// ```
    ///
    /// **Execution flow:**
    /// ```text
    /// TimingMiddleware (before)
    ///   → LoggingMiddleware (before)
    ///     → AuthMiddleware (before)
    ///       → MyEvent.execute()
    ///     ← AuthMiddleware (after)
    ///   ← LoggingMiddleware (after)
    /// ← TimingMiddleware (after)
    /// ```
    ///
    /// This "onion" pattern means infrastructure middleware (timing, logging, metrics)
    /// should typically be added **last** so they wrap around business logic middleware.
    pub fn middleware<M: EventMiddleware + 'static>(mut self, middleware: M) -> Self {
        self.middlewares.push(Box::new(middleware));
        self
    }

    /// Legacy method for adding boxed events (mutable reference API)
    ///
    /// Events execute in the order they are added (FIFO).
    /// See [`event()`](Self::event) for the recommended fluent API.
    pub fn add_event(&mut self, event: Box<dyn ChainableEvent>) -> &mut Self {
        self.events.push(event);
        self
    }

    /// Legacy method for adding boxed middleware (mutable reference API)
    ///
    /// # ️ Execution Order: LIFO (Last In, First Out)
    ///
    /// Middlewares execute in **reverse order** of registration.
    /// See [`middleware()`](Self::middleware) for detailed explanation.
    pub fn use_middleware(&mut self, middleware: Box<dyn EventMiddleware>) -> &mut Self {
        self.middlewares.push(middleware);
        self
    }

    /// Execute the event chain with the provided context
    ///
    /// Events execute in registration order (FIFO), with each event wrapped
    /// by the middleware stack in reverse order (LIFO).
    ///
    /// # Fault Tolerance Behavior
    ///
    /// * **Strict**: Stops on any failure (event or middleware)
    /// * **Lenient**: Continues on all failures, collects them for review
    /// * **BestEffort**: Continues on event failures, stops on middleware failures
    ///
    /// # Returns
    ///
    /// [`ChainResult`] containing success status and any failures that occurred
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut context = EventContext::new();
    /// let result = chain.execute(&mut context);
    ///
    /// match result.status {
    ///     ChainStatus::Completed => println!("Success!"),
    ///     ChainStatus::CompletedWithWarnings => println!("Partial success"),
    ///     ChainStatus::Failed => println!("Failed"),
    /// }
    /// ```
    pub fn execute(&self, context: &mut EventContext) -> ChainResult {
        let mut failures = Vec::new();

        for event in &self.events {
            // Build middleware pipeline (LIFO - last registered executes first)
            let result = self.execute_with_middleware(event.as_ref(), context);

            if result.is_failure() {
                // Determine if this is a middleware or event failure
                let (is_middleware_failure, error_msg) = match result.get_failure_info() {
                    Some((is_mw, msg)) => (is_mw, msg.to_string()),
                    None => (false, "Unknown error".to_string()),
                };

                let failure = if is_middleware_failure {
                    EventFailure::middleware_failure(event.name().to_string(), error_msg)
                } else {
                    EventFailure::new(event.name().to_string(), error_msg)
                };

                failures.push(failure.clone());

                // Decide whether to continue based on fault tolerance mode and failure type
                match self.fault_tolerance {
                    FaultToleranceMode::Strict => {
                        // Strict: Stop on any failure
                        return ChainResult::failure(failures);
                    }
                    FaultToleranceMode::Lenient => {
                        // Lenient: Continue on all failures
                        continue;
                    }
                    FaultToleranceMode::BestEffort => {
                        if is_middleware_failure {
                            // BestEffort: Stop on middleware failures
                            return ChainResult::failure(failures);
                        } else {
                            // BestEffort: Continue on event failures
                            continue;
                        }
                    }
                }
            }
        }

        // Determine final result
        if failures.is_empty() {
            ChainResult::success()
        } else {
            ChainResult::partial_success(failures)
        }
    }

    fn execute_with_middleware(
        &self,
        event: &dyn ChainableEvent,
        context: &mut EventContext,
    ) -> EventResult<()> {
        if self.middlewares.is_empty() {
            return event.execute(context);
        }

        // Execute middleware in reverse order by recursively building the call stack
        self.execute_middleware_recursive(0, event, context)
    }

    fn execute_middleware_recursive(
        &self,
        middleware_index: usize,
        event: &dyn ChainableEvent,
        context: &mut EventContext,
    ) -> EventResult<()> {
        if middleware_index >= self.middlewares.len() {
            // Base case: execute the actual event
            return event.execute(context);
        }

        // Get the current middleware (reverse order)
        let middleware_idx = self.middlewares.len() - 1 - middleware_index;
        let middleware = &self.middlewares[middleware_idx];

        // Create a closure that calls the next middleware (or event)
        let mut next = |ctx: &mut EventContext| -> EventResult<()> {
            self.execute_middleware_recursive(middleware_index + 1, event, ctx)
        };

        // Execute this middleware with the next closure
        middleware.execute(event, context, &mut next)
    }
}

impl Default for EventChain {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ChainStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChainStatus::Completed => write!(f, "COMPLETED"),
            ChainStatus::CompletedWithWarnings => write!(f, "COMPLETED_WITH_WARNINGS"),
            ChainStatus::Failed => write!(f, "FAILED"),
        }
    }
}
