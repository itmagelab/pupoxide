use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A collection of key-value system properties (facts) collected from the target machine.
///
/// Facts are used during engine execution to dynamically customize catalogs, select package
/// managers, evaluate conditions, and inject values into templates/Rhai scripts.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Facts {
    /// The map containing all collected facts (e.g. `os_family -> Darwin`, `hostname -> myhost`).
    pub values: HashMap<String, String>,
}

impl Facts {
    /// Creates a new, empty set of `Facts`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `Facts` container initialized with a single key-value pair.
    pub fn with(key: String, value: String) -> Self {
        let mut facts = Self::default();
        facts.insert(key, value);
        facts
    }

    /// Inserts or replaces a fact.
    pub fn insert(&mut self, key: String, value: String) {
        self.values.insert(key, value);
    }

    /// Builder-style method to insert a fact and return the updated container.
    pub fn with_insert(mut self, key: String, value: String) -> Self {
        self.insert(key, value);
        self
    }

    /// Retrieves the value of a fact if it exists.
    pub fn get(&self, key: &str) -> Option<&String> {
        self.values.get(key)
    }

    /// Returns `true` if the given fact key is present.
    pub fn contains_key(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }
}
