use crate::domain::catalog::Catalog;
use crate::domain::resource::ResourceProvider;
use crate::infrastructure::StateStore;
use anyhow::Result;
use std::sync::Arc;

pub async fn execute_transaction(
    catalog: Catalog,
    state_store: &StateStore,
    provider: Arc<dyn ResourceProvider>,
    dry_run: bool,
) -> Result<()> {
    let transaction_id = format!("tx_{}", chrono::Utc::now().timestamp());
    let mut transaction =
        crate::domain::transaction::Transaction::new(transaction_id.clone(), catalog.clone());

    tracing::info!(id = %transaction_id, dry_run = %dry_run, "Starting transaction");

    for resource in &catalog.resources {
        // Skip Meta resources
        if let crate::domain::resource::Resource::Meta(_) = resource {
            continue;
        }

        if dry_run {
            tracing::info!(id = %resource.id(), "Would ensure resource");
            continue;
        }

        // 1. Snapshot original state
        // We pass full=false because we don't need content for backups anymore
        let state = provider.get_state(resource, false).await?;

        transaction
            .original_states
            .insert(resource.id().to_string(), state.clone());

        // 2. Apply changes
        if let Err(e) = provider.apply(resource).await {
            tracing::error!(id = %resource.id(), error = %e, "Failed to apply resource");
            state_store.save_transaction(&transaction)?;
            return Err(e.into());
        }
    }

    if !dry_run {
        state_store.save_transaction(&transaction)?;
    }
    tracing::info!(id = %transaction_id, "Transaction completed");
    Ok(())
}
