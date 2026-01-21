use pupoxide::application::PupoxideEngine;
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_string_method_include() {
    let base_dir = tempdir().expect("Test invariant failed");
    let site_manifest_dir = base_dir.path().join("manifests");
    fs::create_dir_all(&site_manifest_dir).expect("Test invariant failed");

    let site_script = r#"
        "./sub".include;
    "#;
    let site_rhai = site_manifest_dir.join("site.rhai");
    fs::write(&site_rhai, site_script).expect("Test invariant failed");

    let sub_script = r#"
        file("/tmp/sub_method", #{});
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
            .any(|r| r.id() == "File[/tmp/sub_method]")
    );
}
