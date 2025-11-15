//! Event Chains - A flexible event processing pipeline
//!
//! This library provides a robust system for building and executing
//! event processing chains with middleware support and configurable
//! fault tolerance.
//!
//! # Quick Start
//!
//! ```ignore
//! use event_chains::core::event_chain::EventChain;
//! use event_chains::core::event_context::EventContext;
//! use event_chains::core::fault_tolerance_mode::FaultToleranceMode;
//!
//! let chain = EventChain::new()
//!     .event(MyEvent)
//!     .with_fault_tolerance(FaultToleranceMode::Lenient);
//!
//! let mut context = EventContext::new();
//! let result = chain.execute(&mut context);
//! ```

pub mod core;
pub mod events;
pub mod middleware;

// Convenience re-exports
pub use core::event_chain::EventChain;
pub use core::event_context::EventContext;
pub use core::event_result::EventResult;
pub use core::fault_tolerance_mode::FaultToleranceMode;
pub use events::chainable_event::ChainableEvent;
pub use events::event_middleware::EventMiddleware;
