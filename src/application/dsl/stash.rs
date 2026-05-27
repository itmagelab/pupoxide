use crate::application::engine::CURRENT_EXEC_CTX;
use crate::application::StashProvider;
use rhai::{Dynamic, Engine};
use std::sync::Arc;

pub fn register(engine: &mut Engine, stash: Option<Arc<dyn StashProvider>>) {
    let s = stash.clone();
    engine.register_fn("lookup", move |key: String| -> Dynamic {
        lookup_internal(&s, key, None)
    });

    let s2 = stash.clone();
    engine.register_fn(
        "lookup",
        move |key: String, default_val: Dynamic| -> Dynamic {
            lookup_internal(&s2, key, Some(default_val))
        },
    );
}

fn lookup_internal(stash: &Option<Arc<dyn StashProvider>>, key: String, default_val: Option<Dynamic>) -> Dynamic {
    let exec_ctx =
        CURRENT_EXEC_CTX.with(|ctx| ctx.borrow().clone().expect("Execution context must be set"));

    if let Some(stash_impl) = stash.as_ref()
        && let Some(val) = stash_impl.lookup(&key, &exec_ctx.facts)
    {
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
    default_val.unwrap_or(Dynamic::UNIT)
}
