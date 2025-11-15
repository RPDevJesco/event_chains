use hashbrown::HashMap;
use std::any::Any;

// Context that flows through the event chain
pub struct EventContext {
    data: HashMap<String, Box<dyn Any + Send + Sync>>,
}

impl EventContext {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn set<T: Any + Send + Sync>(&mut self, key: &str, value: T) {
        self.data.insert(key.to_string(), Box::new(value));
    }

    pub fn get<T: Any + Send + Sync + Clone>(&self, key: &str) -> Option<T> {
        self.data
            .get(key)
            .and_then(|boxed| boxed.downcast_ref::<T>().cloned())
    }

    pub fn has(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }
}

impl Default for EventContext {
    fn default() -> Self {
        Self::new()
    }
}
