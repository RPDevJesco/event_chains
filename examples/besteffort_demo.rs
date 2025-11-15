use event_chains::core::event_chain::EventChain;
use event_chains::core::event_context::EventContext;
use event_chains::core::event_result::EventResult;
use event_chains::core::fault_tolerance_mode::FaultToleranceMode;
use event_chains::events::chainable_event::ChainableEvent;
use event_chains::events::event_middleware::EventMiddleware;

// ============================================================================
// MIDDLEWARE EXAMPLES
// ============================================================================

/// Critical audit middleware - must always succeed
struct AuditMiddleware {
    should_fail: bool,
}

impl EventMiddleware for AuditMiddleware {
    fn execute(
        &self,
        event: &dyn ChainableEvent,
        context: &mut EventContext,
        next: &mut dyn FnMut(&mut EventContext) -> EventResult<()>,
    ) -> EventResult<()> {
        // Simulate audit infrastructure failure
        if self.should_fail {
            println!("   [AUDIT] Infrastructure failure - cannot write audit log!");
            return EventResult::MiddlewareFailure(
                "Audit infrastructure unavailable".to_string()
            );
        }

        println!("   [AUDIT] Recording: {}", event.name());
        let result = next(context);
        println!("   [AUDIT] Recorded: {} → {:?}", event.name(),
                 if result.is_success() { "Success" } else { "Failed" });
        result
    }
}

/// Transaction middleware - ensures data integrity
struct TransactionMiddleware {
    should_fail: bool,
}

impl EventMiddleware for TransactionMiddleware {
    fn execute(
        &self,
        event: &dyn ChainableEvent,
        context: &mut EventContext,
        next: &mut dyn FnMut(&mut EventContext) -> EventResult<()>,
    ) -> EventResult<()> {
        if self.should_fail {
            println!("   [TRANSACTION] Cannot start transaction - DB unavailable!");
            return EventResult::MiddlewareFailure(
                "Transaction infrastructure unavailable".to_string()
            );
        }

        println!("   [TRANSACTION] Begin for: {}", event.name());
        let result = next(context);

        match &result {
            EventResult::Success(_) => println!("   [TRANSACTION] Commit"),
            _ => println!("   [TRANSACTION] Rollback"),
        }

        result
    }
}

// ============================================================================
// EVENT EXAMPLES
// ============================================================================

struct CloseConnectionEvent {
    should_fail: bool,
}

impl ChainableEvent for CloseConnectionEvent {
    fn execute(&self, _context: &mut EventContext) -> EventResult<()> {
        if self.should_fail {
            println!("    Close connection: Failed (connection already closed)");
            EventResult::Failure("Connection already closed".to_string())
        } else {
            println!("   Close connection: Success");
            EventResult::Success(())
        }
    }
    fn name(&self) -> &str { "CloseConnection" }
}

struct ReleaseResourcesEvent {
    should_fail: bool,
}

impl ChainableEvent for ReleaseResourcesEvent {
    fn execute(&self, _context: &mut EventContext) -> EventResult<()> {
        if self.should_fail {
            println!("    Release resources: Failed (some resources locked)");
            EventResult::Failure("Some resources still locked".to_string())
        } else {
            println!("   Release resources: Success");
            EventResult::Success(())
        }
    }
    fn name(&self) -> &str { "ReleaseResources" }
}

struct DeleteTempFilesEvent {
    should_fail: bool,
}

impl ChainableEvent for DeleteTempFilesEvent {
    fn execute(&self, _context: &mut EventContext) -> EventResult<()> {
        if self.should_fail {
            println!("    Delete temp files: Failed (permission denied)");
            EventResult::Failure("Permission denied on some files".to_string())
        } else {
            println!("   Delete temp files: Success");
            EventResult::Success(())
        }
    }
    fn name(&self) -> &str { "DeleteTempFiles" }
}

struct LogCleanupEvent;

impl ChainableEvent for LogCleanupEvent {
    fn execute(&self, _context: &mut EventContext) -> EventResult<()> {
        println!("   Log cleanup: Success");
        EventResult::Success(())
    }
    fn name(&self) -> &str { "LogCleanup" }
}

// ============================================================================
// MAIN DEMO
// ============================================================================

fn main() {
    println!("=== BestEffort Mode: Middleware vs Event Failures ===\n");
    println!("BestEffort Mode Behavior:");
    println!("  • Event failures → Continue execution (best effort)");
    println!("  • Middleware failures → Stop immediately (infrastructure must work)\n");

    println!("{}\n", "=".repeat(70));

    // ========================================================================
    // Test 1: Event failures in BestEffort mode
    // ========================================================================
    println!("TEST 1: Event Failures (BestEffort continues)\n");
    println!("Scenario: Cleanup with some operations failing");
    println!("Expected: All cleanup steps attempted despite failures\n");

    let chain1 = EventChain::new()
        .middleware(AuditMiddleware { should_fail: false })
        .middleware(TransactionMiddleware { should_fail: false })
        .event(CloseConnectionEvent { should_fail: true })     // FAILS
        .event(ReleaseResourcesEvent { should_fail: true })    // FAILS
        .event(DeleteTempFilesEvent { should_fail: false })    // Success
        .event(LogCleanupEvent)                                // Success
        .with_fault_tolerance(FaultToleranceMode::BestEffort);

    let mut context1 = EventContext::new();
    let result1 = chain1.execute(&mut context1);

    println!("\n Result:");
    println!("  Status: {:?}", result1.status);
    println!("  Success: {}", result1.success);
    println!("  Total failures: {}", result1.failures.len());
    for (i, failure) in result1.failures.iter().enumerate() {
        println!("    {}. {} ({}): {}",
                 i + 1,
                 failure.event_name,
                 if failure.is_middleware_failure { "MIDDLEWARE" } else { "EVENT" },
                 failure.error_message);
    }
    println!("\n Result: All 4 events executed (2 failed, 2 succeeded)");

    println!("\n{}\n", "=".repeat(70));

    // ========================================================================
    // Test 2: Middleware failure in BestEffort mode
    // ========================================================================
    println!("TEST 2: Middleware Failure (BestEffort stops)\n");
    println!("Scenario: Audit middleware fails");
    println!("Expected: Execution stops immediately (infrastructure critical)\n");

    let chain2 = EventChain::new()
        .middleware(AuditMiddleware { should_fail: true })  // INFRASTRUCTURE FAILS
        .middleware(TransactionMiddleware { should_fail: false })
        .event(CloseConnectionEvent { should_fail: false })
        .event(ReleaseResourcesEvent { should_fail: false })
        .event(DeleteTempFilesEvent { should_fail: false })
        .event(LogCleanupEvent)
        .with_fault_tolerance(FaultToleranceMode::BestEffort);

    let mut context2 = EventContext::new();
    let result2 = chain2.execute(&mut context2);

    println!("\n Result:");
    println!("  Status: {:?}", result2.status);
    println!("  Success: {}", result2.success);
    println!("  Total failures: {}", result2.failures.len());
    for (i, failure) in result2.failures.iter().enumerate() {
        println!("    {}. {} ({}): {}",
                 i + 1,
                 failure.event_name,
                 if failure.is_middleware_failure { "MIDDLEWARE" } else { "EVENT" },
                 failure.error_message);
    }
    println!("\n Result: Stopped at first event (middleware failure is critical)");

    println!("\n{}\n", "=".repeat(70));

    // ========================================================================
    // Test 3: Compare with Lenient mode
    // ========================================================================
    println!("TEST 3: Lenient Mode (continues on all failures)\n");
    println!("Scenario: Same as Test 2, but Lenient mode");
    println!("Expected: All events execute despite middleware failure\n");

    let chain3 = EventChain::new()
        .middleware(AuditMiddleware { should_fail: true })  // FAILS but Lenient continues
        .middleware(TransactionMiddleware { should_fail: false })
        .event(CloseConnectionEvent { should_fail: false })
        .event(ReleaseResourcesEvent { should_fail: false })
        .event(DeleteTempFilesEvent { should_fail: false })
        .event(LogCleanupEvent)
        .with_fault_tolerance(FaultToleranceMode::Lenient);

    let mut context3 = EventContext::new();
    let result3 = chain3.execute(&mut context3);

    println!("\n Result:");
    println!("  Status: {:?}", result3.status);
    println!("  Success: {}", result3.success);
    println!("  Total failures: {}", result3.failures.len());
    for (i, failure) in result3.failures.iter().enumerate() {
        println!("    {}. {} ({}): {}",
                 i + 1,
                 failure.event_name,
                 if failure.is_middleware_failure { "MIDDLEWARE" } else { "EVENT" },
                 failure.error_message);
    }
    println!("\n  Result: All 4 events executed (Lenient ignores all failures)");

    println!("\n{}\n", "=".repeat(70));

    // ========================================================================
    // Summary
    // ========================================================================
    println!("\n SUMMARY: Fault Tolerance Mode Comparison\n");
    println!("┌──────────────┬─────────────────┬───────────────────┬──────────────────┐");
    println!("│ Mode         │ Event Failure   │ Middleware Fail   │ Best For         │");
    println!("├──────────────┼─────────────────┼───────────────────┼──────────────────┤");
    println!("│ Strict       │ Stop            │ Stop              │ Critical ops     │");
    println!("│ Lenient      │ Continue        │ Continue          │ Data collection  │");
    println!("│ BestEffort   │ Continue        │ STOP              │ Cleanup + audit  │");
    println!("└──────────────┴─────────────────┴───────────────────┴──────────────────┘");

    println!("\n Key Insight:");
    println!("  BestEffort = \"Try all business operations, but infrastructure MUST work\"");
    println!("\n  Use BestEffort when:");
    println!("     You want to attempt all cleanup/recovery operations");
    println!("     But you NEED audit trails, logging, or transactions to work");
    println!("     Infrastructure failures indicate a serious problem");
}
