use crate::application::engine::PupoxideEngine;
use crate::application::loader::EnvironmentLoader;
use crate::domain::bootstrap::{BootstrapRequest, BootstrapResponse};
use crate::domain::catalog::Catalog;
use crate::domain::facts::Facts;
use crate::infrastructure::certificate::CertificateAuthority;
use crate::infrastructure::{AgentRegistryFs, BootstrapRequestManager};
use axum::{
    Json, Router,
    extract::ConnectInfo,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{debug, error, info};

use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use tokio_rustls::TlsAcceptor;

pub struct MasterState {
    pub engine: PupoxideEngine,
    pub loader: EnvironmentLoader,
    pub ca: CertificateAuthority,
    pub bootstrap_manager: BootstrapRequestManager,
    pub agent_registry: AgentRegistryFs,
    pub certs_dir: std::path::PathBuf,
}

#[derive(Clone, Debug)]
pub struct ClientIdentity {
    pub cn: String,
    pub cert_der: Vec<u8>,
}

struct OptionalClientCertVerifier;

impl rustls::server::ClientCertVerifier for OptionalClientCertVerifier {
    fn offer_client_auth(&self) -> bool {
        true
    }

    fn client_auth_mandatory(&self) -> bool {
        false
    }

    fn client_auth_root_subjects(&self) -> &[rustls::DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _now: std::time::SystemTime,
    ) -> Result<rustls::server::ClientCertVerified, rustls::Error> {
        Ok(rustls::server::ClientCertVerified::assertion())
    }
}

pub async fn start_master(state: MasterState, port: u16) -> anyhow::Result<()> {
    let certs_dir = state.certs_dir.clone();
    let ca = &state.ca;

    // 1. Build client cert verifier (optional client certs, trusts CA)
    let client_verifier = Arc::new(OptionalClientCertVerifier);

    // 2. Load or generate server cert and private key
    let server_cert_path = certs_dir.join("server.pem");
    let server_key_path = certs_dir.join("server.key");

    let (server_cert_pem, server_key_pem) = if server_cert_path.exists() && server_key_path.exists()
    {
        (
            tokio::fs::read_to_string(&server_cert_path).await?,
            tokio::fs::read_to_string(&server_key_path).await?,
        )
    } else {
        let (cert, key) = ca.generate_server_cert("localhost")?;
        tokio::fs::write(&server_cert_path, &cert).await?;
        tokio::fs::write(&server_key_path, &key).await?;
        (cert, key)
    };

    let mut cert_reader = std::io::BufReader::new(server_cert_pem.as_bytes());
    let cert_chain: Vec<tokio_rustls::rustls::Certificate> =
        rustls_pemfile::certs(&mut cert_reader)
            .collect::<std::io::Result<Vec<_>>>()?
            .into_iter()
            .map(|c| tokio_rustls::rustls::Certificate(c.to_vec()))
            .collect();

    let mut key_reader = std::io::BufReader::new(server_key_pem.as_bytes());
    let key_der = rustls_pemfile::private_key(&mut key_reader)?
        .ok_or_else(|| anyhow::anyhow!("No private key found in server.key"))?;
    let private_key = tokio_rustls::rustls::PrivateKey(key_der.secret_der().to_vec());

    // 3. Build ServerConfig
    let rustls_config = tokio_rustls::rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_client_cert_verifier(client_verifier)
        .with_single_cert(cert_chain, private_key)?;

    let acceptor = TlsAcceptor::from(Arc::new(rustls_config));
    let shared_state = Arc::new(state);

    let app = Router::new()
        .route("/bootstrap", post(bootstrap_request))
        .route("/bootstrap/check", post(check_bootstrap))
        .route("/catalog/{env}/{node}", post(get_catalog))
        .with_state(shared_state.clone());

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("Pupoxide Master (HTTPS/mTLS) listening on port {}", port);

    loop {
        let (stream, peer_addr) = match listener.accept().await {
            Ok(val) => val,
            Err(e) => {
                tracing::error!("Failed to accept TCP connection: {}", e);
                continue;
            }
        };

        let acceptor = acceptor.clone();
        let app = app.clone();

        tokio::spawn(async move {
            match acceptor.accept(stream).await {
                Ok(tls_stream) => {
                    // Extract peer certificate info if present
                    let client_id = if let Some(certs) = tls_stream.get_ref().1.peer_certificates()
                    {
                        if let Some(cert) = certs.first() {
                            if let Ok(cn) =
                                crate::infrastructure::certificate::extract_cn_from_der(&cert.0)
                            {
                                Some(ClientIdentity {
                                    cn,
                                    cert_der: cert.0.clone(),
                                })
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    let service = tower::service_fn(
                        move |req: axum::http::Request<hyper::body::Incoming>| {
                            let mut req = req.map(axum::body::Body::new);
                            req.extensions_mut().insert(ConnectInfo(peer_addr));
                            if let Some(ref identity) = client_id {
                                req.extensions_mut().insert(identity.clone());
                            }

                            let mut app_clone = app.clone();
                            async move {
                                use tower::Service;
                                app_clone.call(req).await
                            }
                        },
                    );

                    let hyper_service = hyper_util::service::TowerToHyperService::new(service);
                    let io = TokioIo::new(tls_stream);
                    if let Err(err) = auto::Builder::new(TokioExecutor::new())
                        .serve_connection(io, hyper_service)
                        .await
                    {
                        tracing::debug!("Error serving connection: {}", err);
                    }
                }
                Err(e) => {
                    tracing::error!("TLS handshake error for peer {}: {}", peer_addr, e);
                }
            }
        });
    }
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
        .create_request(&payload.node_id, payload.csr, payload.certificate.clone())
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
            ServerError(
                StatusCode::NOT_FOUND,
                format!("No request found for {}", node_id),
            )
        })?;

    match request.status.as_str() {
        "pending" => Ok(Json(BootstrapResponse {
            status: "pending".to_string(),
            message: "Request still pending admin approval.".to_string(),
            certificate: None,
            ca_certificate: None,
        })),
        "approved" => {
            // Get the agent's certificate from the request
            let agent_cert = request.certificate.clone().ok_or_else(|| {
                error!("No certificate in bootstrap request for {}", node_id);
                ServerError(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "No certificate in request".into(),
                )
            })?;

            // Register agent
            state
                .agent_registry
                .register(node_id, node_id, agent_cert.clone())
                .await
                .map_err(|e| {
                    error!(error = %e, "Agent registration failed");
                    ServerError(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to register agent".into(),
                    )
                })?;

            info!(node_id = node_id, "Agent approved and registered");

            Ok(Json(BootstrapResponse {
                status: "approved".to_string(),
                message: "Certificate approved and ready.".to_string(),
                certificate: Some(agent_cert),
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
    client_id_ext: Option<axum::Extension<ClientIdentity>>,
    Json(facts): Json<Facts>,
) -> Result<Json<Catalog>, ServerError> {
    debug!(node = %node, addr = %addr, "Catalog request received");

    // Verify client identity (mTLS verification)
    let client_id = match client_id_ext {
        Some(axum::Extension(id)) => id,
        None => {
            error!(node = %node, "Client certificate missing for catalog request");
            return Err(ServerError(
                StatusCode::UNAUTHORIZED,
                "Client certificate missing".into(),
            ));
        }
    };

    // 1. Verify CN matches the node parameter
    if client_id.cn != node {
        error!(
            cn = %client_id.cn,
            node = %node,
            "Client CN does not match requested node ID"
        );
        return Err(ServerError(
            StatusCode::FORBIDDEN,
            format!(
                "Access forbidden: certificate CN '{}' does not match node ID '{}'",
                client_id.cn, node
            ),
        ));
    }

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
        .ok_or(ServerError(
            StatusCode::FORBIDDEN,
            format!("Agent {} not registered", node),
        ))?;

    // 2. Verify certificate matches the registered certificate
    let agent = state.agent_registry.get_agent(&node).await.map_err(|e| {
        error!(error = %e, node = %node, "Failed to get registered agent");
        ServerError(
            StatusCode::FORBIDDEN,
            "Access forbidden: agent not registered".into(),
        )
    })?;

    let mut reader = std::io::BufReader::new(agent.certificate_pem.as_bytes());
    let reg_certs = rustls_pemfile::certs(&mut reader)
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|e| {
            error!(error = %e, node = %node, "Failed to parse registered certificate");
            ServerError(StatusCode::INTERNAL_SERVER_ERROR, "Registry error".into())
        })?;

    let reg_cert_der = reg_certs.first().ok_or_else(|| {
        error!(node = %node, "Registered certificate is empty");
        ServerError(StatusCode::INTERNAL_SERVER_ERROR, "Registry error".into())
    })?;

    if reg_cert_der.as_ref() != client_id.cert_der {
        error!(node = %node, "Client certificate does not match the registered certificate");
        return Err(ServerError(
            StatusCode::FORBIDDEN,
            "Access forbidden: certificate mismatch".into(),
        ));
    }

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
        .run_manifest_with_modules(
            manifest_path,
            modules_path,
            node.clone(),
            env.clone(),
            facts,
        )
        .map_err(|e| {
            error!(error = %e, node = %node, env = %env, "Catalog compilation failed");
            ServerError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    info!(node = %node, env = %env, resources = catalog.resources().len(), "Catalog generated successfully");

    Ok(Json(catalog))
}

pub struct ServerError(StatusCode, String);

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        (self.0, self.1).into_response()
    }
}
