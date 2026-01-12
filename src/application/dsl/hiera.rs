use rhai::{Dynamic, Engine};
use std::sync::Arc;
use crate::infrastructure::hiera::Hiera;
use crate::application::engine::CURRENT_EXEC_CTX;

pub fn register(engine: &mut Engine, hiera: Arc<Option<Hiera>>) {
    let h = hiera.clone();
    engine.register_fn("lookup", move |key: String| -> Dynamic {
        lookup_internal(&h, key, None)
    });

    let h2 = hiera.clone();
    engine.register_fn(
        "lookup",
        move |key: String, default_val: Dynamic| -> Dynamic {
            lookup_internal(&h2, key, Some(default_val))
        },
    );
}

fn lookup_internal(
    hiera: &Option<Hiera>,
    key: String,
    default_val: Option<Dynamic>,
) -> Dynamic {
    let exec_ctx =
        CURRENT_EXEC_CTX.with(|ctx| ctx.borrow().clone().expect("Execution context must be set"));

    if let Some(hiera_impl) = hiera.as_ref() {
        if let Some(val) = hiera_impl.lookup(&key, &exec_ctx.facts) {
            return match val {
                serde_yaml::Value::String(s) => Dynamic::from(s),
                serde_yaml::Value::Bool(b) => Dynamic::from(b),
                serde_yaml::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        Dynamic::from(i)
                    } else if let Some(f) = n.as_f64() {
                        Dynamic::from(f)
                    } else {
                        Dynamic::from(n.to_string())
                    }
                }
                _ => Dynamic::from(serde_yaml::to_string(&val).unwrap_or_default()),
            };
        }
    }
    default_val.unwrap_or(Dynamic::UNIT)
}
