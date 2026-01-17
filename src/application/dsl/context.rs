use crate::application::engine::{CURRENT_EXEC_CTX, ExecutionContext, ModuleHandle};
use crate::domain::resource::{Ensure, Resource};
use rhai::{Dynamic, Map};
use tracing::warn;
use anyhow::Result;

pub struct DslContext;

impl DslContext {
    pub fn get_exec_ctx() -> ExecutionContext {
        CURRENT_EXEC_CTX.with(|ctx| {
            ctx.borrow()
                .clone()
                .expect("Execution context must be set during Rhai evaluation")
        })
    }

    pub fn add_resource(
        exec_ctx: &ExecutionContext,
        resource: Resource,
    ) -> Result<Resource> {
        // Enforce role constraints: Resources cannot be added directly in Roles
        {
            let current_type = exec_ctx
                .current_inclusion_type
                .lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock current inclusion type: {}", e))?;
            if *current_type == Some(crate::application::engine::InclusionType::Role)
                && !matches!(resource, Resource::Meta(_))
            {
                return Err(anyhow::anyhow!("Technical resources like '{}' are NOT allowed directly in Roles. Roles must ONLY include Profiles.", resource.id()));
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
        let mut resources = match exec_ctx.resources.lock() {
            Ok(guard) => guard,
            Err(e) => {
                warn!("Failed to lock resources: {}", e);
                return;
            }
        };
        if let Some(res) = resources.iter_mut().find(|r| r.id() == rhs_id) {
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

    pub fn extract_dependencies(params: &Map, exec_ctx: &ExecutionContext) -> Vec<String> {
        let mut dependencies = Vec::new();

        // Get current module and inclusion type
        let current_info = {
            let stack = match exec_ctx.module_stack.lock() {
                Ok(guard) => guard,
                Err(e) => {
                    warn!("Failed to lock module stack: {}", e);
                    return dependencies;
                }
            };
                
            let current_type = match exec_ctx.current_inclusion_type.lock() {
                Ok(guard) => guard,
                Err(e) => {
                    warn!("Failed to lock current inclusion type: {}", e);
                    return dependencies;
                }
            };

            let current_module = stack.last().cloned();
            let inclusion_type = current_type.clone().unwrap_or(crate::application::engine::InclusionType::Module);

            (current_module, inclusion_type)
        };

        if let (Some(curr_mod), curr_type) = current_info {
            dependencies.push(format!("{:?}Start[{}]", curr_type, curr_mod));
        }

        if let Some(req) = params.get("require") {
            Self::push_dependency(&mut dependencies, req.clone());
        }

        dependencies
    }

    pub fn push_dependency(dependencies: &mut Vec<String>, req: Dynamic) {
        if let Some(dep_res) = req.clone().try_cast::<Resource>() {
            dependencies.push(dep_res.id().to_string());
        } else if let Some(dep_id) = req.clone().try_cast::<String>() {
            dependencies.push(dep_id);
        } else if let Some(m_h) = req.clone().try_cast::<ModuleHandle>() {
            dependencies.push(m_h.end_id);
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
