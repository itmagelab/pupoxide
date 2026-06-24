use super::utils::DslUtils;
use crate::application::engine::ExecutionContext;
use crate::domain::resource::{PackageResource, Resource};
use rhai::{Dynamic, Engine, Map, Module, NativeCallContext};
use tera::{Context, Tera};

fn convert_rhai_to_tera(val: &Dynamic) -> tera::Value {
    if let Some(s) = val.clone().try_cast::<String>() {
        tera::Value::String(s)
    } else if let Some(i) = val.clone().try_cast::<i64>() {
        tera::Value::Number(i.into())
    } else if let Some(f) = val.clone().try_cast::<f64>() {
        if let Some(num) = tera::Number::from_f64(f) {
            tera::Value::Number(num)
        } else {
            tera::Value::Null
        }
    } else if let Some(b) = val.clone().try_cast::<bool>() {
        tera::Value::Bool(b)
    } else if let Some(arr) = val.clone().try_cast::<rhai::Array>() {
        let mut t_arr = Vec::new();
        for item in arr {
            t_arr.push(convert_rhai_to_tera(&item));
        }
        tera::Value::Array(t_arr)
    } else if let Some(m) = val.clone().try_cast::<Map>() {
        let mut t_obj = tera::Map::new();
        for (k, v) in m {
            t_obj.insert(k.to_string(), convert_rhai_to_tera(&v));
        }
        tera::Value::Object(t_obj)
    } else {
        tera::Value::Null
    }
}

pub fn register(engine: &mut Engine) {
    let mut module = Module::new();

    module.set_native_fn("version", || Ok(env!("CARGO_PKG_VERSION").to_string()));

    // 'template' function
    module.set_native_fn(
        "template",
        move |_ctx: NativeCallContext,
              path: String,
              params: Map|
              -> std::result::Result<String, Box<rhai::EvalAltResult>> {
            let exec_ctx = ExecutionContext::get_current();

            // Resolve file path
            let current_p = exec_ctx.current_path.lock().map_err(|e| {
                Box::new(rhai::EvalAltResult::ErrorRuntime(
                    format!("Lock error: {}", e).into(),
                    rhai::Position::NONE,
                ))
            })?;

            let parent_dir = current_p.parent().unwrap_or(&current_p);
            let mut full_path = parent_dir.join(&path);

            // If path doesn't exist relative to current file, and it doesn't start with '.', check if it's in templates dir (optional fallback)
            // But usually we just resolve it relative to current manifest.
            if !full_path.exists() && !path.starts_with(".") {
                let alt_path = parent_dir.join("templates").join(&path);
                if alt_path.exists() {
                    full_path = alt_path;
                }
            }

            if !full_path.exists() {
                return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                    format!("Template file not found: {}", full_path.display()).into(),
                    rhai::Position::NONE,
                )));
            }

            let template_content = std::fs::read_to_string(&full_path).map_err(|e| {
                Box::new(rhai::EvalAltResult::ErrorRuntime(
                    format!("Failed to read template {}: {}", full_path.display(), e).into(),
                    rhai::Position::NONE,
                ))
            })?;

            let mut context = Context::new();

            // Insert params into context
            for (k, v) in params {
                context.insert(k.as_str(), &convert_rhai_to_tera(&v));
            }

            // Insert facts into context
            let mut facts_map = tera::Map::new();
            for (k, v) in exec_ctx.facts.values.iter() {
                facts_map.insert(k.clone(), tera::Value::String(v.clone()));
            }
            context.insert("facts", &tera::Value::Object(facts_map));

            let mut tera = Tera::default();
            tera.add_raw_template("main", &template_content)
                .map_err(|e| {
                    Box::new(rhai::EvalAltResult::ErrorRuntime(
                        format!("Failed to parse template: {}", e).into(),
                        rhai::Position::NONE,
                    ))
                })?;

            let rendered = tera.render("main", &context).map_err(|e| {
                Box::new(rhai::EvalAltResult::ErrorRuntime(
                    format!("Failed to render template: {}", e).into(),
                    rhai::Position::NONE,
                ))
            })?;

            Ok(rendered)
        },
    );

    // 'pkg' function
    module.set_native_fn(
        "pkg",
        move |ctx: NativeCallContext, name: String, params: Map| {
            let exec_ctx = ExecutionContext::get_current();
            let ensure = DslUtils::extract_ensure(&params);
            let dependencies =
                DslUtils::extract_dependencies(&params, &exec_ctx, ctx.call_source());
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

            exec_ctx.add_resource(resource).map_err(|e| {
                Box::new(rhai::EvalAltResult::ErrorRuntime(
                    e.to_string().into(),
                    rhai::Position::NONE,
                ))
            })
        },
    );

    let shared_module: std::sync::Arc<Module> = module.into();
    engine.register_static_module("stdlib", shared_module.clone());
    engine.register_static_module("std", shared_module);
}
