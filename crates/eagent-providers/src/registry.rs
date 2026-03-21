use crate::Provider;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Registry of configured provider instances. Thread-safe for runtime registration.
pub struct ProviderRegistry {
    providers: RwLock<HashMap<String, Arc<dyn Provider>>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
        }
    }

    /// Register or replace a provider by name.
    pub fn register(&self, name: String, provider: Arc<dyn Provider>) {
        self.providers.write().unwrap().insert(name, provider);
    }

    /// Get a provider by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Provider>> {
        self.providers.read().unwrap().get(name).cloned()
    }

    /// List all registered provider names.
    pub fn names(&self) -> Vec<String> {
        self.providers.read().unwrap().keys().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.providers.read().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.read().unwrap().is_empty()
    }

    /// Remove a provider by name.
    pub fn remove(&self, name: &str) -> Option<Arc<dyn Provider>> {
        self.providers.write().unwrap().remove(name)
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
