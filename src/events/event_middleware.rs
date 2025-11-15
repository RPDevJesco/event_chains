use crate::events::chainable_event::ChainableEvent;
use crate::core::event_context::EventContext;
use crate::core::event_result::EventResult;

/// Trait for middleware
pub trait EventMiddleware: Send + Sync {
    fn execute(
        &self,
        event: &dyn ChainableEvent,
        context: &mut EventContext,
        next: &mut dyn FnMut(&mut EventContext) -> EventResult<()>,
    ) -> EventResult<()>;
}
