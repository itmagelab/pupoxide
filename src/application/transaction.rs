use crate::domain::catalog::Catalog;
use crate::domain::report::{ResourceReport, ResourceStatus};
use crate::domain::resource::{Ensure, ResourceProvider, ResourceState};
use crate::infrastructure::StateStore;
use anyhow::Result;
use std::sync::Arc;

pub async fn execute_transaction(
    catalog: Catalog,
    state_store: &StateStore,
    provider: Arc<dyn ResourceProvider>,
    dry_run: bool,
) -> Result<Vec<ResourceReport>> {
    let transaction_id = format!("tx_{}", chrono::Utc::now().timestamp());
    let mut transaction =
        crate::domain::transaction::Transaction::new(transaction_id.clone(), catalog.clone());

    let mut reports = Vec::new();

    tracing::debug!(id = %transaction_id, dry_run = %dry_run, "Starting transaction");

    for resource in &catalog.resources {
        // Skip Meta resources
        if let crate::domain::resource::Resource::Meta(_) = resource {
            continue;
        }

        // 1. Get current state
        let current_state = provider.get_state(resource, false).await?;
        transaction
            .original_states
            .insert(resource.id().to_string(), current_state.clone());

        // 2. Check if already in desired state (idempotency check)
        let is_already_correct = match (resource, &current_state) {
            (crate::domain::resource::Resource::File(f), ResourceState::Ensure(e)) => {
                f.ensure == *e
            }
            (crate::domain::resource::Resource::File(f), ResourceState::Full { ensure, .. }) => {
                f.ensure == *ensure
            }
            (crate::domain::resource::Resource::Directory(d), ResourceState::Ensure(e)) => {
                d.ensure == *e
            }
            (crate::domain::resource::Resource::Exec(_), ResourceState::Ensure(e)) => {
                *e == Ensure::Present
            }
            _ => false,
        };

        if is_already_correct {
            reports.push(ResourceReport::new(
                resource.id().to_string(),
                ResourceStatus::Unchanged,
                false,
            ));
            continue;
        }

        if dry_run {
            reports.push(ResourceReport::new(
                resource.id().to_string(),
                ResourceStatus::WouldApply,
                true,
            ));
            tracing::debug!(id = %resource.id(), "Would ensure resource");
            continue;
        }

        // 3. Apply changes
        if let Err(e) = provider.apply(resource).await {
            reports.push(
                ResourceReport::new(resource.id().to_string(), ResourceStatus::Failed, false)
                    .with_message(e.to_string()),
            );
            tracing::error!(id = %resource.id(), error = %e, "Failed to apply resource");
            state_store.save_transaction(&transaction)?;
            return Ok(reports); // Return what we have so far
        } else {
            reports.push(ResourceReport::new(
                resource.id().to_string(),
                ResourceStatus::Applied,
                true,
            ));
        }
    }

    if !dry_run {
        state_store.save_transaction(&transaction)?;
    }
    tracing::debug!(id = %transaction_id, "Transaction completed");
    Ok(reports)
}
