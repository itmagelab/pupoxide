use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use crate::domain::resource::RollbackStatus;
use crate::domain::catalog::Catalog;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub timestamp: i64,
    pub original_catalog: Catalog,
    /// Maps resource ID to its original state before application
    pub original_states: HashMap<String, crate::domain::resource::ResourceState>,
    /// Maps resource ID to its backup hash (if any)
    pub backups: HashMap<String, String>,
    pub resource_statuses: HashMap<String, RollbackStatus>,
}

impl Transaction {
    pub fn new(id: String, original_catalog: Catalog) -> Self {
        Self {
            id,
            timestamp: chrono::Utc::now().timestamp(),
            original_catalog,
            original_states: HashMap::new(),
            backups: HashMap::new(),
            resource_statuses: HashMap::new(),
        }
    }
}
