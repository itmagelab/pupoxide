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
    fs::write(&site_rhai, script).unwrap();

    // 1. Load environment
    let loader = EnvironmentLoader::new(base_dir.path().to_path_buf());
    let manifest_path = loader.get_site_manifest("prod").unwrap();

    // 2. Execute Rhai
    let engine = PupoxideEngine::new();
    let resources = engine.run_manifest(manifest_path).unwrap();

    // Should have 2 resources in order: Dir then File
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
