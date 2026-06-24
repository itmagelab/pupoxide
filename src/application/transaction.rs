use crate::domain::catalog::Catalog;
use crate::domain::report::{ResourceReport, ResourceStatus};
use crate::domain::resource::{Ensure, Resource, ResourceProvider, ResourceState};
use anyhow::Result;

/// Trait (Port) abstracting transaction history and persistence.
///
/// Implementations handle saving and loading transactions to/from persistent storage
/// (e.g., local files, database) to track configuration history and states.
pub trait StateStore: Send + Sync {
    /// Persists a transaction to the storage.
    fn save_transaction(&self, transaction: &crate::domain::transaction::Transaction)
    -> Result<()>;
    /// Loads a specific transaction by its unique identifier.
    fn load_transaction(&self, id: &str) -> Result<crate::domain::transaction::Transaction>;
    /// Loads the most recently applied transaction.
    fn load_latest_transaction(&self) -> Result<crate::domain::transaction::Transaction>;
}
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

struct ExecutionGraph {
    resource_map: HashMap<String, Resource>,
    dependents: HashMap<String, Vec<String>>,
    pending_deps: HashMap<String, usize>,
    reports_order: Vec<String>,
}

impl ExecutionGraph {
    fn new(catalog: &Catalog) -> Self {
        let mut dependents: HashMap<String, Vec<String>> = HashMap::new();
        let mut pending_deps: HashMap<String, usize> = HashMap::new();
        let mut resource_map: HashMap<String, Resource> = HashMap::new();
        let mut reports_order: Vec<String> = Vec::new();

        let resources = catalog.resources();
        for resource in &resources {
            let id = resource.id().to_string();
            resource_map.insert(id.clone(), resource.clone());
            reports_order.push(id.clone());

            let deps = resource.dependencies();
            pending_deps.insert(id.clone(), deps.len());

            for dep in deps {
                dependents.entry(dep.clone()).or_default().push(id.clone());
            }
        }

        Self {
            resource_map,
            dependents,
            pending_deps,
            reports_order,
        }
    }

    fn get_ready_resources(&self) -> VecDeque<String> {
        self.reports_order
            .iter()
            .filter(|id| self.pending_deps.get(*id) == Some(&0))
            .cloned()
            .collect()
    }

    fn update_dependents(&mut self, res_id: &str) -> Vec<String> {
        let mut newly_ready = Vec::new();
        if let Some(deps) = self.dependents.get(res_id) {
            for dependent_id in deps {
                if let Some(count) = self.pending_deps.get_mut(dependent_id) {
                    *count -= 1;
                    if *count == 0 {
                        newly_ready.push(dependent_id.clone());
                    }
                }
            }
        }
        newly_ready
    }
}

struct ExecutionState {
    reports: HashMap<String, ResourceReport>,
    failed_roots: HashSet<String>,
    completed_count: usize,
    running_tasks: usize,
}

impl ExecutionState {
    fn new() -> Self {
        Self {
            reports: HashMap::new(),
            failed_roots: HashSet::new(),
            completed_count: 0,
            running_tasks: 0,
        }
    }

    fn is_dependency_failed(&self, resource: &Resource) -> Option<String> {
        let failed_deps: Vec<_> = resource
            .dependencies()
            .iter()
            .filter(|d| self.failed_roots.contains(*d))
            .cloned()
            .collect();

        if failed_deps.is_empty() {
            None
        } else if failed_deps.len() == 1 {
            Some(format!("Dependency failed: {}", failed_deps[0]))
        } else {
            Some(format!("Dependencies failed: {}", failed_deps.join(", ")))
        }
    }

    fn record_completion(&mut self, res_id: String, report: ResourceReport) {
        if report.status == ResourceStatus::Failed {
            self.failed_roots.insert(res_id.clone());
        }
        self.reports.insert(res_id, report);
        self.completed_count += 1;
    }
}

/// Executes a transaction to apply the resource catalog.
///
/// This method applies resources in parallel on the target system, strictly respecting the dependency order.
/// The execution uses a parallel Kahn-like topological sort algorithm (ready-queue approach):
/// 1. All resources with zero active dependencies are placed in the `ready_queue`.
/// 2. For each ready resource, a concurrent asynchronous task is spawned using `tokio::spawn`.
/// 3. Resources with the same `mutex_id` (e.g. package managers like `brew` or `apt`) are executed sequentially via an internal mutex pool to prevent conflicts.
/// 4. Upon successful completion of a resource, its dependent children decrement their expected connection counters. Fully unlocked resources are pushed to the queue.
/// 5. If any parent resource fails (`Failed`), all its transitive descendants are recursively marked as `Skipped` (cascade failure/skip propagation).
///
/// # Example:
/// ```rust,ignore
/// let reports = execute_transaction(catalog, &state_store, provider, false, |report| {
///     println!("Resource [{}] applied with status {:?}", report.resource_id, report.status);
/// }).await?;
/// ```
pub async fn execute_transaction(
    catalog: Catalog,
    state_store: &dyn StateStore,
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

    let mut graph = ExecutionGraph::new(&catalog);
    let mut state = ExecutionState::new();
    let mut ready_queue = graph.get_ready_resources();
    let total_resources = graph.resource_map.len();

    let (tx, mut rx) = mpsc::channel(total_resources + 1);
    let mutex_pool: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    while state.completed_count < total_resources {
        // 1. Process ready resources
        while let Some(res_id) = ready_queue.pop_front() {
            let resource = graph
                .resource_map
                .get(&res_id)
                .expect("Resource must exist")
                .clone();

            // Check for failed dependencies
            if let Some(error_msg) = state.is_dependency_failed(&resource) {
                let report = ResourceReport::new(res_id.clone(), ResourceStatus::Skipped, false)
                    .with_message(error_msg)
                    .with_source_context(get_source_context(&resource));

                on_report(&report);
                state.record_completion(res_id.clone(), report);
                state.failed_roots.insert(res_id.clone()); // Propagation

                ready_queue.extend(graph.update_dependents(&res_id));
                continue;
            }

            // Handle Meta resources immediately
            if let Resource::Meta(_) = resource {
                let report = ResourceReport::new(res_id.clone(), ResourceStatus::Unchanged, false);
                state.record_completion(res_id.clone(), report);
                ready_queue.extend(graph.update_dependents(&res_id));
                continue;
            }

            // Spawn execution task
            let provider = Arc::clone(&provider);
            let transaction = Arc::clone(&transaction);
            let mutex_pool = Arc::clone(&mutex_pool);
            let tx_clone = tx.clone();

            state.running_tasks += 1;
            tokio::spawn(async move {
                let mutex_guard = if let Some(mutex_id) = resource.mutex() {
                    let mut pool = mutex_pool.lock().await;
                    let m = pool.entry(mutex_id.to_string()).or_default().clone();
                    drop(pool);
                    Some(m.lock_owned().await)
                } else {
                    None
                };

                let report =
                    process_single_resource(resource, provider, transaction, dry_run).await;
                drop(mutex_guard);

                let _ = tx_clone.send((res_id, report)).await;
            });
        }

        if state.completed_count == total_resources {
            break;
        }

        if state.running_tasks == 0 && ready_queue.is_empty() {
            break; // Potential cycle or deadlock
        }

        // 2. Wait for a task to complete
        if let Some((res_id, report)) = rx.recv().await {
            state.running_tasks -= 1;
            on_report(&report);
            state.record_completion(res_id.clone(), report);

            ready_queue.extend(graph.update_dependents(&res_id));
        }
    }

    // Double check for any missed resources (cycle protection)
    for id in &graph.reports_order {
        if !state.reports.contains_key(id) {
            let resource = graph.resource_map.get(id).expect("Exists");
            let report = ResourceReport::new(id.clone(), ResourceStatus::Skipped, false)
                .with_message("Dependency cycle or unhandled state".to_string())
                .with_source_context(get_source_context(resource));
            state.reports.insert(id.clone(), report);
        }
    }

    // Filter and prepare final reports
    let final_reports: Vec<ResourceReport> = graph
        .reports_order
        .iter()
        .filter(|id| !matches!(graph.resource_map.get(*id), Some(Resource::Meta(_))))
        .filter_map(|id| state.reports.get(id).cloned())
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

/// Processes a single system resource.
///
/// The resource processing algorithm guarantees idempotency:
/// 1. Queries the resource's current state on the target system (`provider.get_state`).
/// 2. Saves the original state in the transaction (`original_states`) for audit or rollback purposes.
/// 3. Performs a detailed comparison of the desired and actual states (idempotency check):
///    - If the states match, the resource is marked as `Unchanged` and does not affect the system.
///    - If the states differ and dry-run mode is active (`dry_run`), returns `WouldApply`.
/// 4. If there are mismatches in normal execution mode, calls `provider.apply` to bring the system to the desired state.
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
        (Resource::Package(p), ResourceState::Ensure(e)) => p.ensure == *e,
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
        Resource::Package(p) => p.source_context.clone(),
        Resource::Meta(_) => None,
    }
}
