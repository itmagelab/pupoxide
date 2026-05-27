use crate::domain::catalog::Catalog;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A transaction record representing a catalog application attempt.
///
/// Tracks the catalog applied and keeps snapshots of resource states prior to execution,
/// allowing for audits and future rollback features.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// The unique identifier of this transaction.
    pub id: String,
    /// UNIX timestamp when the transaction execution began.
    pub timestamp: i64,
    /// The catalog copy that was applied.
    pub original_catalog: Catalog,
    /// Maps resource ID to its system state prior to synchronization.
    pub original_states: HashMap<String, crate::domain::resource::ResourceState>,
}

impl Transaction {
    /// Creates a new `Transaction` for a given catalog.
    pub fn new(id: String, original_catalog: Catalog) -> Self {
        Self {
            id,
            timestamp: chrono::Utc::now().timestamp(),
            original_catalog,
            original_states: HashMap::new(),
        }
    }
}
