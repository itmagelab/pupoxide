use anyhow::Result;
use pupoxide::application::PupoxideEngine;
use pupoxide::application::loader::EnvironmentLoader;
use pupoxide::domain::facts::Facts;
use pupoxide::infrastructure::certificate::CertificateAuthority;
use pupoxide::infrastructure::{AgentRegistryFs, BootstrapRequestManager};
use pupoxide::interface::agent::PupoxideAgent;
use pupoxide::interface::server::{MasterState, start_master};
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_mtls_full_workflow() -> Result<()> {
    // 1. Create temporary directory for configuration and certs
    let base_dir = tempdir().expect("Failed to create tempdir");
    let config_dir = base_dir.path().to_path_buf();

    // Create necessary paths
    let certs_dir = config_dir.join("certs");
    fs::create_dir_all(&certs_dir).expect("Failed to create certs dir");

    let env_dir = config_dir.join("environments").join("production");
    let manifests_dir = env_dir.join("manifests");
    let modules_dir = env_dir.join("modules");
    fs::create_dir_all(&manifests_dir).expect("Failed to create manifests dir");
    fs::create_dir_all(&modules_dir).expect("Failed to create modules dir");

    // Create site.rhai manifest
    fs::write(
        manifests_dir.join("site.rhai"),
        r#"file("/tmp/test_mtls_ok.txt", #{ ensure: "present", content: "mTLS OK" });"#,
    )
    .expect("Failed to write site.rhai");

    // Initialize CA certs
    let ca_cert_path = certs_dir.join("ca.pem");
    let ca_key_path = certs_dir.join("ca.key");
    let ca = CertificateAuthority::new_or_load(&ca_cert_path, &ca_key_path)?;
    ca.save(&ca_cert_path, &ca_key_path)?;

    let bootstrap_requests_dir = certs_dir.join("bootstrap_requests");
    let agents_dir = certs_dir.join("agents");
    fs::create_dir_all(&bootstrap_requests_dir).expect("Failed to create bootstrap requests dir");
    fs::create_dir_all(&agents_dir).expect("Failed to create agents dir");

    let bootstrap_manager = BootstrapRequestManager::new(bootstrap_requests_dir);
    let agent_registry = AgentRegistryFs::new(agents_dir);
    let loader = EnvironmentLoader::new(config_dir.clone());
    let engine = PupoxideEngine::new(None);

    let state = MasterState {
        engine,
        loader,
        ca,
        bootstrap_manager,
        agent_registry,
        certs_dir: certs_dir.clone(),
    };

    // Start Master on port 18080
    let port = 18080;
    let state_clone = MasterState {
        engine: state.engine.clone(),
        loader: state.loader.clone(),
        ca: CertificateAuthority::new_or_load(&ca_cert_path, &ca_key_path)?,
        bootstrap_manager: BootstrapRequestManager::new(certs_dir.join("bootstrap_requests")),
        agent_registry: AgentRegistryFs::new(certs_dir.join("agents")),
        certs_dir: certs_dir.clone(),
    };

    tokio::spawn(async move {
        let _ = start_master(state_clone, port).await;
    });

    // Wait a bit for server to spin up
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // 2. Initialize Agent client
    let server_url = format!("https://localhost:{}", port);
    let node_name = "test-agent-01".to_string();
    let environment = "production".to_string();
    let agent_cert_dir = certs_dir.join("agents").join(&node_name);

    let agent = PupoxideAgent::new(
        server_url.clone(),
        node_name.clone(),
        environment.clone(),
        Some(agent_cert_dir.clone()),
    );

    // Test phase 1: Bootstrap (submits CSR request)
    agent.bootstrap().await.expect("Agent bootstrap failed");

    // Verify bootstrap request exists
    let req = state.bootstrap_manager.get_request(&node_name).await?;
    assert_eq!(req.node_id, node_name);
    assert!(req.is_pending());

    // Test check_bootstrap_status before approval (should timeout/fail since not approved)
    let check_res = agent.check_bootstrap_status(1).await;
    assert!(
        check_res.is_err(),
        "Should timeout because it's not approved yet"
    );

    // Approve the request
    let approved_req = state.bootstrap_manager.approve_request(&node_name).await?;
    assert!(approved_req.is_approved());

    // Check bootstrap status again (should succeed and save certificates)
    agent
        .check_bootstrap_status(5)
        .await
        .expect("Checking bootstrap status failed after approval");

    // Verify certificate and CA exist
    assert!(agent_cert_dir.join("agent.pem").exists());
    assert!(agent_cert_dir.join("ca.pem").exists());
    assert!(agent_cert_dir.join("agent.key").exists());

    // Run the agent (should fetch catalog successfully)
    agent.run(true, true).await.expect("Agent run failed");

    // 3. Test security (spoofing check):
    // Create another agent (unauthorized) and try to request node-01 catalog
    let unauthorized_agent = PupoxideAgent::new(
        server_url.clone(),
        "unauthorized-agent".to_string(),
        environment.clone(),
        Some(certs_dir.join("agents").join("unauthorized-agent")),
    );

    // Bootstrap & Approve unauthorized agent so it gets a valid certificate
    unauthorized_agent.bootstrap().await?;
    state
        .bootstrap_manager
        .approve_request("unauthorized-agent")
        .await?;
    unauthorized_agent.check_bootstrap_status(5).await?;

    // Try to fetch catalog of test-agent-01 using unauthorized-agent credentials
    let client_pem = fs::read_to_string(
        certs_dir
            .join("agents")
            .join("unauthorized-agent")
            .join("agent.pem"),
    )?;
    let client_key = fs::read_to_string(
        certs_dir
            .join("agents")
            .join("unauthorized-agent")
            .join("agent.key"),
    )?;
    let ca_pem = fs::read_to_string(
        certs_dir
            .join("agents")
            .join("unauthorized-agent")
            .join("ca.pem"),
    )?;

    let combined_pem = format!("{}\n{}", client_pem.trim_end(), client_key.trim_end());
    let identity = reqwest::Identity::from_pem(combined_pem.as_bytes())?;
    let ca_cert = reqwest::Certificate::from_pem(ca_pem.as_bytes())?;

    let client = reqwest::Client::builder()
        .identity(identity)
        .add_root_certificate(ca_cert)
        .build()?;

    // Send request for "test-agent-01"'s catalog but using "unauthorized-agent" certificate
    let spoof_url = format!("{}/catalog/production/test-agent-01", server_url);
    let resp = client
        .post(&spoof_url)
        .json(&Facts::default())
        .send()
        .await?;
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::FORBIDDEN,
        "Server should reject spoofed catalog requests!"
    );

    Ok(())
}
