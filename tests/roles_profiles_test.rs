use pupoxide::application::{EnvironmentLoader, PupoxideEngine};
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_roles_and_profiles() {
    let base_dir = tempdir().expect("Test invariant failed");
    let env_dir = base_dir.path().join("environments").join("prod");
    
    let manifests_dir = env_dir.join("manifests");
    let role_dir = env_dir.join("role");
    let profile_dir = env_dir.join("profile");
    let modules_dir = env_dir.join("modules");

    fs::create_dir_all(&manifests_dir).expect("Test invariant failed");
    fs::create_dir_all(&role_dir).expect("Test invariant failed");
    fs::create_dir_all(&profile_dir).expect("Test invariant failed");
    fs::create_dir_all(&modules_dir).expect("Test invariant failed");

    // 1. Create a Profile
    fs::write(
        profile_dir.join("common.rhai"),
        r#"file("/tmp/profile_common", #{});"#,
    ).expect("Test invariant failed");

    // 2. Create a Role that includes the Profile
    fs::write(
        role_dir.join("webserver.rhai"),
        r#"
        "common".profile;
        "#,
    ).expect("Test invariant failed");

    // 3. Site manifest uses the Role
    let site_script = r#"
        "webserver".role;
    "#;
    let site_rhai = manifests_dir.join("site.rhai");
    fs::write(&site_rhai, site_script).expect("Test invariant failed");

    // 4. Execute
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

    let ids: Vec<_> = catalog.resources.iter().map(|r| r.id()).collect();
    
    // Check presence
    assert!(ids.contains(&"File[/tmp/profile_common]"));
    assert!(ids.contains(&"File[/tmp/profile_common]"));
}
