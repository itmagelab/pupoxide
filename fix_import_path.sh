sed -i 's/\*th_ctx.borrow_mut() = None;/*th_ctx.borrow_mut() = Some(exec_ctx.clone());/' src/application/dsl/composition.rs
