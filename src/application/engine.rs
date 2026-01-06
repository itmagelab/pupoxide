use rhai::{Engine, Scope, Dynamic};
use std::path::PathBuf;
use crate::domain::resource::{Resource, FileResource, Ensure};
use crate::domain::error::{Result, DomainError};

pub struct PupoxideEngine {
    engine: Engine,
}

impl PupoxideEngine {
    pub fn new() -> Self {
        let mut engine = Engine::new();

        // Register Ensure
        engine.register_type_with_name::<Ensure>("Ensure")
              .register_fn("present", || Ensure::Present)
              .register_fn("absent", || Ensure::Absent);

        // Register Resource creation
        engine.register_fn("file", |path: String, ensure: Ensure, content: String| {
            Resource::File(FileResource {
                path: PathBuf::from(path),
                ensure,
                content: Some(content),
            })
        });

        engine.register_fn("file", |path: String, ensure: Ensure| {
            Resource::File(FileResource {
                path: PathBuf::from(path),
                ensure,
                content: None,
            })
        });

        Self { engine }
    }

    pub fn run_manifest(&self, path: PathBuf) -> Result<Vec<Resource>> {
        let mut scope = Scope::new();
        let ast = self.engine.compile_file(path)
            .map_err(|e| DomainError::Internal(format!("Rhai compilation error: {}", e)))?;
        
        let result: Dynamic = self.engine.eval_ast_with_scope(&mut scope, &ast)
            .map_err(|e| DomainError::Internal(format!("Rhai execution error: {}", e)))?;

        // If the script returns a list of resources as the final expression
        if result.is_array() {
            let res_array = result.into_typed_array::<Resource>()
                .map_err(|_| DomainError::Internal("Failed to cast Rhai result to Vec<Resource>".to_string()))?;
            Ok(res_array)
        } else if let Some(res) = result.try_cast::<Resource>() {
            Ok(vec![res])
        } else {
            // Check for a 'manifest' variable if script didn't return expression
            if let Some(manifest) = scope.get_value::<Dynamic>("manifest") {
                if manifest.is_array() {
                    return Ok(manifest.into_typed_array::<Resource>().unwrap_or_default());
                }
            }
            Ok(vec![])
        }
    }
}
