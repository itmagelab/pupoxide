use std::path::{Path, PathBuf};

/// Smart module path resolution
/// 1. If module_path is explicitly provided, use it.
/// 2. If manifest is in a 'manifests' directory, look for 'modules' sibling of 'manifests' parent.
/// 3. Otherwise, look for 'modules' sibling of the manifest itself.
pub fn resolve_module_path(file: &Path, module_path: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(mp) = module_path {
        Some(mp)
    } else {
        file.parent().and_then(|p| {
            let sibling_modules = if p.ends_with("manifests") {
                p.parent().map(|parent| parent.join("modules"))
            } else {
                Some(p.join("modules"))
            };
            sibling_modules.filter(|path| path.exists())
        })
    }
}
