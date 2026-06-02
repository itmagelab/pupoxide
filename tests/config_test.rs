use anyhow::Result;
use pupoxide::infrastructure::Config;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

fn merge_str(cli: Option<String>, config: String) -> String {
    cli.unwrap_or(config)
}

#[test]
fn test_config_merging_logic() -> Result<()> {
    let dir = tempdir()?;
    let config_file = dir.path().join("pupoxide.yaml");

    let yaml = r#"
master:
  port: 7070
agent:
  server_url: https://config.example.com
  node_name: config-node
  environment: staging
  show_unchanged: true
  dry_run: false
common:
  state_dir: /var/lib/config-state
"#;
    fs::write(&config_file, yaml)?;

    let file_config = Config::load_or_default(&config_file)?;
    assert_eq!(file_config.master.port, 7070);
    assert_eq!(file_config.agent.node_name, "config-node");
    assert_eq!(
        file_config.common.state_dir,
        PathBuf::from("/var/lib/config-state")
    );

    // Simulating CLI override:
    let cli_server = Some("https://cli.example.com".to_string());
    let cli_node = None; // Should fallback to config file
    let cli_dry_run = true; // Should override dry_run (false in config)

    // Merging logic similar to handlers.rs
    let final_server = merge_str(cli_server, file_config.agent.server_url);
    let final_node = merge_str(cli_node, file_config.agent.node_name);
    let final_dry_run = cli_dry_run || file_config.agent.dry_run;

    assert_eq!(final_server, "https://cli.example.com");
    assert_eq!(final_node, "config-node");
    assert!(final_dry_run);

    Ok(())
}
