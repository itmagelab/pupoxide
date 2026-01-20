use crate::application::engine::{CURRENT_EXEC_CTX, ExecutionContext, InclusionType, ModuleHandle};
use crate::domain::resource::{Ensure, Resource};
use anyhow::Result;
use rhai::{Dynamic, Map};
use tracing::warn;

// Helper to lock a mutex safely or log warning
fn lock_or_warn<'a, T>(
    mutex: &'a std::sync::Mutex<T>,
    label: &str,
) -> Option<std::sync::MutexGuard<'a, T>> {
    mutex
        .lock()
        .map_err(|e| {
            warn!("Failed to lock {}: {}", label, e);
        })
        .ok()
}

pub struct DslContext;

impl DslContext {
    pub fn get_exec_ctx() -> ExecutionContext {
        CURRENT_EXEC_CTX.with(|ctx| {
            ctx.borrow()
                .clone()
                // SAFETY: Execution context must be set during Rhai evaluation.
                // This is ensured by the engine's run_manifest method.
                .expect("Execution context must be set during Rhai evaluation")
        })
    }

    pub fn add_resource(exec_ctx: &ExecutionContext, resource: Resource) -> Result<Resource> {
        // Enforce role constraints: Resources cannot be added directly in Roles
        {
            let current_type = exec_ctx
                .current_inclusion_type
                .lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock current inclusion type: {}", e))?;
            if *current_type == Some(InclusionType::Role) && !matches!(resource, Resource::Meta(_))
            {
                return Err(anyhow::anyhow!(
                    "Technical resources like '{}' are NOT allowed directly in Roles. Roles must ONLY include Profiles.",
                    resource.id()
                ));
            }
        }

        exec_ctx
            .resources
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock resources: {}", e))?
            .push(resource.clone());
        Ok(resource)
    }

    pub fn add_dependency_between_ids(lhs_id: &str, rhs_id: &str) {
        let exec_ctx = Self::get_exec_ctx();
        if let Some(mut resources) = lock_or_warn(&exec_ctx.resources, "resources")
            && let Some(res) = resources.iter_mut().find(|r| r.id() == rhs_id)
        {
            res.add_dependency(lhs_id.to_string());
        }
    }

    pub fn extract_ensure(params: &Map) -> Ensure {
        params
            .get("ensure")
            .and_then(|v| {
                v.clone()
                    .try_cast::<String>()
                    .map(|s| match s.as_str() {
                        "absent" => Ensure::Absent,
                        _ => Ensure::Present,
                    })
                    .or_else(|| v.clone().try_cast::<Ensure>())
            })
            .unwrap_or(Ensure::Present)
    }

    pub fn extract_dependencies(
        params: &Map,
        exec_ctx: &ExecutionContext,
        source: Option<&str>,
    ) -> Vec<String> {
        let mut dependencies = Vec::new();

        // 1. Parent attribution based on Rhai source
        if let Some(src) = source {
            let s_map = exec_ctx
                .source_map
                .lock()
                // SAFETY: Lock failure indicates a poisoned mutex, which is a fatal error.
                .expect("Failed to lock source map");

            // Try specific source mapping, then fallback to current module stack
            if let Some(marker_id) = s_map.get(src) {
                dependencies.push(marker_id.clone());
            } else if let Some(stack_guard) = lock_or_warn(&exec_ctx.module_stack, "module_stack")
                && let Some((inc_type, curr_mod)) = stack_guard.last()
            {
                dependencies.push(format!("{:?}Start[{}]", inc_type, curr_mod));
            }
        } else if let Some(stack_guard) = lock_or_warn(&exec_ctx.module_stack, "module_stack") {
            // General fallback if no source provided
            if let Some((inc_type, curr_mod)) = stack_guard.last() {
                dependencies.push(format!("{:?}Start[{}]", inc_type, curr_mod));
            }
        }

        // 2. Add explicit dependencies from params
        if let Some(req) = params.get("require") {
            Self::push_dependency(&mut dependencies, req.clone());
        }

        dependencies
    }

    pub fn push_dependency(dependencies: &mut Vec<String>, req: Dynamic) {
        // Try to extract dependency in order of specificity
        if let Some(m_h) = req.clone().try_cast::<ModuleHandle>() {
            dependencies.push(m_h.end_id);
        } else if let Some(dep_res) = req.clone().try_cast::<Resource>() {
            dependencies.push(dep_res.id().to_string());
        } else if let Some(dep_id) = req.clone().try_cast::<String>() {
            dependencies.push(dep_id);
        } else if let Some(m) = req.clone().try_cast::<rhai::Shared<rhai::Module>>() {
            if let Some(m_h) = m
                .get_var("module_handle")
                .and_then(|v| v.try_cast::<ModuleHandle>())
            {
                dependencies.push(m_h.end_id);
            }
        } else if let Some(arr) = req.try_cast::<rhai::Array>() {
            for item in arr {
                Self::push_dependency(dependencies, item);
            }
        }
    }

    pub fn extract_string(params: &Map, key: &str) -> Option<String> {
        params.get(key).and_then(|v| v.clone().try_cast::<String>())
    }

    pub fn extract_bool(params: &Map, key: &str, default: bool) -> bool {
        params
            .get(key)
            .and_then(|v| v.clone().try_cast::<bool>())
            .unwrap_or(default)
    }
}
