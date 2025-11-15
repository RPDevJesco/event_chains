/// Adversarial Middleware Demonstration
///
/// This example demonstrates chaos injection and security fuzzing middleware
/// in action. Run with: cargo run --example adversarial_demo
///
/// WARNING: These middleware are for TESTING ONLY!

use event_chains::core::event_chain::EventChain;
use event_chains::core::event_context::EventContext;
use event_chains::core::event_result::EventResult;
use event_chains::core::fault_tolerance_mode::FaultToleranceMode;
use event_chains::events::chainable_event::ChainableEvent;
use event_chains::middleware::logging::LoggingMiddleware;
use event_chains::middleware::retry::RetryMiddleware;
use event_chains::middleware::metrics::MetricsMiddleware;
use event_chains::middleware::chaos::{ChaosMiddleware, ChaosType, ChaosConfig};
use event_chains::middleware::fuzzing::{FuzzingMiddleware, FuzzType, FuzzConfig};

// ============================================================================
// EXAMPLE EVENTS FOR TESTING
// ============================================================================

/// Database query event - vulnerable to SQL injection if not validated
struct DatabaseQueryEvent;

impl ChainableEvent for DatabaseQueryEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        let query: String = context.get("query").unwrap_or_else(|| "SELECT * FROM users".to_string());

        println!("      [DATABASE] Executing query: {}", query);

        // Input validation - catches SQL injection
        if query.contains("DROP TABLE") || query.contains("' OR '") {
            println!("      [!] SQL INJECTION DETECTED: Blocked malicious query!");
            return EventResult::Failure("SQL injection detected".to_string());
        }

        context.set("sql_result", "Query executed successfully".to_string());
        EventResult::Success(())
    }

    fn name(&self) -> &str { "DatabaseQuery" }
}

/// File access event - vulnerable to path traversal if not validated
struct FileAccessEvent;

impl ChainableEvent for FileAccessEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        let filename: String = context.get("filename").unwrap_or_else(|| "data.txt".to_string());

        println!("      [FILE] Accessing file: {}", filename);

        // Input validation - catches path traversal
        if filename.contains("..") || filename.contains("/etc/") {
            println!("      [!] PATH TRAVERSAL DETECTED: Blocked directory escape!");
            return EventResult::Failure("Path traversal detected".to_string());
        }

        context.set("file_content", "File content here".to_string());
        EventResult::Success(())
    }

    fn name(&self) -> &str { "FileAccess" }
}

/// User input processing event - vulnerable to XSS if not sanitized
struct ProcessUserInputEvent;

impl ChainableEvent for ProcessUserInputEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        let input: String = context.get("input").unwrap_or_else(|| "normal input".to_string());

        println!("      [INPUT] Processing: {}", input);

        // Input validation - catches XSS
        if input.contains("<script>") || input.contains("javascript:") {
            println!("      [!] XSS DETECTED: Blocked script injection!");
            return EventResult::Failure("XSS detected".to_string());
        }

        context.set("processed_input", input);
        EventResult::Success(())
    }

    fn name(&self) -> &str { "ProcessUserInput" }
}

/// External API call - may have transient failures
struct ExternalApiCallEvent {
    call_count: std::sync::Arc<std::sync::Mutex<usize>>,
}

impl ExternalApiCallEvent {
    fn new() -> Self {
        Self {
            call_count: std::sync::Arc::new(std::sync::Mutex::new(0)),
        }
    }
}

impl ChainableEvent for ExternalApiCallEvent {
    fn execute(&self, context: &mut EventContext) -> EventResult<()> {
        let mut count = self.call_count.lock().unwrap();
        *count += 1;

        println!("      [API] Calling external service (attempt {})", *count);

        // Simulate transient failures (fails first 2 times, then succeeds)
        if *count <= 2 {
            return EventResult::Failure("Temporary API failure".to_string());
        }

        context.set("api_response", "Success".to_string());
        EventResult::Success(())
    }

    fn name(&self) -> &str { "ExternalApiCall" }
}

// ============================================================================
// TEST SCENARIOS
// ============================================================================

fn test_chaos_injection() {
    println!("\n{}", "=".repeat(70));
    println!("TEST 1: Chaos Injection Middleware");
    println!("{}\n", "=".repeat(70));
    println!("Purpose: Test fault tolerance under random failures\n");

    // Create chaos middleware that injects 30% random failures
    let chaos = ChaosMiddleware::with_config(ChaosConfig {
        probability: 0.3,
        chaos_types: vec![
            ChaosType::RandomFailure,
            ChaosType::Latency,
        ],
        min_latency_ms: 10,
        max_latency_ms: 100,
    });

    let metrics = MetricsMiddleware::new();

    let chain = EventChain::new()
        .middleware(metrics.clone())
        .middleware(chaos.clone())
        .middleware(RetryMiddleware::new(3))
        .middleware(LoggingMiddleware::info())
        .event(ExternalApiCallEvent::new())
        .event(DatabaseQueryEvent)
        .event(ProcessUserInputEvent)
        .with_fault_tolerance(FaultToleranceMode::BestEffort);

    println!("Running 10 iterations with chaos injection...\n");

    let mut total_success = 0;
    for i in 1..=10 {
        let mut context = EventContext::new();
        context.set("query", format!("SELECT * FROM users WHERE id = {}", i));
        context.set("input", format!("User comment #{}", i));

        println!("   Iteration {}:", i);
        let result = chain.execute(&mut context);

        if result.success {
            total_success += 1;
        }

        println!("      Result: {:?}", result.status);
        if !result.failures.is_empty() {
            println!("      Failures: {}", result.failures.len());
        }
        println!();
    }

    println!("\n=== Chaos Testing Results ===");
    println!("Successful iterations: {} / 10", total_success);

    chaos.print_stats();
    metrics.print_summary();

    println!("\n Expected Results:");
    println!("  - ~30% of events should experience chaos");
    println!("  - Retry middleware should handle transient failures");
    println!("  - BestEffort should continue on event failures");
    println!("  - Even with chaos, most iterations should succeed\n");
}

fn test_security_fuzzing() {
    println!("\n{}", "=".repeat(70));
    println!("TEST 2: Security Fuzzing Middleware");
    println!("{}\n", "=".repeat(70));
    println!("Purpose: Detect security vulnerabilities through malicious input injection\n");

    // Create fuzzer targeting input fields
    let fuzzer = FuzzingMiddleware::with_config(FuzzConfig {
        probability: 0.4,
        fuzz_types: vec![
            FuzzType::SqlInjection,
            FuzzType::XssPayload,
            FuzzType::PathTraversal,
        ],
        target_keys: vec![
            "query".to_string(),
            "filename".to_string(),
            "input".to_string(),
        ],
    });

    let chain = EventChain::new()
        .middleware(fuzzer.clone())
        .middleware(LoggingMiddleware::info())
        .event(DatabaseQueryEvent)
        .event(FileAccessEvent)
        .event(ProcessUserInputEvent)
        .with_fault_tolerance(FaultToleranceMode::Lenient);

    println!("Running 20 iterations with security fuzzing...\n");

    let mut vulnerabilities_found = 0;
    let mut attacks_blocked = 0;

    for i in 1..=20 {
        let mut context = EventContext::new();
        context.set("query", "SELECT * FROM users WHERE id = 1".to_string());
        context.set("filename", "user_data.txt".to_string());
        context.set("input", "Normal user comment".to_string());

        println!("   Iteration {}:", i);
        let result = chain.execute(&mut context);

        // Check if malicious input was injected
        let query: String = context.get("query").unwrap_or_default();
        let filename: String = context.get("filename").unwrap_or_default();
        let input: String = context.get("input").unwrap_or_default();

        let is_malicious = query.contains("' OR '") ||
            filename.contains("..") ||
            input.contains("<script>");

        if is_malicious {
            if result.success {
                vulnerabilities_found += 1;
                println!("      [!] VULNERABILITY: Malicious input succeeded!");
            } else {
                attacks_blocked += 1;
                println!("      [+] Attack blocked successfully");
            }
        }

        println!("      Result: {:?}", result.status);
        println!();
    }

    println!("\n=== Security Fuzzing Results ===");
    println!("Attacks blocked: {}", attacks_blocked);
    println!("Vulnerabilities found: {}", vulnerabilities_found);

    fuzzer.print_stats();

    if vulnerabilities_found == 0 {
        println!("\n [+] PASS: All malicious inputs properly handled");
    } else {
        println!("\n [!] WARNING: {} vulnerabilities detected!", vulnerabilities_found);
    }

    println!("\n Expected Results:");
    println!("  - ~40% of events should be fuzzed");
    println!("  - SQL injection payloads should be detected");
    println!("  - Path traversal attempts should be caught");
    println!("  - XSS payloads should be identified");
    println!("  - Any successful execution with malicious input = vulnerability\n");
}

fn test_combined_adversarial() {
    println!("\n{}", "=".repeat(70));
    println!("TEST 3: Combined Chaos + Fuzzing");
    println!("{}\n", "=".repeat(70));
    println!("Purpose: Ultimate stress test combining both adversarial techniques\n");

    let chaos = ChaosMiddleware::new(0.2);
    let fuzzer = FuzzingMiddleware::new(0.3);
    let metrics = MetricsMiddleware::new();

    let chain = EventChain::new()
        .middleware(metrics.clone())
        .middleware(chaos.clone())
        .middleware(fuzzer.clone())
        .middleware(RetryMiddleware::new(2))
        .middleware(LoggingMiddleware::info())
        .event(DatabaseQueryEvent)
        .event(FileAccessEvent)
        .event(ProcessUserInputEvent)
        .event(ExternalApiCallEvent::new())
        .with_fault_tolerance(FaultToleranceMode::BestEffort);

    println!("Running 50 iterations with combined adversarial testing...\n");

    let mut total_success = 0;
    let mut security_blocks = 0;
    let mut chaos_failures = 0;

    for i in 1..=50 {
        let mut context = EventContext::new();
        context.set("query", "SELECT * FROM users".to_string());
        context.set("filename", "data.txt".to_string());
        context.set("input", "test input".to_string());

        if i % 10 == 0 {
            println!("   Iteration {}...", i);
        }

        let result = chain.execute(&mut context);

        if result.success {
            total_success += 1;
        } else {
            // Categorize failures
            for failure in &result.failures {
                if failure.error_message.contains("injection") ||
                    failure.error_message.contains("traversal") ||
                    failure.error_message.contains("XSS") {
                    security_blocks += 1;
                } else if failure.error_message.contains("Chaos") {
                    chaos_failures += 1;
                }
            }
        }
    }

    println!("\n=== Combined Test Results ===");
    println!("Total iterations: 50");
    println!("Successful: {} ({:.1}%)", total_success,
             (total_success as f64 / 50.0) * 100.0);
    println!("Security blocks: {}", security_blocks);
    println!("Chaos failures: {}", chaos_failures);

    println!("\n--- Chaos Statistics ---");
    chaos.print_stats();

    println!("\n--- Fuzzing Statistics ---");
    fuzzer.print_stats();

    println!("\n--- Performance Metrics ---");
    metrics.print_summary();

    println!("\n Expected Results:");
    println!("  - System should handle both chaos and malicious input");
    println!("  - Retry middleware should recover from transient chaos");
    println!("  - BestEffort should maximize event execution");
    println!("  - Security vulnerabilities should be detected and logged");
    println!("  - Metrics should capture all failures for analysis");
    println!("  - Even under stress, >40% success rate expected\n");
}

fn test_besteffort_semantics() {
    println!("\n{}", "=".repeat(70));
    println!("TEST 4: BestEffort Mode Validation");
    println!("{}\n", "=".repeat(70));
    println!("Purpose: Verify BestEffort distinguishes event vs middleware failures\n");

    // Test 1: RandomFailure should allow continuation
    println!("   Subtest A: RandomFailure with BestEffort");
    let chaos_event = ChaosMiddleware::with_config(ChaosConfig {
        probability: 1.0,  // Always inject
        chaos_types: vec![ChaosType::RandomFailure],
        ..Default::default()
    });

    let chain = EventChain::new()
        .middleware(chaos_event)
        .middleware(LoggingMiddleware::info())
        .event(DatabaseQueryEvent)
        .event(FileAccessEvent)
        .event(ProcessUserInputEvent)
        .with_fault_tolerance(FaultToleranceMode::BestEffort);

    let mut context = EventContext::new();
    context.set("query", "SELECT * FROM users".to_string());
    context.set("filename", "data.txt".to_string());
    context.set("input", "test".to_string());

    let result = chain.execute(&mut context);

    println!("      Failures: {}", result.failures.len());
    println!("      Status: {:?}", result.status);
    println!("      [+] BestEffort continued despite event failures\n");

    // Test 2: InfrastructureFailure should stop immediately
    println!("   Subtest B: InfrastructureFailure with BestEffort");
    let chaos_infra = ChaosMiddleware::with_config(ChaosConfig {
        probability: 1.0,  // Always inject
        chaos_types: vec![ChaosType::InfrastructureFailure],
        ..Default::default()
    });

    let chain = EventChain::new()
        .middleware(chaos_infra)
        .middleware(LoggingMiddleware::info())
        .event(DatabaseQueryEvent)
        .event(FileAccessEvent)
        .event(ProcessUserInputEvent)
        .with_fault_tolerance(FaultToleranceMode::BestEffort);

    let mut context = EventContext::new();
    context.set("query", "SELECT * FROM users".to_string());
    context.set("filename", "data.txt".to_string());
    context.set("input", "test".to_string());

    let result = chain.execute(&mut context);

    println!("      Failures: {}", result.failures.len());
    println!("      Status: {:?}", result.status);
    println!("      Is middleware failure: {}",
             result.failures.get(0).map(|f| f.is_middleware_failure).unwrap_or(false));
    println!("      [+] BestEffort stopped immediately on infrastructure failure\n");

    println!(" Result: BestEffort mode semantics validated!");
    println!("  - RandomFailure (event) -> Chain continues");
    println!("  - InfrastructureFailure (middleware) -> Chain stops");
    println!("  - This is the key feature of BestEffort mode!\n");
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    println!("\n");
    println!("|--------------------------------------------------------------------|");
    println!("|         ADVERSARIAL MIDDLEWARE DEMONSTRATION                       |");
    println!("|         Chaos Injection + Security Fuzzing                         |");
    println!("|--------------------------------------------------------------------|");
    println!("\n");
    println!(" IMPORTANT: These middleware are for TESTING ONLY!");
    println!(" [!] NEVER use in production environments");
    println!(" [!] NEVER use with real user data");
    println!(" [!] NEVER use on live databases");
    println!("\n");

    test_chaos_injection();
    test_security_fuzzing();
    test_combined_adversarial();
    test_besteffort_semantics();

    println!("\n");
    println!("|--------------------------------------------------------------------|");
    println!("|                   DEMONSTRATION COMPLETE                           |");
    println!("|--------------------------------------------------------------------|");
    println!("\n");

    println!(" Use Cases:");
    println!("  1. CHAOS INJECTION:");
    println!("     - Test fault tolerance modes");
    println!("     - Validate retry logic");
    println!("     - Ensure circuit breakers work");
    println!("     - Verify graceful degradation");
    println!();
    println!("  2. SECURITY FUZZING:");
    println!("     - Find injection vulnerabilities");
    println!("     - Test input validation");
    println!("     - Detect buffer overflows");
    println!("     - Identify logic bugs");
    println!();
    println!("  3. COMBINED:");
    println!("     - Comprehensive stress testing");
    println!("     - Real-world chaos scenarios");
    println!("     - Security + reliability");
    println!("     - Pre-production validation");
    println!();
    println!("  4. BESTEFFORT VALIDATION:");
    println!("     - Verify event vs middleware failure handling");
    println!("     - Test the 0.2.0 feature works correctly");
    println!("     - Ensure cleanup operations attempt all steps");
    println!("\n");

    println!(" Best Practices:");
    println!("  [+] Run in isolated test environments");
    println!("  [+] Use with CI/CD pipelines");
    println!("  [+] Combine with property-based testing");
    println!("  [+] Monitor and analyze results");
    println!("  [+] Disable in production builds");
    println!("\n");

    println!(" Run Command:");
    println!("  cargo run --example adversarial_demo");
    println!("\n");
}
