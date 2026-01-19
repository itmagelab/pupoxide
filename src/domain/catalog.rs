use crate::domain::resource::Resource;
use chrono;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    pub node_name: String,
    pub environment: String,
    pub resources: Vec<Resource>,
    pub evaluation_order: Vec<Resource>,
    pub timestamp: i64,
}

impl Catalog {
    pub fn new(
        node_name: String,
        environment: String,
        resources: Vec<Resource>,
        evaluation_order: Vec<Resource>,
    ) -> Self {
        Self {
            node_name,
            environment,
            resources,
            evaluation_order,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}
