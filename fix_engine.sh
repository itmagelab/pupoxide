sed -i 's/\*ctx.borrow_mut() = None;/\*ctx.borrow_mut() = Some(exec_ctx.clone());/' src/application/engine.rs
