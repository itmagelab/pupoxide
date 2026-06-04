use crate::application::engine::{ExecutionContext, ModuleHandle};
use crate::domain::resource::{Ensure, Resource};
use rhai::{Dynamic, Map};
use tracing::warn;

// Helper to lock a mutex safely or log warning
pub fn lock_or_warn<'a, T>(
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

pub struct DslUtils;

impl DslUtils {
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
        ctx: &ExecutionContext,
        source: Option<&str>,
    ) -> Vec<String> {
        let mut dependencies = Vec::new();

        // 1. Parent attribution based on Rhai source
        if let Some(src) = source {
            if let Some(state) = lock_or_warn(&ctx.state, "state") {
                // Try specific source mapping, then fallback to current module stack
                if let Some(marker_id) = state.source_map.get(src) {
                    dependencies.push(marker_id.clone());
                } else if let Some(inc) = state.module_stack.last() {
                    dependencies.push(format!("{:?}Start[{}]", inc.inclusion_type, inc.name));
                }
            }
        } else if let Some(state) = lock_or_warn(&ctx.state, "state") {
            // General fallback if no source provided
            if let Some(inc) = state.module_stack.last() {
                dependencies.push(format!("{:?}Start[{}]", inc.inclusion_type, inc.name));
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

    pub fn extract_resource_links(params: &Map, key: &str) -> Vec<String> {
        let mut links = Vec::new();
        if let Some(val) = params.get(key) {
            Self::push_dependency(&mut links, val.clone());
        }
        links
    }
}
