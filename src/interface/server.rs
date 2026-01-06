use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use std::sync::Arc;
use crate::application::engine::PupoxideEngine;
use crate::application::loader::EnvironmentLoader;
use crate::domain::catalog::Catalog;

pub struct MasterState {
    pub engine: PupoxideEngine,
    pub loader: EnvironmentLoader,
}

pub async fn start_master(state: MasterState, port: u16) -> anyhow::Result<()> {
    let shared_state = Arc::new(state);

    let app = Router::new()
        .route("/catalog/:env/:node", get(get_catalog))
        .with_state(shared_state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("Pupoxide Master listening on port {}", port);
    axum::serve(listener, app).await?;

    Ok(())
}

async fn get_catalog(
    Path((env, node)): Path<(String, String)>,
    State(state): State<Arc<MasterState>>,
) -> Json<Catalog> {
    // 1. Find manifest
    let manifest_path = state.loader.get_site_manifest(&env).expect("Manifest not found");
    let modules_path = state.loader.get_modules_path(&env);

    // 2. Compile catalog
    let catalog = state.engine.run_manifest_with_modules(
        manifest_path,
        modules_path,
        node,
        env
    ).expect("Failed to compile catalog");

    Json(catalog)
}
