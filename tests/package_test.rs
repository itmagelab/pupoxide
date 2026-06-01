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

#[tokio::test]
async fn test_package_provider_mapping_yum() {
    let engine = PupoxideEngine::new(None);

    // Test CentOS mapping to yum
    let mut facts = pupoxide::domain::Facts::new();
    facts.insert("os_family".to_string(), "CentOS".to_string());

    let manifest = r#"stdlib::pkg("nginx", #{})"#;

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
        assert_eq!(pkg.name, "nginx");
        assert_eq!(pkg.provider, "yum");
        assert_eq!(pkg.mutex, Some("yum".to_string()));
    } else {
        panic!("Expected PackageResource");
    }
}

#[tokio::test]
async fn test_apt_and_yum_providers_fail_fast_on_macos() {
    use pupoxide::domain::resource::ResourceProvider;
    use pupoxide::domain::resource::{Ensure, PackageResource};
    use pupoxide::infrastructure::PackageAdapter;

    let adapter = PackageAdapter::default();

    // 1. Check apt package fail fast
    let apt_res = Resource::Package(PackageResource {
        id: "Package[git]".to_string(),
        name: "git".to_string(),
        ensure: Ensure::Present,
        provider: "apt".to_string(),
        dependencies: vec![],
        source_context: None,
        mutex: Some("apt".to_string()),
        params: None,
    });

    let res = adapter.get_state(&apt_res, false).await;

    // On macOS, dpkg is missing, so it must return Err with fail fast message
    #[cfg(target_os = "macos")]
    {
        assert!(res.is_err());
        let err_msg = res.err().unwrap().to_string();
        assert!(
            err_msg.contains("dpkg/apt-get not found")
                || err_msg.contains("dpkg command not found")
        );
    }

    // 2. Check yum package fail fast
    let yum_res = Resource::Package(PackageResource {
        id: "Package[git]".to_string(),
        name: "git".to_string(),
        ensure: Ensure::Present,
        provider: "yum".to_string(),
        dependencies: vec![],
        source_context: None,
        mutex: Some("yum".to_string()),
        params: None,
    });

    let res_yum = adapter.get_state(&yum_res, false).await;

    // On macOS, rpm is missing, so it must return Err with fail fast message
    #[cfg(target_os = "macos")]
    {
        assert!(res_yum.is_err());
        let err_msg = res_yum.err().unwrap().to_string();
        assert!(err_msg.contains("rpm/yum not found") || err_msg.contains("rpm command not found"));
    }
}

#[tokio::test]
async fn test_package_update_cache_dsl_parsing() {
    let engine = PupoxideEngine::new(None);
    let facts = pupoxide::infrastructure::Facter::collect();

    let manifest = r#"
        stdlib::pkg("htop", #{ ensure: "present", params: #{ update_cache: false } });
        stdlib::pkg("wget", #{ ensure: "present", params: #{ update_cache: true } });
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

    let res1 = resources.first().unwrap();
    if let Resource::Package(pkg) = res1 {
        assert_eq!(pkg.name, "htop");
        let params = pkg.params.as_ref().unwrap();
        assert_eq!(params.get("update_cache").unwrap().as_bool(), Some(false));
    } else {
        panic!("Expected PackageResource");
    }

    let res2 = resources.get(1).unwrap();
    if let Resource::Package(pkg) = res2 {
        assert_eq!(pkg.name, "wget");
        let params = pkg.params.as_ref().unwrap();
        assert_eq!(params.get("update_cache").unwrap().as_bool(), Some(true));
    } else {
        panic!("Expected PackageResource");
    }
}

#[tokio::test]
async fn test_package_version_params_parsing() {
    let engine = PupoxideEngine::new(None);
    let facts = pupoxide::infrastructure::Facter::collect();

    let manifest = r#"
        stdlib::pkg("htop", #{ ensure: "present", params: #{ version: "3.3.0" } });
        stdlib::pkg("wget", #{ ensure: "present", params: #{ version: "1.21.4", update_cache: false } });
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

    let res1 = resources.first().unwrap();
    if let Resource::Package(pkg) = res1 {
        assert_eq!(pkg.name, "htop");
        let params = pkg.params.as_ref().unwrap();
        assert_eq!(params.get("version").unwrap().as_str(), Some("3.3.0"));
    } else {
        panic!("Expected PackageResource");
    }

    let res2 = resources.get(1).unwrap();
    if let Resource::Package(pkg) = res2 {
        assert_eq!(pkg.name, "wget");
        let params = pkg.params.as_ref().unwrap();
        assert_eq!(params.get("version").unwrap().as_str(), Some("1.21.4"));
        assert_eq!(params.get("update_cache").unwrap().as_bool(), Some(false));
    } else {
        panic!("Expected PackageResource");
    }
}
