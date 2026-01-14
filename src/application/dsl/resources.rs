use rhai::{Engine, Map};
use std::collections::HashMap;
use std::path::PathBuf;
use crate::domain::resource::{ExecResource, FileResource, Resource};
use super::context::DslContext;

pub fn register(engine: &mut Engine) {
    // 'directory' function
    engine.register_fn(
        "directory",
        move |path: String,
              params: Map|
              -> std::result::Result<Resource, Box<rhai::EvalAltResult>> {
            let exec_ctx = DslContext::get_exec_ctx();
            let ensure = DslContext::extract_ensure(&params);
            let dependencies = DslContext::extract_dependencies(&params, &exec_ctx);
            let backup = DslContext::extract_bool(&params, "backup", false);

            let resource = Resource::Directory(crate::domain::resource::DirectoryResource {
                id: format!("Directory[{}]", path),
                path: PathBuf::from(path),
                ensure,
                dependencies,
                backup,
                owner: DslContext::extract_string(&params, "owner"),
                group: DslContext::extract_string(&params, "group"),
                mode: DslContext::extract_string(&params, "mode"),
            });

            DslContext::add_resource(&exec_ctx, resource)
        },
    );

    // 'exec' function
    engine.register_fn(
        "exec",
        move |id_or_command: String,
              params: Map|
              -> std::result::Result<Resource, Box<rhai::EvalAltResult>> {
            let exec_ctx = DslContext::get_exec_ctx();
            let dependencies = DslContext::extract_dependencies(&params, &exec_ctx);

            let creates = DslContext::extract_string(&params, "creates").map(PathBuf::from);
            let unless = DslContext::extract_string(&params, "unless");
            let cwd = DslContext::extract_string(&params, "cwd").map(PathBuf::from);

            let explicit_command = DslContext::extract_string(&params, "command");
            let command = explicit_command.unwrap_or_else(|| id_or_command.clone());

            let environment = params
                .get("environment")
                .and_then(|v| v.clone().try_cast::<Map>())
                .map(|map| {
                    map.into_iter()
                        .filter_map(|(k, v)| v.try_cast::<String>().map(|s| (k.to_string(), s)))
                        .collect::<HashMap<String, String>>()
                });

            let resource = Resource::Exec(ExecResource {
                id: format!("Exec[{}]", id_or_command),
                command,
                creates,
                unless,
                cwd,
                environment,
                dependencies,
                backup: false,
            });

            DslContext::add_resource(&exec_ctx, resource)
        },
    );

    // 'file' function
    engine.register_fn(
        "file",
        move |path: String,
              params: Map|
              -> std::result::Result<Resource, Box<rhai::EvalAltResult>> {
            let exec_ctx = DslContext::get_exec_ctx();
            let ensure = DslContext::extract_ensure(&params);
            let dependencies = DslContext::extract_dependencies(&params, &exec_ctx);
            let backup = DslContext::extract_bool(&params, "backup", true);

            let content = DslContext::extract_string(&params, "content");
            let max_backup_size = params.get("max_backup_size").and_then(|v| {
                v.clone()
                    .try_cast::<i64>()
                    .map(|i| i as u64)
                    .or_else(|| v.clone().try_cast::<u64>())
            });

            let resource = Resource::File(FileResource {
                id: format!("File[{}]", path),
                path: PathBuf::from(path),
                ensure,
                content,
                dependencies,
                backup,
                max_backup_size,
                owner: DslContext::extract_string(&params, "owner"),
                group: DslContext::extract_string(&params, "group"),
                mode: DslContext::extract_string(&params, "mode"),
            });

            DslContext::add_resource(&exec_ctx, resource)
        },
    );
}
