use pupoxide::application::PupoxideEngine;
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_stdlib_module_version() {
    let base_dir = tempdir().expect("Test invariant failed");
    let site_rhai = base_dir.path().join("site.rhai");

    let script = r#"
        let v = std::version();
        file("/tmp/version.txt", #{ content: v });
    "#;
    fs::write(&site_rhai, script).expect("Test invariant failed");

    let engine = PupoxideEngine::new(None);
    let catalog = engine
        .run_manifest(
            site_rhai,
            "test_node".to_string(),
            "prod".to_string(),
            pupoxide::domain::Facts::default(),
        )
        .expect("Failed to run manifest");

    let resources = catalog.resources();
    let file_res = resources
        .iter()
        .find(|r| r.id() == "File[/tmp/version.txt]")
        .expect("File resource missing");

    if let pupoxide::domain::resource::Resource::File(f) = file_res {
        assert_eq!(f.content.as_ref().unwrap(), env!("CARGO_PKG_VERSION"));
    } else {
        panic!("Wrong resource type");
    }
}

#[tokio::test]
async fn test_stdlib_pkg() {
    let base_dir = tempdir().expect("Test invariant failed");
    let site_rhai = base_dir.path().join("site.rhai");

    let script = r#"
        stdlib::pkg("vim", #{ ensure: "present", provider: "brew" });
    "#;
    fs::write(&site_rhai, script).expect("Test invariant failed");

    let engine = PupoxideEngine::new(None);
    let catalog = engine
        .run_manifest(
            site_rhai,
            "test_node".to_string(),
            "prod".to_string(),
            pupoxide::domain::Facts::default(),
        )
        .expect("Failed to run manifest");

    let resources = catalog.resources();
    let pkg_res = resources
        .iter()
        .find(|r| r.id() == "Package[vim]")
        .expect("Package resource missing");

    if let pupoxide::domain::resource::Resource::Package(p) = pkg_res {
        assert_eq!(p.name, "vim");
        assert_eq!(p.provider, "brew");
    } else {
        panic!("Wrong resource type");
    }
}
