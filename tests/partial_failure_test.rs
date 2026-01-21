use pupoxide::application::{ProviderRegistry, PupoxideEngine};
use pupoxide::domain::Facts;
use pupoxide::infrastructure::{ExecAdapter, FsAdapter, StateStore};
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn test_continue_on_failure() {
    let dir = tempdir().unwrap();
    let state_store = StateStore::new(dir.path().join("state"));
    let provider = Arc::new(
        ProviderRegistry::new()
            .with_provider(Arc::new(FsAdapter))
            .with_provider(Arc::new(ExecAdapter)),
    );
    let engine = PupoxideEngine::new(None);

    let script = r#"
        exec("A", #{ command: "exit 1" });
        exec("B", #{ command: "echo 'B' > /tmp/pupoxide_test_b_fail" });
        let a = exec("FailMe", #{ command: "exit 1" });
        a -> exec("C", #{ command: "echo 'C' > /tmp/pupoxide_test_c_fail" });
    "#;

    let manifest_path = dir.path().join("site.rhai");
    std::fs::write(&manifest_path, script).unwrap();

    let catalog = engine
        .run_manifest(
            manifest_path,
            "test_node".to_string(),
            "prod".to_string(),
            Facts::default(),
        )
        .unwrap();

    let reports =
        pupoxide::application::execute_transaction(catalog, &state_store, provider, false, |_| {})
            .await
            .unwrap();

    assert!(reports.iter().any(|r| r.resource_id == "Exec[A]"
        && matches!(r.status, pupoxide::domain::report::ResourceStatus::Failed)));
    assert!(reports.iter().any(|r| r.resource_id == "Exec[B]"
        && matches!(r.status, pupoxide::domain::report::ResourceStatus::Applied)));
    assert!(reports.iter().any(|r| r.resource_id == "Exec[FailMe]"
        && matches!(r.status, pupoxide::domain::report::ResourceStatus::Failed)));
    assert!(reports.iter().any(|r| r.resource_id == "Exec[C]"
        && matches!(r.status, pupoxide::domain::report::ResourceStatus::Skipped)));

    println!("Reports received: {}", reports.len());
    for r in &reports {
        println!("{} -> {:?}", r.resource_id, r.status);
    }
}
