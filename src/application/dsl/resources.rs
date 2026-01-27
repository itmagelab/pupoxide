use super::utils::DslUtils;
use crate::application::engine::ExecutionContext;
use crate::domain::resource::{ExecResource, FileResource, PackageResource, Resource};
use rhai::{Engine, Map, NativeCallContext};
use std::collections::HashMap;
use std::path::PathBuf;

// Helper to convert anyhow errors to Rhai errors
fn to_rhai_error(e: anyhow::Error) -> Box<rhai::EvalAltResult> {
    Box::new(rhai::EvalAltResult::ErrorRuntime(
        e.to_string().into(),
        rhai::Position::NONE,
    ))
}

// Helper to extract environment variables map from params
fn extract_environment(params: &Map) -> Option<HashMap<String, String>> {
    params
        .get("environment")
        .and_then(|v| v.clone().try_cast::<Map>())
        .map(|map| {
            map.into_iter()
                .filter_map(|(k, v)| v.try_cast::<String>().map(|s| (k.to_string(), s)))
                .collect::<HashMap<String, String>>()
        })
}

pub fn register(engine: &mut Engine) {
    // 'directory' function
    engine.register_fn(
        "directory",
        move |ctx: NativeCallContext,
              path: String,
              params: Map|
              -> std::result::Result<Resource, Box<rhai::EvalAltResult>> {
            let exec_ctx = ExecutionContext::get_current();
            let ensure = DslUtils::extract_ensure(&params);
            let dependencies = DslUtils::extract_dependencies(&params, &exec_ctx, ctx.call_source());
            let resource = Resource::Directory(crate::domain::resource::DirectoryResource {
                id: format!("Directory[{}]", path),
                path: PathBuf::from(path),
                ensure,
                dependencies,
                owner: DslUtils::extract_string(&params, "owner"),
                group: DslUtils::extract_string(&params, "group"),
                mode: DslUtils::extract_string(&params, "mode"),
                mutex: DslUtils::extract_string(&params, "mutex"),
                source_context: exec_ctx.get_source_context(),
            });

            exec_ctx.add_resource(resource).map_err(to_rhai_error)
        },
    );

    // 'exec' function
    engine.register_fn(
        "exec",
        move |ctx: NativeCallContext,
              id_or_command: String,
              params: Map|
              -> std::result::Result<Resource, Box<rhai::EvalAltResult>> {
            let exec_ctx = ExecutionContext::get_current();
            let dependencies = DslUtils::extract_dependencies(&params, &exec_ctx, ctx.call_source());

            let creates = DslUtils::extract_string(&params, "creates").map(PathBuf::from);
            let unless = DslUtils::extract_string(&params, "unless");
            let cwd = DslUtils::extract_string(&params, "cwd").map(PathBuf::from);
            let command = DslUtils::extract_string(&params, "command")
                .unwrap_or_else(|| id_or_command.clone());

            let resource = Resource::Exec(ExecResource {
                id: format!("Exec[{}]", id_or_command),
                command,
                creates,
                unless,
                cwd,
                environment: extract_environment(&params),
                dependencies,
                mutex: DslUtils::extract_string(&params, "mutex"),
                source_context: exec_ctx.get_source_context(),
            });

            exec_ctx.add_resource(resource).map_err(to_rhai_error)
        },
    );

    // 'file' function
    engine.register_fn(
        "file",
        move |ctx: NativeCallContext,
              path: String,
              params: Map|
              -> std::result::Result<Resource, Box<rhai::EvalAltResult>> {
            let exec_ctx = ExecutionContext::get_current();
            let ensure = DslUtils::extract_ensure(&params);
            let dependencies = DslUtils::extract_dependencies(&params, &exec_ctx, ctx.call_source());
            let content = DslUtils::extract_string(&params, "content");

            let resource = Resource::File(FileResource {
                id: format!("File[{}]", path),
                path: PathBuf::from(path),
                ensure,
                content,
                dependencies,
                owner: DslUtils::extract_string(&params, "owner"),
                group: DslUtils::extract_string(&params, "group"),
                mode: DslUtils::extract_string(&params, "mode"),
                mutex: DslUtils::extract_string(&params, "mutex"),
                source_context: exec_ctx.get_source_context(),
            });

            exec_ctx.add_resource(resource).map_err(to_rhai_error)
        },
    );

    // 'pkg_resource' function
    engine.register_fn(
        "pkg_resource",
        move |ctx: NativeCallContext,
              name: String,
              params: Map|
              -> std::result::Result<Resource, Box<rhai::EvalAltResult>> {
            let exec_ctx = ExecutionContext::get_current();
            let ensure = DslUtils::extract_ensure(&params);
            let dependencies = DslUtils::extract_dependencies(&params, &exec_ctx, ctx.call_source());
            let provider = DslUtils::extract_string(&params, "provider")
                .unwrap_or_else(|| exec_ctx.get_default_provider());

            // Automatic mutex based on provider
            let mutex =
                DslUtils::extract_string(&params, "mutex").unwrap_or_else(|| provider.clone());

            let resource = Resource::Package(PackageResource {
                id: format!("Package[{}]", name),
                name,
                ensure,
                provider,
                dependencies,
                mutex: Some(mutex),
                source_context: exec_ctx.get_source_context(),
            });

            exec_ctx.add_resource(resource).map_err(to_rhai_error)
        },
    );
}
