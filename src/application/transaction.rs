use crate::domain::catalog::Catalog;
use crate::domain::resource::{ResourceProvider, ResourceState};
use crate::infrastructure::{BackupStore, StateStore};
use anyhow::Result;
use std::sync::Arc;

pub async fn execute_transaction(
    catalog: Catalog,
    backup_store: &BackupStore,
    state_store: &StateStore,
    provider: Arc<dyn ResourceProvider>,
    dry_run: bool,
) -> Result<()> {
    let transaction_id = format!("tx_{}", chrono::Utc::now().timestamp());
    let mut transaction =
        crate::domain::transaction::Transaction::new(transaction_id.clone(), catalog.clone());

    tracing::info!(id = %transaction_id, dry_run = %dry_run, "Starting transaction");

    for resource in &catalog.resources {
        // 1. Snapshot original state
        let backup_needed = match resource {
            crate::domain::resource::Resource::File(f) => f.backup,
            crate::domain::resource::Resource::Directory(d) => d.backup,
            _ => false,
        };

        if dry_run {
            tracing::info!(id = %resource.id(), "Would ensure resource");
            continue;
        }

        // Use the injected provider for all operations
        let state = provider.get_state(resource, backup_needed).await?;

        transaction
            .original_states
            .insert(resource.id().to_string(), state.clone());

        // 2. Store backup if it's a file with content
        if let crate::domain::resource::ResourceState::Full {
            content: Some(bytes),
            ..
        } = state
        {
            let hash = backup_store.store(&bytes)?;
            transaction.backups.insert(resource.id().to_string(), hash);
        }

        // 3. Apply changes using the injected provider
        let apply_result = provider.apply(resource).await;

        match apply_result {
            Ok(_) => {
                transaction.resource_statuses.insert(
                    resource.id().to_string(),
                    crate::domain::resource::RollbackStatus::Success,
                );
            }
            Err(e) => {
                transaction.resource_statuses.insert(
                    resource.id().to_string(),
                    crate::domain::resource::RollbackStatus::Failed(e.to_string()),
                );
                state_store.save_transaction(&transaction)?;
                return Err(e.into());
            }
        }
    }

    if !dry_run {
        state_store.save_transaction(&transaction)?;
    }
    tracing::info!(id = %transaction_id, "Transaction completed");
    Ok(())
}
