use pupoxide::application::{ProviderRegistry, execute_transaction};
use pupoxide::domain::catalog::Catalog;
use pupoxide::domain::resource::{Ensure, ExecResource, FileResource, Resource};
use pupoxide::infrastructure::{ExecAdapter, FsAdapter, StateStore};
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn test_trigger_refreshonly_not_executed_by_default() {
    let temp_dir = tempdir().unwrap();
    let state_dir = temp_dir.path().join("state");
    let state_store = StateStore::new(state_dir);

    let output_file = temp_dir.path().join("exec_output.txt");
    let output_path = output_file.to_str().unwrap();

    let mut catalog = Catalog::new("node-1".to_string(), "production".to_string());

    // 1. Create a refreshonly exec resource (should NOT run by default)
    let exec = Resource::Exec(ExecResource {
        id: "Exec[test_refresh]".to_string(),
        command: format!("echo 'should not run' > {}", output_path),
        creates: None,
        unless: None,
        cwd: None,
        environment: None,
        dependencies: Vec::new(),
        notify: Vec::new(),
        subscribe: Vec::new(),
        refreshonly: Some(true),
        source_context: None,
        mutex: None,
    });
    catalog.add_resource(exec);
    catalog.build_edges();

    let mut registry = ProviderRegistry::new();
    registry.register(Arc::new(FsAdapter));
    registry.register(Arc::new(ExecAdapter));
    let provider = Arc::new(registry);

    let reports = execute_transaction(catalog, &state_store, provider, false, |_| {})
        .await
        .expect("Transaction failed");

    // Verify it was skipped/unchanged
    let report = reports
        .iter()
        .find(|r| r.resource_id == "Exec[test_refresh]")
        .unwrap();
    assert_eq!(
        report.status,
        pupoxide::domain::report::ResourceStatus::Unchanged
    );
    assert!(!output_file.exists());
}

#[tokio::test]
async fn test_trigger_notify_executes_refreshonly() {
    let temp_dir = tempdir().unwrap();
    let state_dir = temp_dir.path().join("state");
    let state_store = StateStore::new(state_dir);

    let trigger_file = temp_dir.path().join("trigger.txt");
    let trigger_path = trigger_file.to_str().unwrap();

    let output_file = temp_dir.path().join("output.txt");
    let output_path = output_file.to_str().unwrap();

    let mut catalog = Catalog::new("node-1".to_string(), "production".to_string());

    // 1. Create a file resource that notifies the exec resource
    let file = Resource::File(FileResource {
        id: format!("File[{}]", trigger_path),
        path: trigger_file.clone(),
        ensure: Ensure::Present,
        content: Some("new content".to_string()),
        dependencies: Vec::new(),
        notify: vec!["Exec[test_refresh]".to_string()],
        subscribe: Vec::new(),
        owner: None,
        group: None,
        mode: None,
        mutex: None,
        source_context: None,
    });

    // 2. Create the refreshonly exec resource
    let exec = Resource::Exec(ExecResource {
        id: "Exec[test_refresh]".to_string(),
        command: format!("echo 'refresh ran' > {}", output_path),
        creates: None,
        unless: None,
        cwd: None,
        environment: None,
        dependencies: Vec::new(),
        notify: Vec::new(),
        subscribe: Vec::new(),
        refreshonly: Some(true),
        source_context: None,
        mutex: None,
    });

    catalog.add_resource(file);
    catalog.add_resource(exec);
    catalog.build_edges();

    let mut registry = ProviderRegistry::new();
    registry.register(Arc::new(FsAdapter));
    registry.register(Arc::new(ExecAdapter));
    let provider = Arc::new(registry);

    let reports = execute_transaction(catalog, &state_store, provider, false, |_| {})
        .await
        .expect("Transaction failed");

    // Verify both resources ran (status Applied)
    let file_report = reports
        .iter()
        .find(|r| r.resource_id.starts_with("File["))
        .unwrap();
    assert_eq!(
        file_report.status,
        pupoxide::domain::report::ResourceStatus::Applied
    );

    let exec_report = reports
        .iter()
        .find(|r| r.resource_id == "Exec[test_refresh]")
        .unwrap();
    assert_eq!(
        exec_report.status,
        pupoxide::domain::report::ResourceStatus::Applied
    );

    // Verify output file exists and has correct content
    assert!(output_file.exists());
    let content = fs::read_to_string(&output_file).unwrap();
    assert_eq!(content.trim(), "refresh ran");
}

#[tokio::test]
async fn test_trigger_subscribe_executes_refreshonly() {
    let temp_dir = tempdir().unwrap();
    let state_dir = temp_dir.path().join("state");
    let state_store = StateStore::new(state_dir);

    let target_file = temp_dir.path().join("target.txt");
    let target_path = target_file.to_str().unwrap();

    let output_file = temp_dir.path().join("output.txt");
    let output_path = output_file.to_str().unwrap();

    let mut catalog = Catalog::new("node-1".to_string(), "production".to_string());

    // 1. Create a file resource
    let file = Resource::File(FileResource {
        id: format!("File[{}]", target_path),
        path: target_file.clone(),
        ensure: Ensure::Present,
        content: Some("some content".to_string()),
        dependencies: Vec::new(),
        notify: Vec::new(),
        subscribe: Vec::new(),
        owner: None,
        group: None,
        mode: None,
        mutex: None,
        source_context: None,
    });

    // 2. Create the refreshonly exec resource subscribing to the file
    let exec = Resource::Exec(ExecResource {
        id: "Exec[test_refresh]".to_string(),
        command: format!("echo 'subscribe ran' > {}", output_path),
        creates: None,
        unless: None,
        cwd: None,
        environment: None,
        dependencies: Vec::new(),
        notify: Vec::new(),
        subscribe: vec![format!("File[{}]", target_path)],
        refreshonly: Some(true),
        source_context: None,
        mutex: None,
    });

    catalog.add_resource(file);
    catalog.add_resource(exec);
    catalog.build_edges();

    let mut registry = ProviderRegistry::new();
    registry.register(Arc::new(FsAdapter));
    registry.register(Arc::new(ExecAdapter));
    let provider = Arc::new(registry);

    let reports = execute_transaction(catalog, &state_store, provider, false, |_| {})
        .await
        .expect("Transaction failed");

    // Verify both resources ran (status Applied)
    let file_report = reports
        .iter()
        .find(|r| r.resource_id.starts_with("File["))
        .unwrap();
    assert_eq!(
        file_report.status,
        pupoxide::domain::report::ResourceStatus::Applied
    );

    let exec_report = reports
        .iter()
        .find(|r| r.resource_id == "Exec[test_refresh]")
        .unwrap();
    assert_eq!(
        exec_report.status,
        pupoxide::domain::report::ResourceStatus::Applied
    );

    // Verify output file exists and has correct content
    assert!(output_file.exists());
    let content = fs::read_to_string(&output_file).unwrap();
    assert_eq!(content.trim(), "subscribe ran");
}
