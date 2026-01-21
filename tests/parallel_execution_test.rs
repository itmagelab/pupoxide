use pupoxide::application::{ProviderRegistry, PupoxideEngine};
use pupoxide::domain::Facts;
use pupoxide::infrastructure::{ExecAdapter, FsAdapter, StateStore};
use std::sync::Arc;
use std::time::Instant;
use tempfile::tempdir;

#[tokio::test]
async fn test_parallel_execution_speed() {
    let dir = tempdir().unwrap();
    let state_store = StateStore::new(dir.path().join("state"));
    let provider = Arc::new(
        ProviderRegistry::new()
            .with_provider(Arc::new(FsAdapter))
            .with_provider(Arc::new(ExecAdapter)),
    );
    let engine = PupoxideEngine::new(None);

    // Create a manifest with 3 sleep commands, each 1s
    // If sequential, should take ~3s
    // If parallel, should take ~1s + overhead
    let script = r#"
        exec("sleep1", #{ command: "sleep 1" });
        exec("sleep2", #{ command: "sleep 1" });
        exec("sleep3", #{ command: "sleep 1" });
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

    let start = Instant::now();
    let reports =
        pupoxide::application::execute_transaction(catalog, &state_store, provider, false, |_| {})
            .await
            .unwrap();
    let duration = start.elapsed();

    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(matches!(
            report.status,
            pupoxide::domain::report::ResourceStatus::Applied
        ));
    }

    println!("Parallel execution duration: {:?}", duration);
    // Allowing some buffer for overhead, but it should definitely be < 2s
    assert!(
        duration.as_secs_f32() < 2.0,
        "Execution took too long: {:?}",
        duration
    );
    assert!(
        duration.as_secs_f32() >= 1.0,
        "Execution was too fast (did sleep run?): {:?}",
        duration
    );
}

#[tokio::test]
async fn test_parallel_dependency_respect() {
    let dir = tempdir().unwrap();
    let state_store = StateStore::new(dir.path().join("state"));
    let provider = Arc::new(
        ProviderRegistry::new()
            .with_provider(Arc::new(FsAdapter))
            .with_provider(Arc::new(ExecAdapter)),
    );
    let engine = PupoxideEngine::new(None);

    // B depends on A. A sleeps for 1s.
    // We want to ensure B starts ONLY after A ends.
    let script = r#"
        let a = exec("A", #{ command: "sleep 1" });
        a -> exec("B", #{ command: "echo 'done' > /tmp/pupoxide_test_b" });
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

    let start = Instant::now();
    pupoxide::application::execute_transaction(catalog, &state_store, provider, false, |_| {})
        .await
        .unwrap();
    let duration = start.elapsed();

    assert!(
        duration.as_secs_f32() >= 1.0,
        "B might have run before A finished: {:?}",
        duration
    );
}
