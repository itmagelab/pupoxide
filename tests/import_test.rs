use pupoxide::application::{EnvironmentLoader, PupoxideEngine};
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_native_import_and_dependencies() {
    let base_dir = tempdir().expect("Test invariant failed");
    let env_dir = base_dir.path().join("environments").join("prod");
    let modules_dir = env_dir.join("modules");
    let site_manifest_dir = env_dir.join("manifests");

    fs::create_dir_all(&site_manifest_dir).expect("Test invariant failed");
    fs::create_dir_all(&modules_dir).expect("Test invariant failed");

    // 1. Create Module structure
    let mod_dir = modules_dir.join("test_mod");
    let mod_manifests = mod_dir.join("manifests");
    fs::create_dir_all(&mod_manifests).expect("Test invariant failed");

    // init.rhai: imports service.rhai
    fs::write(
        mod_manifests.join("init.rhai"),
        r#"
        file("/tmp/init", #{});
        let svc = include("./service");
        svc; // Return the module handle
        "#,
    )
    .expect("Test invariant failed");

    // service.rhai: defines a file
    fs::write(
        mod_manifests.join("service.rhai"),
        r#"
        file("/tmp/service", #{});
        "#,
    )
    .expect("Test invariant failed");

    // 2. Site manifest with dependencies between imports and resources
    let site_script = r#"
        let my_mod = include("test_mod");
        let f1 = file("/tmp/top", #{});
        f1 -> my_mod;
    "#;
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

    let resources = catalog
        .topological_sort()
        .expect("Failed to sort resources");

    // Expected order:
    // 1. File[/tmp/top]
    // 2. ModuleStart[test_mod]
    // 3. File[/tmp/init]
    // 4. ModuleStart[./service]
    // 5. File[/tmp/service]
    // 6. ModuleEnd[./service]
    // 7. ModuleEnd[test_mod]

    let find_pos = |id: &str| {
        resources
            .iter()
            .position(|r| r.id() == id)
            .unwrap_or_else(|| panic!("Missing {}", id))
    };

    let pos_top = find_pos("File[/tmp/top]");
    let pos_start_mod = find_pos("ModuleStart[test_mod]");
    let pos_init = find_pos("File[/tmp/init]");
    let pos_start_svc = find_pos("ModuleStart[./service]");
    let pos_service = find_pos("File[/tmp/service]");
    let pos_end_svc = find_pos("ModuleEnd[./service]");
    let pos_end_mod = find_pos("ModuleEnd[test_mod]");

    assert!(
        pos_top < pos_start_mod,
        "File[/tmp/top] must be before ModuleStart[test_mod]"
    );
    assert!(
        pos_start_mod < pos_init,
        "ModuleStart[test_mod] must be before File[/tmp/init]"
    );
    assert!(
        pos_start_mod < pos_start_svc,
        "ModuleStart[test_mod] must be before ModuleStart[./service]"
    );

    assert!(
        pos_init < pos_end_mod,
        "File[/tmp/init] must be before ModuleEnd[test_mod]"
    );
    assert!(
        pos_service < pos_end_svc,
        "File[/tmp/service] must be before ModuleEnd[./service]"
    );
    assert!(
        pos_end_svc < pos_end_mod,
        "ModuleEnd[./service] must be before ModuleEnd[test_mod]"
    );
}

#[tokio::test]
async fn test_relative_include() {
    let base_dir = tempdir().expect("Test invariant failed");
    let site_manifest_dir = base_dir.path().join("manifests");
    fs::create_dir_all(&site_manifest_dir).expect("Test invariant failed");

    let site_script = r#"
        include("./sub");
    "#;
    let site_rhai = site_manifest_dir.join("site.rhai");
    fs::write(&site_rhai, site_script).expect("Test invariant failed");

    let sub_script = r#"
        file("/tmp/sub", #{});
    "#;
    fs::write(site_manifest_dir.join("sub.rhai"), sub_script).expect("Test invariant failed");

    let engine = PupoxideEngine::new(None);
    let catalog = engine
        .run_manifest(
            site_rhai,
            "test_node".to_string(),
            "local".to_string(),
            pupoxide::domain::Facts::default(),
        )
        .expect("Test invariant failed");

    assert!(
        catalog
            .resources()
            .iter()
            .any(|r| r.id() == "File[/tmp/sub]")
    );
}
