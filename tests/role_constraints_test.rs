use pupoxide::application::{EnvironmentLoader, PupoxideEngine};
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_role_cannot_contain_file() {
    let base_dir = tempdir().expect("Test invariant failed");
    let env_dir = base_dir.path().join("environments").join("prod");
    fs::create_dir_all(env_dir.join("manifests")).expect("Test invariant failed");
    fs::create_dir_all(env_dir.join("role")).expect("Test invariant failed");

    // Role with a forbidden file resource
    fs::write(
        env_dir.join("role").join("bad_role.rhai"),
        r#"file("/tmp/forbidden", #{});"#,
    ).expect("Test invariant failed");

    let site_script = r#""bad_role".role;"#;
    let site_rhai = env_dir.join("manifests").join("site.rhai");
    fs::write(&site_rhai, site_script).expect("Test invariant failed");

    let loader = EnvironmentLoader::new(base_dir.path().to_path_buf());
    let engine = PupoxideEngine::new(None);
    let result = engine.run_manifest_with_modules(
        site_rhai,
        loader.get_modules_path("prod"),
        "test_node".to_string(),
        "prod".to_string(),
        pupoxide::domain::Facts::default(),
    );

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    println!("Actual error: {}", err);
    assert!(err.contains("Technical resources like 'File[/tmp/forbidden]' are NOT allowed directly in Roles"));
}

#[tokio::test]
async fn test_role_cannot_include_module() {
    let base_dir = tempdir().expect("Test invariant failed");
    let env_dir = base_dir.path().join("environments").join("prod");
    fs::create_dir_all(env_dir.join("manifests")).expect("Test invariant failed");
    fs::create_dir_all(env_dir.join("role")).expect("Test invariant failed");
    fs::create_dir_all(env_dir.join("modules").join("some_mod").join("manifests")).expect("Test invariant failed");

    fs::write(
        env_dir.join("modules").join("some_mod").join("manifests").join("init.rhai"),
        "",
    ).expect("Test invariant failed");

    // Role with a forbidden module include
    fs::write(
        env_dir.join("role").join("bad_role.rhai"),
        r#""some_mod".include;"#,
    ).expect("Test invariant failed");

    let site_script = r#""bad_role".role;"#;
    let site_rhai = env_dir.join("manifests").join("site.rhai");
    fs::write(&site_rhai, site_script).expect("Test invariant failed");

    let loader = EnvironmentLoader::new(base_dir.path().to_path_buf());
    let engine = PupoxideEngine::new(None);
    let result = engine.run_manifest_with_modules(
        site_rhai,
        loader.get_modules_path("prod"),
        "test_node".to_string(),
        "prod".to_string(),
        pupoxide::domain::Facts::default(),
    );

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Roles can ONLY include profiles"));
}

#[tokio::test]
async fn test_role_cannot_include_role() {
    let base_dir = tempdir().expect("Test invariant failed");
    let env_dir = base_dir.path().join("environments").join("prod");
    fs::create_dir_all(env_dir.join("manifests")).expect("Test invariant failed");
    fs::create_dir_all(env_dir.join("role")).expect("Test invariant failed");

    fs::write(env_dir.join("role").join("other_role.rhai"), "").expect("Test invariant failed");

    // Role with a forbidden role include
    fs::write(
        env_dir.join("role").join("bad_role.rhai"),
        r#""other_role".role;"#,
    ).expect("Test invariant failed");

    let site_script = r#""bad_role".role;"#;
    let site_rhai = env_dir.join("manifests").join("site.rhai");
    fs::write(&site_rhai, site_script).expect("Test invariant failed");

    let loader = EnvironmentLoader::new(base_dir.path().to_path_buf());
    let engine = PupoxideEngine::new(None);
    let result = engine.run_manifest_with_modules(
        site_rhai,
        loader.get_modules_path("prod"),
        "test_node".to_string(),
        "prod".to_string(),
        pupoxide::domain::Facts::default(),
    );

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Roles can ONLY include profiles"));
}

#[tokio::test]
async fn test_role_can_include_profile() {
    let base_dir = tempdir().expect("Test invariant failed");
    let env_dir = base_dir.path().join("environments").join("prod");
    fs::create_dir_all(env_dir.join("manifests")).expect("Test invariant failed");
    fs::create_dir_all(env_dir.join("role")).expect("Test invariant failed");
    fs::create_dir_all(env_dir.join("profile")).expect("Test invariant failed");

    fs::write(env_dir.join("profile").join("good_profile.rhai"), "").expect("Test invariant failed");

    // Role with a valid profile include
    fs::write(
        env_dir.join("role").join("good_role.rhai"),
        r#""good_profile".profile;"#,
    ).expect("Test invariant failed");

    let site_script = r#""good_role".role;"#;
    let site_rhai = env_dir.join("manifests").join("site.rhai");
    fs::write(&site_rhai, site_script).expect("Test invariant failed");

    let loader = EnvironmentLoader::new(base_dir.path().to_path_buf());
    let engine = PupoxideEngine::new(None);
    let result = engine.run_manifest_with_modules(
        site_rhai,
        loader.get_modules_path("prod"),
        "test_node".to_string(),
        "prod".to_string(),
        pupoxide::domain::Facts::default(),
    );

    assert!(result.is_ok());
}
