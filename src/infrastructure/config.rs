use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Config {
    #[serde(default)]
    pub master: MasterConfig,
    #[serde(default)]
    pub agent: AgentConfig,
    #[serde(default)]
    pub common: CommonConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CommonConfig {
    pub state_dir: PathBuf,
}

impl Default for CommonConfig {
    fn default() -> Self {
        Self {
            state_dir: PathBuf::from(".pupoxide"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MasterConfig {
    pub port: u16,
    pub certs_dir: PathBuf,
    pub config_dir: PathBuf,
}

impl Default for MasterConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            certs_dir: PathBuf::from("/etc/pupoxide/certs"),
            config_dir: PathBuf::from("/etc/pupoxide"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AgentConfig {
    pub server_url: String,
    pub node_name: String,
    pub environment: String,
    pub cert_dir: Option<PathBuf>,
    pub show_unchanged: bool,
    pub dry_run: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            server_url: "https://localhost:8080".to_string(),
            node_name: "localhost".to_string(),
            environment: "production".to_string(),
            cert_dir: None,
            show_unchanged: false,
            dry_run: false,
        }
    }
}

impl Config {
    /// Loads configuration from the given YAML file path.
    /// If the file does not exist, returns the default configuration.
    pub fn load_or_default(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read configuration file {:?}: {}", path, e))?;

        let config: Config = yaml_serde::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse configuration file {:?}: {}", path, e))?;

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.master.port, 8080);
        assert_eq!(config.common.state_dir, PathBuf::from(".pupoxide"));
        assert_eq!(config.agent.node_name, "localhost");
    }

    #[test]
    fn test_load_non_existent_file() {
        let path = Path::new("non_existent_file_for_pupoxide_test.yaml");
        let config = Config::load_or_default(path).expect("Should load default config");
        assert_eq!(config, Config::default());
    }

    #[test]
    fn test_load_valid_yaml() -> Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("pupoxide.yaml");

        let yaml_content = r#"
master:
  port: 9090
  certs_dir: /tmp/certs
  config_dir: /tmp/config
agent:
  server_url: https://master.example.com
  node_name: agent-node-01
  environment: staging
  show_unchanged: true
  dry_run: true
common:
  state_dir: /tmp/state
"#;
        fs::write(&file_path, yaml_content)?;

        let config = Config::load_or_default(&file_path)?;

        assert_eq!(config.master.port, 9090);
        assert_eq!(config.master.certs_dir, PathBuf::from("/tmp/certs"));
        assert_eq!(config.master.config_dir, PathBuf::from("/tmp/config"));
        assert_eq!(config.agent.server_url, "https://master.example.com");
        assert_eq!(config.agent.node_name, "agent-node-01");
        assert_eq!(config.agent.environment, "staging");
        assert!(config.agent.show_unchanged);
        assert!(config.agent.dry_run);
        assert_eq!(config.common.state_dir, PathBuf::from("/tmp/state"));

        Ok(())
    }

    #[test]
    fn test_load_partial_yaml() -> Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("pupoxide.yaml");

        let yaml_content = r#"
master:
  port: 7070
"#;
        fs::write(&file_path, yaml_content)?;

        let config = Config::load_or_default(&file_path)?;

        assert_eq!(config.master.port, 7070);
        // Other fields should remain default
        assert_eq!(
            config.master.certs_dir,
            PathBuf::from("/etc/pupoxide/certs")
        );
        assert_eq!(config.common.state_dir, PathBuf::from(".pupoxide"));
        assert_eq!(config.agent.node_name, "localhost");

        Ok(())
    }
}
