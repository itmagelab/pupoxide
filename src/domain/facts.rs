use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Facts {
    pub values: HashMap<String, String>,
}

impl Facts {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: String, value: String) {
        self.values.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.values.get(key)
    }
}
