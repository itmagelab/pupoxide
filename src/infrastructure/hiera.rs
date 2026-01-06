use crate::domain::Facts;
use crate::domain::error::{DomainError, Result};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HieraConfig {
    pub version: u32,
    pub defaults: Option<HashMap<String, Value>>,
    pub hierarchy: Vec<HierarchyEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HierarchyEntry {
    pub name: String,
    pub path: Option<String>,
    pub paths: Option<Vec<String>>,
}

#[derive(Clone)]
pub struct Hiera {
    config_path: PathBuf,
    data_dir: PathBuf,
    config: HieraConfig,
}

impl Hiera {
    pub fn new(environment_path: PathBuf) -> Result<Option<Self>> {
        let config_path = environment_path.join("hiera.yaml");
        if !config_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| DomainError::Hiera(format!("Failed to read hiera.yaml: {}", e)))?;

        let config: HieraConfig = serde_yaml::from_str(&content)
            .map_err(|e| DomainError::Hiera(format!("Failed to parse hiera.yaml: {}", e)))?;

        let data_dir = environment_path.join("data");

        Ok(Some(Self {
            config_path,
            data_dir,
            config,
        }))
    }

    pub fn lookup(&self, key: &str, facts: &Facts) -> Option<Value> {
        debug!("Hiera lookup for key: {}", key);

        for entry in &self.config.hierarchy {
            let mut paths = Vec::new();
            if let Some(p) = &entry.path {
                paths.push(p.clone());
            }
            if let Some(ps) = &entry.paths {
                paths.extend(ps.clone());
            }

            for path_pattern in paths {
                let path_str = self.interpolate(&path_pattern, facts);
                let file_path = self.data_dir.join(&path_str);

                debug!("Checking Hiera file: {:?}", file_path);

                if file_path.exists() {
                    match self.read_yaml_value(&file_path, key) {
                        Ok(Some(v)) => return Some(v),
                        Ok(None) => continue, // Key not found in this file
                        Err(e) => {
                            warn!("Error reading Hiera file {:?}: {}", file_path, e);
                            continue;
                        }
                    }
                }
            }
        }

        // Check defaults if configured
        if let Some(defaults) = &self.config.defaults {
            if let Some(v) = defaults.get(key) {
                return Some(v.clone());
            }
        }

        None
    }

    fn interpolate(&self, pattern: &str, facts: &Facts) -> String {
        // Simple interpolation %{facts.key}
        // TODO: Use a proper regex or string replacement logic
        let mut result = pattern.to_string();
        for (k, v) in &facts.values {
            let placeholder = format!("%{{facts.{}}}", k);
            result = result.replace(&placeholder, v);
        }
        result
    }

    fn read_yaml_value(&self, path: &PathBuf, key: &str) -> Result<Option<Value>> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| DomainError::Hiera(format!("Failed to read data file: {}", e)))?;

        // We parse as generic Value to handle scalar, array, or map
        let yaml_map: serde_yaml::Value = serde_yaml::from_str(&content)
            .map_err(|e| DomainError::Hiera(format!("Failed to parse data file: {}", e)))?;

        match yaml_map {
            Value::Mapping(map) => {
                let key_val = serde_yaml::Value::String(key.to_string());
                Ok(map.get(&key_val).cloned())
            }
            _ => Ok(None),
        }
    }
}
