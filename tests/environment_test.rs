use pupoxide::application::{EnvironmentLoader, PupoxideEngine};
use pupoxide::domain::resource::ResourceProvider;
use pupoxide::infrastructure::FsAdapter;
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_environment_loading_and_execution() {
    let base_dir = tempdir().expect("Test invariant failed");
    let env_path = base_dir.path().join("environments").join("prod");
    let manifests_path = env_path.join("manifests");
    fs::create_dir_all(&manifests_path).expect("Test invariant failed");

    let _site_rhai = manifests_path.join("site.rhai");
    let target_file = base_dir.path().join("site_result.txt");
    let target_file_str = target_file.to_str().expect("Test invariant failed");

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
    fs::write(&manifest_path, script).expect("Test invariant failed");

    // 2. Execute Rhai
    let engine = PupoxideEngine::new(None);
    let catalog = engine
        .run_manifest(
            manifest_path,
            "test_node".to_string(),
            "prod".to_string(),
            pupoxide::domain::Facts::default(),
        )
        .expect("Test invariant failed");

    // Should have 2 resources in order: Dir then File
    // ... (omitting unchanged lines in replacement for brevity if possible, but I'll provide full block)
    let resources = catalog.resources;
    assert_eq!(resources.len(), 2);
    assert!(resources[0].id().contains("_dir"));
    assert_eq!(resources[1].id(), format!("File[{}]", target_file_str));

    // 3. Apply via adapter
    let adapter = FsAdapter;
    for res in resources {
        adapter.apply(&res).await.expect("Test invariant failed");
    }

    // 4. Verify result
    assert!(target_file.exists());
    let content = fs::read_to_string(&target_file).expect("Test invariant failed");
    assert_eq!(content, "Config from simplified Rhai!");
}

#[tokio::test]
async fn test_module_inclusion() {
    let base_dir = tempdir().expect("Test invariant failed");
    let env_dir = base_dir.path().join("environments").join("prod");
    let modules_dir = env_dir.join("modules");
    let module_manifest_dir = modules_dir.join("test_mod").join("manifests");
    let site_manifest_dir = env_dir.join("manifests");

    fs::create_dir_all(&module_manifest_dir).expect("Test invariant failed");
    fs::create_dir_all(&site_manifest_dir).expect("Test invariant failed");

    let target_file = base_dir.path().join("module_result.txt");
    let target_file_str = target_file.to_str().expect("Test invariant failed");

    // 1. Create module manifest
    let module_script =
        format!(r#"file("{target_file_str}", #{{ ensure: "present", content: "from module" }});"#);
    fs::write(module_manifest_dir.join("init.rhai"), module_script).expect("Test invariant failed");

    // 2. Create site manifest that includes the module
    let site_script = r#"include("test_mod");"#;
    let site_rhai = site_manifest_dir.join("site.rhai");
    fs::write(&site_rhai, site_script).expect("Test invariant failed");

    // 3. Execute
    let loader = EnvironmentLoader::new(base_dir.path().to_path_buf());
    let engine = PupoxideEngine::new(None);
    let catalog = engine
        .run_manifest_with_modules(
            site_rhai,
            loader.get_modules_path("prod"),
            "test_node".to_string(),
            "prod".to_string(),
            pupoxide::domain::Facts::default(),
        )
        .expect("Test invariant failed");

    // 4. Verify
    let resources = catalog.resources;
    // Expected: ModuleStart, File, ModuleEnd
    assert_eq!(resources.len(), 3);
    assert!(
        resources
            .iter()
            .any(|r| r.id() == format!("File[{}]", target_file_str))
    );

    let adapter = FsAdapter;
    for res in resources {
        adapter.apply(&res).await.expect("Test invariant failed");
    }

    assert!(target_file.exists());
    assert_eq!(fs::read_to_string(&target_file).expect("Test invariant failed"), "from module");
}

#[tokio::test]
async fn test_facts_availability_in_rhai() {
    let base_dir = tempdir().expect("Test invariant failed");
    let site_rhai = base_dir.path().join("site.rhai");
    let target_file = base_dir.path().join("facts_result.txt");
    let target_file_str = target_file.to_str().expect("Test invariant failed");

    let script = format!(
        r#"file("{target_file_str}", #{{ ensure: "present", content: "Hostname is " + facts.hostname }});"#
    );
    fs::write(&site_rhai, script).expect("Test invariant failed");

    let mut facts = pupoxide::domain::Facts::new();
    facts.insert("hostname".to_string(), "test_host".to_string());

    let engine = PupoxideEngine::new(None);
    let catalog = engine
        .run_manifest(site_rhai, "node1".to_string(), "env1".to_string(), facts)
        .expect("Test invariant failed");

    let adapter = FsAdapter;
    adapter
        .apply(
            catalog
                .resources
                .iter()
                .find(|r| r.id().contains("facts_result"))
                .expect("Test invariant failed"),
        )
        .await
        .expect("Test invariant failed");

    let content = fs::read_to_string(&target_file).expect("Test invariant failed");
    assert_eq!(content, "Hostname is test_host");
}

#[tokio::test]
async fn test_module_dependency_chain() {
    let base_dir = tempdir().expect("Test invariant failed");
    let env_dir = base_dir.path().join("environments").join("prod");
    let modules_dir = env_dir.join("modules");
    let site_manifest_dir = env_dir.join("manifests");

    fs::create_dir_all(&site_manifest_dir).expect("Test invariant failed");

    // Create Module A
    let mod_a_dir = modules_dir.join("mod_a").join("manifests");
    fs::create_dir_all(&mod_a_dir).expect("Test invariant failed");
    fs::write(mod_a_dir.join("init.rhai"), r#"file("/tmp/a", #{})"#).expect("Test invariant failed");

    // Create Module B
    let mod_b_dir = modules_dir.join("mod_b").join("manifests");
    fs::create_dir_all(&mod_b_dir).expect("Test invariant failed");
    fs::write(mod_b_dir.join("init.rhai"), r#"file("/tmp/b", #{})"#).expect("Test invariant failed");

    // Site manifest with dependency: A must complete before B starts
    let site_script = r#"include("mod_a") -> include("mod_b");"#;
    let site_rhai = site_manifest_dir.join("site.rhai");
    fs::write(&site_rhai, site_script).expect("Test invariant failed");

    let loader = EnvironmentLoader::new(base_dir.path().to_path_buf());
    let engine = PupoxideEngine::new(None);
    let catalog = engine
        .run_manifest_with_modules(
            site_rhai,
            loader.get_modules_path("prod"),
            "node_test".to_string(),
            "prod".to_string(),
            pupoxide::domain::Facts::default(),
        )
        .expect("Test invariant failed");

    let resources = catalog.resources;

    // Check for ModuleStart/End and order
    let end_a_pos = resources
        .iter()
        .position(|r| r.id() == "ModuleEnd[mod_a]")
        .expect("ModuleEnd[mod_a] missing");
    let start_b_pos = resources
        .iter()
        .position(|r| r.id() == "ModuleStart[mod_b]")
        .expect("ModuleStart[mod_b] missing");

    assert!(
        end_a_pos < start_b_pos,
        "Module A must end before Module B starts"
    );

    // Check that file in A depends on Start[A]
    let file_a = resources
        .iter()
        .find(|r| r.id() == "File[/tmp/a]")
        .expect("File[/tmp/a] missing");
    assert!(
        file_a
            .dependencies()
            .contains(&"ModuleStart[mod_a]".to_string())
    );
}
