use serde::{Serialize, Deserialize};
use crate::domain::resource::Resource;
use chrono;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    pub node_name: String,
    pub environment: String,
    pub resources: Vec<Resource>,
    pub timestamp: i64,
}

impl Catalog {
    pub fn new(node_name: String, environment: String, resources: Vec<Resource>) -> Self {
        Self {
            node_name,
            environment,
            resources,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}
