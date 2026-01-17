use crate::application::engine::PupoxideEngine;
use crate::application::loader::EnvironmentLoader;
use crate::domain::bootstrap::{BootstrapRequest, BootstrapResponse};
use crate::domain::catalog::Catalog;
use crate::domain::facts::Facts;
use crate::infrastructure::{AgentRegistry, BootstrapTokenManager};
use crate::infrastructure::certificate::CertificateAuthority;
use axum::{
    extract::ConnectInfo,
    http::{HeaderMap, StatusCode},
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
    pub bootstrap_manager: BootstrapTokenManager,
    pub agent_registry: AgentRegistry,
}

pub async fn start_master(state: MasterState, port: u16) -> anyhow::Result<()> {
    let shared_state = Arc::new(state);

    let app = Router::new()
        .route("/bootstrap", post(bootstrap))
        .route("/catalog/{env}/{node}", post(get_catalog))
        .with_state(shared_state)
        .into_make_service_with_connect_info::<SocketAddr>();

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("Pupoxide Master listening on port {}", port);
    axum::serve(listener, app).await?;

    Ok(())
}

/// Bootstrap endpoint - Phase 1
/// Agent sends CSR and bootstrap token
async fn bootstrap(
    State(state): State<Arc<MasterState>>,
    headers: HeaderMap,
    Json(payload): Json<BootstrapRequest>,
) -> Result<Json<BootstrapResponse>, ServerError> {
    // 1. Verify bootstrap token
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(ServerError(StatusCode::UNAUTHORIZED, "Missing Authorization header".into()))?;

    // Extract token from "Bearer TOKEN" format
    let token_str = auth_header
        .strip_prefix("Bearer ")
        .ok_or(ServerError(StatusCode::UNAUTHORIZED, "Invalid Authorization format".into()))?;

    let verified_node_id = state
        .bootstrap_manager
        .verify_token(token_str)
        .await
        .map_err(|e| {
            error!(error = %e, "Token verification failed");
            ServerError(StatusCode::FORBIDDEN, "Invalid or expired token".into())
        })?;

    // Ensure CSR is for the correct node_id
    if payload.node_id != verified_node_id {
        return Err(ServerError(
            StatusCode::BAD_REQUEST,
            format!("node_id in CSR ({}) doesn't match token ({})", payload.node_id, verified_node_id),
        ));
    }

    // 2. Sign CSR with Master CA
    let signed_cert = state
        .ca
        .sign_csr(&payload.node_id, 365)
        .map_err(|e| {
            error!(error = %e, "Certificate signing failed");
            ServerError(StatusCode::INTERNAL_SERVER_ERROR, "Failed to sign certificate".into())
        })?;

    // 3. Register agent
    state
        .agent_registry
        .register(&payload.node_id, &payload.node_id, signed_cert.clone())
        .await
        .map_err(|e| {
            error!(error = %e, "Agent registration failed");
            ServerError(StatusCode::INTERNAL_SERVER_ERROR, "Failed to register agent".into())
        })?;

    info!(node_id = %payload.node_id, "Agent successfully registered");

    Ok(Json(BootstrapResponse {
        certificate: signed_cert,
        ca_certificate: state.ca.cert_pem().to_string(),
    }))
}

/// Get catalog endpoint - Phase 2 (mTLS)
async fn get_catalog(
    Path((env, node)): Path<(String, String)>,
    State(state): State<Arc<MasterState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(facts): Json<Facts>,
) -> Result<Json<Catalog>, ServerError> {
    // TODO: Verify mTLS certificate CN matches node parameter
    // For now, this is a placeholder for future mTLS verification
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
