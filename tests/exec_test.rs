use pupoxide::domain::resource::{Ensure, ResourceProvider};
use pupoxide::infrastructure::ExecAdapter;
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_exec_creates_idempotency() {
    let dir = tempdir().unwrap();
    let output_file = dir.path().join("exec_output.txt");
    let output_path = output_file.to_str().unwrap();

    // Create ExecResource manually since we don't have a macro yet
    let resource = pupoxide::domain::resource::Resource::Exec(
        pupoxide::domain::resource::ExecResource {
            id: format!("Exec[echo test > {}]", output_path),
            command: format!("echo 'test content' > {}", output_path),
            creates: Some(output_file.clone()),
            unless: None,
            cwd: None,
            environment: None,
            dependencies: Vec::new(),
            backup: false,
        },
    );

    let adapter = ExecAdapter;

    // 1. First run - file doesn't exist, should execute
    let state = adapter
        .get_state(&resource, false)
        .await
        .expect("Failed to get state");
    assert_eq!(
        state,
        pupoxide::domain::resource::ResourceState::Ensure(Ensure::Absent)
    );

    adapter.apply(&resource).await.expect("Failed to apply");

    // 2. Verify file was created
    assert!(output_file.exists());
    let content = fs::read_to_string(&output_file).expect("Failed to read");
    assert!(content.contains("test content"));

    // 3. Second run - file exists, should skip (idempotency)
    let state2 = adapter
        .get_state(&resource, false)
        .await
        .expect("Failed to get state");
    assert_eq!(
        state2,
        pupoxide::domain::resource::ResourceState::Ensure(Ensure::Present)
    );

    // Store original content
    let original_content = fs::read_to_string(&output_file).expect("Failed to read");

    // Apply again - should be skipped
    adapter.apply(&resource).await.expect("Failed to apply");

    // Content should be unchanged (command was not executed)
    let new_content = fs::read_to_string(&output_file).expect("Failed to read");
    assert_eq!(original_content, new_content);
}

#[tokio::test]
async fn test_exec_unless_condition() {
    let dir = tempdir().unwrap();
    let marker_file = dir.path().join("marker.txt");
    let marker_path = marker_file.to_str().unwrap();

    // Create marker file
    fs::write(&marker_file, "marker").expect("Failed to create marker");

    let resource = pupoxide::domain::resource::Resource::Exec(
        pupoxide::domain::resource::ExecResource {
            id: "Exec[test unless]".to_string(),
            command: "echo 'should not run'".to_string(),
            creates: None,
            unless: Some(format!("test -f {}", marker_path)),
            cwd: None,
            environment: None,
            dependencies: Vec::new(),
            backup: false,
        },
    );

    let adapter = ExecAdapter;

    // Should skip because marker file exists
    let state = adapter
        .get_state(&resource, false)
        .await
        .expect("Failed to get state");
    assert_eq!(
        state,
        pupoxide::domain::resource::ResourceState::Ensure(Ensure::Present)
    );
}

#[tokio::test]
async fn test_exec_with_environment() {
    let dir = tempdir().unwrap();
    let output_file = dir.path().join("env_output.txt");
    let output_path = output_file.to_str().unwrap();

    let mut env = std::collections::HashMap::new();
    env.insert("TEST_VAR".to_string(), "test_value".to_string());

    let resource = pupoxide::domain::resource::Resource::Exec(
        pupoxide::domain::resource::ExecResource {
            id: "Exec[test env]".to_string(),
            command: format!("echo $TEST_VAR > {}", output_path),
            creates: None,
            unless: None,
            cwd: None,
            environment: Some(env),
            dependencies: Vec::new(),
            backup: false,
        },
    );

    let adapter = ExecAdapter;
    adapter.apply(&resource).await.expect("Failed to apply");

    // Verify environment variable was used
    assert!(output_file.exists());
    let content = fs::read_to_string(&output_file).expect("Failed to read");
    assert!(content.contains("test_value"));
}

#[tokio::test]
async fn test_exec_with_cwd() {
    let dir = tempdir().unwrap();
    let output_file = "cwd_test.txt";

    let resource = pupoxide::domain::resource::Resource::Exec(
        pupoxide::domain::resource::ExecResource {
            id: "Exec[test cwd]".to_string(),
            command: format!("echo 'cwd test' > {}", output_file),
            creates: None,
            unless: None,
            cwd: Some(dir.path().to_path_buf()),
            environment: None,
            dependencies: Vec::new(),
            backup: false,
        },
    );

    let adapter = ExecAdapter;
    adapter.apply(&resource).await.expect("Failed to apply");

    // Verify file was created in the specified directory
    let full_path = dir.path().join(output_file);
    assert!(full_path.exists());
    let content = fs::read_to_string(&full_path).expect("Failed to read");
    assert!(content.contains("cwd test"));
}

#[tokio::test]
async fn test_exec_command_failure() {
    let resource = pupoxide::domain::resource::Resource::Exec(
        pupoxide::domain::resource::ExecResource {
            id: "Exec[failing command]".to_string(),
            command: "exit 1".to_string(),
            creates: None,
            unless: None,
            cwd: None,
            environment: None,
            dependencies: Vec::new(),
            backup: false,
        },
    );

    let adapter = ExecAdapter;
    let result = adapter.apply(&resource).await;

    // Should return an error
    assert!(result.is_err());
}
