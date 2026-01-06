use pupoxide::application::{PupoxideEngine, EnvironmentLoader};
use pupoxide::infrastructure::FsAdapter;
use pupoxide::domain::resource::ResourceProvider;
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_environment_loading_and_execution() {
    let base_dir = tempdir().unwrap();
    let env_path = base_dir.path().join("environments").join("prod");
    let manifests_path = env_path.join("manifests");
    fs::create_dir_all(&manifests_path).unwrap();

    let site_rhai = manifests_path.join("site.rhai");
    let target_file = base_dir.path().join("site_result.txt");
    let target_file_str = target_file.to_str().unwrap();

    // Rhai script with dependencies and Maps
    let script = format!(
        r#"
        let f1 = file("{target_file_str}_dir", #{{ ensure: "present" }});
        f1 -> file("{target_file_str}", #{{ 
            ensure: "present", 
            content: "Config from simplified Rhai!" 
        }});
        "#
    );
    let manifest_path = manifests_path.join("site.rhai");
    fs::write(&manifest_path, script).unwrap();

    // 2. Execute Rhai
    let engine = PupoxideEngine::new();
    let catalog = engine.run_manifest(manifest_path, "test_node".to_string(), "prod".to_string()).unwrap();

    // Should have 2 resources in order: Dir then File
    let resources = catalog.resources;
    assert_eq!(resources.len(), 2);
    assert!(resources[0].id().contains("_dir"));
    assert_eq!(resources[1].id(), format!("File[{}]", target_file_str));

    // 3. Apply via adapter
    let adapter = FsAdapter;
    for res in resources {
        adapter.apply(&res).await.unwrap();
    }

    // 4. Verify result
    assert!(target_file.exists());
    let content = fs::read_to_string(&target_file).unwrap();
    assert_eq!(content, "Config from simplified Rhai!");
}

#[tokio::test]
async fn test_module_inclusion() {
    let base_dir = tempdir().unwrap();
    let env_dir = base_dir.path().join("environments").join("prod");
    let modules_dir = env_dir.join("modules");
    let module_manifest_dir = modules_dir.join("test_mod").join("manifests");
    let site_manifest_dir = env_dir.join("manifests");

    fs::create_dir_all(&module_manifest_dir).unwrap();
    fs::create_dir_all(&site_manifest_dir).unwrap();

    let target_file = base_dir.path().join("module_result.txt");
    let target_file_str = target_file.to_str().unwrap();

    // 1. Create module manifest
    let module_script = format!(
        r#"file("{target_file_str}", #{{ ensure: "present", content: "from module" }});"#
    );
    fs::write(module_manifest_dir.join("init.rhai"), module_script).unwrap();

    // 2. Create site manifest that includes the module
    let site_script = r#"include("test_mod");"#;
    let site_rhai = site_manifest_dir.join("site.rhai");
    fs::write(&site_rhai, site_script).unwrap();

    // 3. Execute
    let loader = EnvironmentLoader::new(base_dir.path().to_path_buf());
    let engine = PupoxideEngine::new();
    let catalog = engine.run_manifest_with_modules(
        site_rhai,
        loader.get_modules_path("prod"),
        "test_node".to_string(),
        "prod".to_string()
    ).unwrap();

    // 4. Verify
    let resources = catalog.resources;
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].id(), format!("File[{}]", target_file_str));
    
    let adapter = FsAdapter;
    for res in resources {
        adapter.apply(&res).await.unwrap();
    }
    
    assert!(target_file.exists());
    assert_eq!(fs::read_to_string(&target_file).unwrap(), "from module");
}
