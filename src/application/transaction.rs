use crate::domain::catalog::Catalog;
use crate::domain::report::{ResourceReport, ResourceStatus};
use crate::domain::resource::{Ensure, Resource, ResourceProvider, ResourceState};
use crate::infrastructure::StateStore;
use anyhow::Result;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

pub async fn execute_transaction(
    catalog: Catalog,
    state_store: &StateStore,
    provider: Arc<dyn ResourceProvider>,
    dry_run: bool,
    mut on_report: impl FnMut(&ResourceReport),
) -> Result<Vec<ResourceReport>> {
    let transaction_id = format!("tx_{}", chrono::Utc::now().timestamp());
    let transaction = Arc::new(Mutex::new(crate::domain::transaction::Transaction::new(
        transaction_id.clone(),
        catalog.clone(),
    )));

    let total_start = std::time::Instant::now();
    tracing::debug!(id = %transaction_id, dry_run = %dry_run, "Starting transaction");

    // 1. Build dependency graph
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();
    let mut pending_deps: HashMap<String, usize> = HashMap::new();
    let mut resource_map: HashMap<String, Resource> = HashMap::new();
    let mut reports_order: Vec<String> = Vec::new();

    // Initialize maps
    for resource in &catalog.resources {
        let id = resource.id().to_string();
        resource_map.insert(id.clone(), resource.clone());
        reports_order.push(id.clone());

        let deps = resource.dependencies();
        pending_deps.insert(id.clone(), deps.len());

        for dep in deps {
            dependents.entry(dep.clone()).or_default().push(id.clone());
        }
    }

    // 2. Ready queue: resources with 0 pending dependencies
    let mut ready_queue: VecDeque<String> = catalog
        .resources
        .iter()
        .filter(|r| r.dependencies().is_empty())
        .map(|r| r.id().to_string())
        .collect();

    let (tx, mut rx) = mpsc::channel(catalog.resources.len());
    let mut running_tasks: usize = 0;
    let mut completed_count = 0;
    let total_resources = catalog.resources.len();

    let mut reports: HashMap<String, ResourceReport> = HashMap::new();
    let mut failed_roots: HashSet<String> = HashSet::new();

    while completed_count < total_resources {
        // 1. Spawn tasks for ready resources or process them immediately
        while let Some(res_id) = ready_queue.pop_front() {
            let resource = resource_map
                .get(&res_id)
                .cloned()
                .expect("Resource must exist");

            // If any dependency failed, this resource is skipped
            let mut dependency_failed = false;
            for dep in resource.dependencies() {
                if failed_roots.contains(dep) {
                    dependency_failed = true;
                    break;
                }
            }

            if dependency_failed {
                failed_roots.insert(res_id.clone());
                let source_context = get_source_context(&resource);
                let report = ResourceReport::new(res_id.clone(), ResourceStatus::Skipped, false)
                    .with_message("Dependency failed".to_string())
                    .with_source_context(source_context);

                reports.insert(res_id.clone(), report.clone());
                on_report(&report);
                completed_count += 1;

                // Update dependents immediately
                update_dependents(&res_id, &dependents, &mut pending_deps, &mut ready_queue);
                continue;
            }

            // Handle Meta resources immediately - skip reporting
            if let Resource::Meta(_) = resource {
                let report = ResourceReport::new(res_id.clone(), ResourceStatus::Unchanged, false);
                reports.insert(res_id.clone(), report);
                completed_count += 1;

                update_dependents(&res_id, &dependents, &mut pending_deps, &mut ready_queue);
                continue;
            }

            let provider = Arc::clone(&provider);
            let transaction = Arc::clone(&transaction);
            let tx_clone = tx.clone();

            running_tasks += 1;
            tokio::spawn(async move {
                let report =
                    process_single_resource(resource, provider, transaction, dry_run).await;
                if let Err(e) = tx_clone.send((res_id.clone(), report)).await {
                    tracing::error!(id = %res_id, error = %e, "Failed to send report to main thread");
                }
            });
        }

        if completed_count == total_resources {
            break;
        }

        if running_tasks == 0 && ready_queue.is_empty() {
            // Safety break for cycles or unhandled cases
            break;
        }

        // 2. Wait for a task to complete
        if let Some((res_id, report)) = rx.recv().await {
            completed_count += 1;
            running_tasks = running_tasks.saturating_sub(1);

            if report.status == ResourceStatus::Failed {
                failed_roots.insert(res_id.clone());
            }

            on_report(&report);
            reports.insert(res_id.clone(), report);

            // Update dependents
            update_dependents(&res_id, &dependents, &mut pending_deps, &mut ready_queue);
        }
    }

    // Double check for any missed resources
    for id in reports_order.iter() {
        if !reports.contains_key(id) {
            let resource = resource_map.get(id).expect("Exists");
            let source_context = get_source_context(resource);

            // Check if it was supposed to be skipped due to a failed parent
            let parent_failed = resource
                .dependencies()
                .iter()
                .any(|d| failed_roots.contains(d));
            let msg = if parent_failed {
                "Dependency failed".to_string()
            } else {
                "Dependency cycle or unhandled state".to_string()
            };

            let report = ResourceReport::new(id.clone(), ResourceStatus::Skipped, false)
                .with_message(msg)
                .with_source_context(source_context);
            reports.insert(id.clone(), report);
        }
    }

    // Filter out Meta resources from the final reports list
    let final_reports: Vec<ResourceReport> = reports_order
        .iter()
        .filter(|id| {
            let res = resource_map.get(*id).expect("Exists");
            !matches!(res, Resource::Meta(_))
        })
        .map(|id| {
            reports
                .get(id)
                .cloned()
                .expect("All resources must have reports")
        })
        .collect();

    if !dry_run {
        let tx_guard = transaction.lock().await;
        state_store.save_transaction(&tx_guard)?;
    }

    tracing::debug!(id = %transaction_id, "Transaction completed");
    crate::interface::formatter::PrettyFormatter::print_summary(
        &final_reports,
        total_start.elapsed(),
    );
    Ok(final_reports)
}

fn update_dependents(
    res_id: &str,
    dependents: &HashMap<String, Vec<String>>,
    pending_deps: &mut HashMap<String, usize>,
    ready_queue: &mut VecDeque<String>,
) {
    if let Some(deps) = dependents.get(res_id) {
        for dependent_id in deps {
            if let Some(count) = pending_deps.get_mut(dependent_id) {
                *count -= 1;
                if *count == 0 {
                    ready_queue.push_back(dependent_id.clone());
                }
            }
        }
    }
}

async fn process_single_resource(
    resource: Resource,
    provider: Arc<dyn ResourceProvider>,
    transaction: Arc<Mutex<crate::domain::transaction::Transaction>>,
    dry_run: bool,
) -> ResourceReport {
    let start_time = std::time::Instant::now();
    let source_context = get_source_context(&resource);
    let id = resource.id().to_string();

    // 1. Get current state
    let current_state = match provider.get_state(&resource, false).await {
        Ok(s) => s,
        Err(e) => {
            return ResourceReport::new(id, ResourceStatus::Failed, false)
                .with_message(format!("Failed to get state: {}", e))
                .with_duration(start_time.elapsed())
                .with_source_context(source_context);
        }
    };

    {
        let mut tx_guard = transaction.lock().await;
        tx_guard
            .original_states
            .insert(id.clone(), current_state.clone());
    }

    // 2. Check if already in desired state (idempotency check)
    let is_already_correct = match (&resource, &current_state) {
        (Resource::File(f), ResourceState::Ensure(e)) => f.ensure == *e,
        (Resource::File(f), ResourceState::Full { ensure, .. }) => f.ensure == *ensure,
        (Resource::Directory(d), ResourceState::Ensure(e)) => d.ensure == *e,
        (Resource::Exec(_), ResourceState::Ensure(e)) => *e == Ensure::Present,
        _ => false,
    };

    if is_already_correct {
        return ResourceReport::new(id, ResourceStatus::Unchanged, false)
            .with_duration(start_time.elapsed())
            .with_source_context(source_context);
    }

    if dry_run {
        tracing::debug!(id = %id, "Would ensure resource");
        return ResourceReport::new(id, ResourceStatus::WouldApply, true)
            .with_duration(start_time.elapsed())
            .with_source_context(source_context);
    }

    // 3. Apply changes
    match provider.apply(&resource).await {
        Err(e) => {
            tracing::error!(id = %id, error = %e, "Failed to apply resource");
            ResourceReport::new(id, ResourceStatus::Failed, false)
                .with_message(e.to_string())
                .with_duration(start_time.elapsed())
                .with_source_context(source_context)
        }
        Ok(_) => ResourceReport::new(id, ResourceStatus::Applied, true)
            .with_duration(start_time.elapsed())
            .with_source_context(source_context),
    }
}

fn get_source_context(resource: &Resource) -> Option<crate::domain::resource::SourceContext> {
    match resource {
        Resource::File(f) => f.source_context.clone(),
        Resource::Directory(d) => d.source_context.clone(),
        Resource::Exec(e) => e.source_context.clone(),
        _ => None,
    }
}
