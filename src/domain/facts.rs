use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Facts {
    pub values: HashMap<String, String>,
}

impl Facts {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(key: String, value: String) -> Self {
        let mut facts = Self::default();
        facts.insert(key, value);
        facts
    }

    pub fn insert(&mut self, key: String, value: String) {
        self.values.insert(key, value);
    }

    pub fn with_insert(mut self, key: String, value: String) -> Self {
        self.insert(key, value);
        self
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.values.get(key)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }
}
