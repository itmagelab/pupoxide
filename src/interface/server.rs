use crate::application::engine::PupoxideEngine;
use crate::application::loader::EnvironmentLoader;
use crate::domain::bootstrap::{BootstrapRequest, BootstrapResponse};
use crate::domain::catalog::Catalog;
use crate::domain::facts::Facts;
use crate::infrastructure::{BootstrapRequestManager, AgentRegistryFs};
use crate::infrastructure::certificate::CertificateAuthority;
use axum::{
    extract::ConnectInfo,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json, Router,
    extract::{Path, State},
    routing::post,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{debug, error, info};

pub struct MasterState {
    pub engine: PupoxideEngine,
    pub loader: EnvironmentLoader,
    pub ca: CertificateAuthority,
    pub bootstrap_manager: BootstrapRequestManager,
    pub agent_registry: AgentRegistryFs,
}

pub async fn start_master(state: MasterState, port: u16) -> anyhow::Result<()> {
    let shared_state = Arc::new(state);

    let app = Router::new()
        .route("/bootstrap", post(bootstrap_request))
        .route("/bootstrap/check", post(check_bootstrap))
        .route("/catalog/{env}/{node}", post(get_catalog))
        .with_state(shared_state)
        .into_make_service_with_connect_info::<SocketAddr>();

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("Pupoxide Master listening on port {}", port);
    axum::serve(listener, app).await?;

    Ok(())
}

/// Bootstrap endpoint - Phase 1
/// Agent sends CSR, request is stored for admin approval
async fn bootstrap_request(
    State(state): State<Arc<MasterState>>,
    Json(payload): Json<BootstrapRequest>,
) -> Result<Json<BootstrapResponse>, ServerError> {
    info!(node_id = %payload.node_id, "Received bootstrap request from agent");

    // Store the bootstrap request
    let request = state
        .bootstrap_manager
        .create_request(&payload.node_id, payload.csr)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create bootstrap request");
            ServerError(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to process bootstrap request".into(),
            )
        })?;

    Ok(Json(BootstrapResponse {
        status: request.status,
        message: "Request received. Awaiting admin approval.".to_string(),
        certificate: None,
        ca_certificate: None,
    }))
}

/// Check bootstrap status endpoint
/// Agent checks if their request was approved and fetches certificate
async fn check_bootstrap(
    State(state): State<Arc<MasterState>>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<BootstrapResponse>, ServerError> {
    let node_id = payload
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or(ServerError(
            StatusCode::BAD_REQUEST,
            "Missing node_id".into(),
        ))?;

    debug!(node_id = node_id, "Checking bootstrap status");

    // Get the request
    let request = state
        .bootstrap_manager
        .get_request(node_id)
        .await
        .map_err(|_| {
            ServerError(StatusCode::NOT_FOUND, format!("No request found for {}", node_id))
        })?;

    match request.status.as_str() {
        "pending" => Ok(Json(BootstrapResponse {
            status: "pending".to_string(),
            message: "Request still pending admin approval.".to_string(),
            certificate: None,
            ca_certificate: None,
        })),
        "approved" => {
            // Sign certificate
            let signed_cert = state
                .ca
                .sign_csr(node_id, 365)
                .map_err(|e| {
                    error!(error = %e, "Certificate signing failed");
                    ServerError(StatusCode::INTERNAL_SERVER_ERROR, "Failed to sign certificate".into())
                })?;

            // Register agent
            state
                .agent_registry
                .register(node_id, node_id, signed_cert.clone())
                .await
                .map_err(|e| {
                    error!(error = %e, "Agent registration failed");
                    ServerError(StatusCode::INTERNAL_SERVER_ERROR, "Failed to register agent".into())
                })?;

            info!(node_id = node_id, "Agent approved and certificate signed");

            Ok(Json(BootstrapResponse {
                status: "approved".to_string(),
                message: "Certificate approved and ready.".to_string(),
                certificate: Some(signed_cert),
                ca_certificate: Some(state.ca.cert_pem().to_string()),
            }))
        }
        "rejected" => Ok(Json(BootstrapResponse {
            status: "rejected".to_string(),
            message: "Request was rejected by admin.".to_string(),
            certificate: None,
            ca_certificate: None,
        })),
        _ => Err(ServerError(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Unknown request status".into(),
        )),
    }
}

/// Get catalog endpoint - Phase 2 (mTLS)
async fn get_catalog(
    Path((env, node)): Path<(String, String)>,
    State(state): State<Arc<MasterState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(facts): Json<Facts>,
) -> Result<Json<Catalog>, ServerError> {
    debug!(node = %node, addr = %addr, "Catalog request received");

    // Verify agent is registered
    state
        .agent_registry
        .is_registered(&node)
        .await
        .map_err(|e| {
            error!(error = %e, node = %node, "Agent lookup failed");
            ServerError(StatusCode::INTERNAL_SERVER_ERROR, "Database error".into())
        })?
        .then_some(())
        .ok_or(ServerError(StatusCode::FORBIDDEN, format!("Agent {} not registered", node)))?;

    // Update last seen
    state
        .agent_registry
        .update_last_seen(&node)
        .await
        .map_err(|e| {
            error!(error = %e, node = %node, "Failed to update last_seen");
            ServerError(StatusCode::INTERNAL_SERVER_ERROR, "Database error".into())
        })?;

    // 1. Find manifest
    let manifest_path = state
        .loader
        .get_site_manifest(&env)
        .map_err(|e| ServerError(StatusCode::NOT_FOUND, e.to_string()))?;

    let modules_path = state.loader.get_modules_path(&env);

    // 2. Compile catalog
    let catalog = state
        .engine
        .run_manifest_with_modules(manifest_path, modules_path, node.clone(), env.clone(), facts)
        .map_err(|e| {
            error!(error = %e, node = %node, env = %env, "Catalog compilation failed");
            ServerError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    info!(node = %node, env = %env, resources = catalog.resources.len(), "Catalog generated successfully");

    Ok(Json(catalog))
}

pub struct ServerError(StatusCode, String);

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        (self.0, self.1).into_response()
    }
}
