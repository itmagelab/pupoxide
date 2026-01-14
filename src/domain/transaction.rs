use crate::domain::catalog::Catalog;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub timestamp: i64,
    pub original_catalog: Catalog,
    /// Maps resource ID to its original state before application
    pub original_states: HashMap<String, crate::domain::resource::ResourceState>,

}

impl Transaction {
    pub fn new(id: String, original_catalog: Catalog) -> Self {
        Self {
            id,
            timestamp: chrono::Utc::now().timestamp(),
            original_catalog,
            original_states: HashMap::new(),
        }
    }
}
