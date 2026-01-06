use anyhow::Result;
use crate::infrastructure::FsAdapter;
use crate::domain::resource::ResourceProvider;
use crate::domain::catalog::Catalog;
use crate::infrastructure::{BackupStore, StateStore};

pub async fn execute_transaction(
    catalog: Catalog,
    backup_store: &BackupStore,
    state_store: &StateStore,
    dry_run: bool,
) -> Result<()> {
    let adapter = FsAdapter;
    let transaction_id = format!("tx_{}", chrono::Utc::now().timestamp());
    let mut transaction = crate::domain::transaction::Transaction::new(transaction_id.clone(), catalog.clone());

    tracing::info!(id = %transaction_id, dry_run = %dry_run, "Starting transaction");

    for resource in &catalog.resources {
        // 1. Snapshot original state
        let backup_needed = match resource {
            crate::domain::resource::Resource::File(f) => f.backup,
            crate::domain::resource::Resource::Directory(d) => d.backup,
            _ => false,
        };

        // Even in dry-run, we might want to know the current state, but for now let's just proceed
        // If we wanted to report "Would change", we'd need to compare.
        // For simplicity in this step, we just report intent.

        if dry_run {
             tracing::info!(id = %resource.id(), "Would ensure resource");
             continue;
        }

        let state = adapter.get_state(resource, backup_needed).await?;
        transaction.original_states.insert(resource.id().to_string(), state.clone());

        // 2. Store backup if it's a file with content
        if let crate::domain::resource::ResourceState::Full { content: Some(bytes), .. } = state {
            let hash = backup_store.store(&bytes)?;
            transaction.backups.insert(resource.id().to_string(), hash);
        }

        // 3. Apply changes
        match adapter.apply(resource).await {
            Ok(_) => {
                transaction.resource_statuses.insert(resource.id().to_string(), crate::domain::resource::RollbackStatus::Success);
            }
            Err(e) => {
                transaction.resource_statuses.insert(resource.id().to_string(), crate::domain::resource::RollbackStatus::Failed(e.to_string()));
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
