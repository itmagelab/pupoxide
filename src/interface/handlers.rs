use crate::application::{
    EnvironmentLoader, ProviderRegistry, PupoxideEngine, execute_transaction,
};
use crate::interface::cli::MasterAction;
use crate::interface::utils::resolve_module_path;
use anyhow::{Result, anyhow};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info, warn};

pub async fn handle_run(
    file: PathBuf,
    module_path: Option<PathBuf>,
    dry_run: bool,
    show_unchanged: bool,
) -> Result<()> {
    let state_dir = PathBuf::from("/tmp/pupoxide");
    let state_store = crate::infrastructure::StateStore::new(state_dir.join("state"));

    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Arc::new(crate::infrastructure::FsAdapter));
    provider_registry.register(Arc::new(crate::infrastructure::ExecAdapter));
    let provider = Arc::new(provider_registry);

    let engine = PupoxideEngine::new(None);

    if let Some(mp) = resolve_module_path(&file, module_path) {
        engine.set_module_path(mp);
    }

    let facts = crate::infrastructure::Facter::collect();
    let catalog = engine.run_manifest(file, "localhost".to_string(), "local".to_string(), facts)?;

    crate::interface::formatter::PrettyFormatter::print_header();
    let reports = execute_transaction(catalog, &state_store, provider, dry_run, |report| {
        if show_unchanged || report.status != crate::domain::report::ResourceStatus::Unchanged {
            println!(
                "{}",
                crate::interface::formatter::PrettyFormatter::format_line(report)
            );
        }
    })
    .await?;

    crate::interface::formatter::PrettyFormatter::print_summary(&reports);

    Ok(())
}

pub async fn handle_apply(
    environment: String,
    dry_run: bool,
    show_unchanged: bool,
    config: PathBuf,
) -> Result<()> {
    let state_dir = PathBuf::from("/tmp/pupoxide");
    let state_store = crate::infrastructure::StateStore::new(state_dir.join("state"));

    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Arc::new(crate::infrastructure::FsAdapter));
    provider_registry.register(Arc::new(crate::infrastructure::ExecAdapter));
    let provider = Arc::new(provider_registry);

    let loader = EnvironmentLoader::new(config);
    let manifest_path = loader.get_site_manifest(&environment)?;
    let modules_path = loader.get_modules_path(&environment);

    let mut stash = None;
    let env_path = modules_path
        .parent()
        .ok_or_else(|| anyhow!("Environment path must have a parent"))?
        .to_path_buf();

    match crate::infrastructure::Stash::new(env_path) {
        Ok(s) => stash = s,
        Err(e) => warn!("Failed to load Stash: {}", e),
    }

    let engine = PupoxideEngine::new(stash);
    let facts = crate::infrastructure::Facter::collect();
    let catalog = engine.run_manifest_with_modules(
        manifest_path,
        modules_path,
        "localhost".to_string(),
        environment,
        facts,
    )?;

    crate::interface::formatter::PrettyFormatter::print_header();
    let reports = execute_transaction(catalog, &state_store, provider, dry_run, |report| {
        if show_unchanged || report.status != crate::domain::report::ResourceStatus::Unchanged {
            println!(
                "{}",
                crate::interface::formatter::PrettyFormatter::format_line(report)
            );
        }
    })
    .await?;

    crate::interface::formatter::PrettyFormatter::print_summary(&reports);

    Ok(())
}

pub async fn handle_master(
    action: MasterAction,
    config: Option<PathBuf>,
    default_config: PathBuf,
) -> Result<()> {
    let config_dir = config.unwrap_or(default_config);

    tokio::fs::create_dir_all(&config_dir).await?;

    let loader = EnvironmentLoader::new(config_dir.clone());
    let engine = PupoxideEngine::new(None);

    let certs_dir = config_dir.join("certs");
    tokio::fs::create_dir_all(&certs_dir).await?;

    let ca_cert_path = certs_dir.join("ca.pem");
    let ca_key_path = certs_dir.join("ca.key");
    let ca = crate::infrastructure::CertificateAuthority::new_or_load(&ca_cert_path, &ca_key_path)?;

    ca.save(&ca_cert_path, &ca_key_path)?;

    let bootstrap_requests_dir = certs_dir.join("bootstrap_requests");
    let agents_dir = certs_dir.join("agents");

    tokio::fs::create_dir_all(&bootstrap_requests_dir).await?;
    tokio::fs::create_dir_all(&agents_dir).await?;

    let bootstrap_manager =
        crate::infrastructure::BootstrapRequestManager::new(bootstrap_requests_dir);
    let agent_registry = crate::infrastructure::AgentRegistryFs::new(agents_dir);

    let state = crate::interface::server::MasterState {
        engine,
        loader,
        ca,
        bootstrap_manager,
        agent_registry,
    };

    match action {
        MasterAction::Start { port } => {
            crate::interface::server::start_master(state, port).await?;
        }
        MasterAction::Sign { node } => {
            let request = state.bootstrap_manager.get_request(&node).await?;

            if !request.is_pending() {
                error!(
                    "Request for node {} is not pending (status: {})",
                    node, request.status
                );
                anyhow::bail!(
                    "Request for node {} cannot be signed (status: {})",
                    node,
                    request.status
                );
            }

            let cert_pem = state.ca.sign_csr(&node, 365)?;
            state.bootstrap_manager.approve_request(&node).await?;
            state
                .agent_registry
                .register(&node, &node, cert_pem)
                .await?;

            info!("Successfully signed and registered node: {}", node);
            println!("✓ Node '{}' has been approved and registered", node);
        }
        MasterAction::Reject { node } => {
            let request = state.bootstrap_manager.get_request(&node).await?;

            if !request.is_pending() {
                error!(
                    "Request for node {} is not pending (status: {})",
                    node, request.status
                );
                anyhow::bail!(
                    "Request for node {} cannot be rejected (status: {})",
                    node,
                    request.status
                );
            }

            state.bootstrap_manager.reject_request(&node).await?;

            info!("Rejected bootstrap request for node: {}", node);
            println!("✓ Request for node '{}' has been rejected", node);
        }
        MasterAction::List => {
            let requests = state.bootstrap_manager.list_pending_requests().await?;

            if requests.is_empty() {
                println!("No pending bootstrap requests");
            } else {
                println!("\nPending Bootstrap Requests:");
                println!("{:-<60}", "");
                println!("{:<20} {:<20} {:<10}", "Node ID", "Requested At", "Status");
                println!("{:-<60}", "");

                for req in requests {
                    let dt = chrono::DateTime::<chrono::Utc>::from_timestamp(req.requested_at, 0)
                        .unwrap_or_default();
                    println!(
                        "{:<20} {:<20} {:<10}",
                        req.node_id,
                        dt.format("%Y-%m-%d %H:%M:%S"),
                        req.status
                    );
                }
            }
        }
    }
    Ok(())
}

pub struct AgentOptions {
    pub server: String,
    pub node: String,
    pub environment: String,
    pub bootstrap: bool,
    pub check: bool,
    pub check_timeout: u64,
    pub dry_run: bool,
    pub show_unchanged: bool,
    pub cert_dir: Option<PathBuf>,
}

pub async fn handle_agent(opts: AgentOptions) -> Result<()> {
    let agent = crate::interface::agent::PupoxideAgent::new(
        opts.server,
        opts.node,
        opts.environment,
        opts.cert_dir,
    );

    if opts.check {
        agent.check_bootstrap_status(opts.check_timeout).await?;
    } else if opts.bootstrap {
        agent.bootstrap().await?;
    } else {
        agent.run(opts.dry_run, opts.show_unchanged).await?;
    }
    Ok(())
}

pub async fn handle_graph(
    file: PathBuf,
    module_path: Option<PathBuf>,
    filter: Option<Vec<String>>,
    max_depth: usize,
    style: String,
) -> Result<()> {
    let engine = PupoxideEngine::new(None);

    if let Some(mp) = resolve_module_path(&file, module_path) {
        engine.set_module_path(mp);
    }

    let facts = crate::infrastructure::Facter::collect();
    let catalog = engine.run_manifest(file, "localhost".to_string(), "local".to_string(), facts)?;

    let style = match style.as_str() {
        "mermaid" => crate::interface::graph::GraphStyle::Mermaid,
        _ => crate::interface::graph::GraphStyle::Ascii,
    };

    crate::interface::graph::display_graph(&catalog, filter.as_deref(), max_depth, style)?;
    Ok(())
}
