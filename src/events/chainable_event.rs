use crate::core::event_context::EventContext;
use crate::core::event_result::EventResult;

/// Trait for chainable events
pub trait ChainableEvent: Send + Sync {
    fn execute(&self, context: &mut EventContext) -> EventResult<()>;
    fn name(&self) -> &str;
}
