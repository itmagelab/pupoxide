use pupoxide::application::PupoxideEngine;
use pupoxide::domain::resource::Resource;
use pupoxide::infrastructure::Facter;
use tempfile::tempdir;

#[tokio::test]
async fn test_package_resource_compilation() {
    let engine = PupoxideEngine::new(None);
    let facts = Facter::collect();

    let manifest = r#"
        stdlib::pkg("htop", #{ ensure: "present" });
        stdlib::pkg("wget", #{ ensure: "present", provider: "brew" });
    "#;

    let dir = tempdir().unwrap();
    let temp_file = dir.path().join("manifest.rhai");
    std::fs::write(&temp_file, manifest).unwrap();

    let catalog = engine
        .run_manifest(
            temp_file,
            "localhost".to_string(),
            "local".to_string(),
            facts,
        )
        .unwrap();

    let resources = catalog.resources();
    assert_eq!(resources.len(), 2);

    // Check first package (htop)
    let res1 = resources.first().unwrap();
    if let Resource::Package(pkg) = res1 {
        assert_eq!(pkg.name, "htop");
        assert_eq!(pkg.provider, "brew"); // Default
        assert_eq!(pkg.mutex, Some("brew".to_string())); // Auto-mutex
    } else {
        panic!("Expected PackageResource for htop");
    }

    // Check second package (wget)
    let res2 = resources.get(1).unwrap();
    if let Resource::Package(pkg) = res2 {
        assert_eq!(pkg.name, "wget");
        assert_eq!(pkg.provider, "brew");
        assert_eq!(pkg.mutex, Some("brew".to_string())); // Auto-mutex
    } else {
        panic!("Expected PackageResource for wget");
    }
}

#[tokio::test]
async fn test_package_provider_mapping() {
    let engine = PupoxideEngine::new(None);

    // Test Ubuntu mapping to apt
    let mut facts = pupoxide::domain::Facts::new();
    facts.insert("os_family".to_string(), "Ubuntu".to_string());

    let manifest = r#"stdlib::pkg("vim", #{})"#;

    let dir = tempdir().unwrap();
    let temp_file = dir.path().join("manifest.rhai");
    std::fs::write(&temp_file, manifest).unwrap();

    let catalog = engine
        .run_manifest(
            temp_file,
            "localhost".to_string(),
            "local".to_string(),
            facts,
        )
        .unwrap();

    let resources = catalog.resources();
    let res = resources.first().unwrap();
    if let Resource::Package(pkg) = res {
        assert_eq!(pkg.name, "vim");
        assert_eq!(pkg.provider, "apt");
        assert_eq!(pkg.mutex, Some("apt".to_string()));
    } else {
        panic!("Expected PackageResource");
    }
}
