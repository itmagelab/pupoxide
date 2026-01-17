use crate::domain::Facts;
use crate::domain::error::Result;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StashConfig {
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
pub struct Stash {
    _config_path: PathBuf,
    data_dir: PathBuf,
    config: StashConfig,
}

impl Stash {
    pub fn new(environment_path: PathBuf) -> Result<Option<Self>> {
        let config_path = environment_path.join("stash.yaml");
        if !config_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| anyhow::anyhow!("Failed to read stash.yaml: {}", e))?;

        let config: StashConfig = serde_yaml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse stash.yaml: {}", e))?;

        let data_dir = environment_path.join("data");

        Ok(Some(Self {
            _config_path: config_path,
            data_dir,
            config,
        }))
    }

    pub fn lookup(&self, key: &str, facts: &Facts) -> Option<Value> {
        debug!("Stash lookup for key: {}", key);

        for entry in &self.config.hierarchy {
            if let Some(value) = self.lookup_in_entry(entry, key, facts) {
                return Some(value);
            }
        }

        // Check defaults if configured
        self.config.defaults.as_ref().and_then(|defaults| {
            defaults.get(key).cloned()
        })
    }

    fn lookup_in_entry(
        &self,
        entry: &HierarchyEntry,
        key: &str,
        facts: &Facts,
    ) -> Option<Value> {
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

            debug!("Checking Stash file: {:?}", file_path);

            if file_path.exists() {
                match self.read_yaml_value(&file_path, key) {
                    Ok(Some(v)) => return Some(v),
                    Ok(None) => continue,
                    Err(e) => {
                        warn!("Error reading Stash file {:?}: {}", file_path, e);
                        continue;
                    }
                }
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
            .map_err(|e| anyhow::anyhow!("Failed to read data file: {}", e))?;

        // We parse as generic Value to handle scalar, array, or map
        let yaml_map: serde_yaml::Value = serde_yaml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse data file: {}", e))?;

        match yaml_map {
            Value::Mapping(map) => {
                let key_val = serde_yaml::Value::String(key.to_string());
                Ok(map.get(&key_val).cloned())
            }
            _ => Ok(None),
        }
    }
}
