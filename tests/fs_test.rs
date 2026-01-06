use pupoxide::{pupoxide, file};
use pupoxide::domain::resource::{Ensure, ResourceProvider};
use pupoxide::infrastructure::FsAdapter;
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_file_resource_present() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_file.txt");
    let file_path_str = file_path.to_str().unwrap();
    
    let manifest = pupoxide! [
        file!(file_path_str => { 
            ensure: Ensure::Present, 
            content: "Hello from DSL!" 
        })
    ];

    let adapter = FsAdapter;
    let resource = &manifest[0];
    
    // 1. Initial state should be absent
    let state = adapter.get_state(resource, false).await.expect("Failed to get state");
    assert_eq!(state, pupoxide::domain::resource::ResourceState::Ensure(Ensure::Absent));

    // 2. Apply resource
    adapter.apply(resource).await.expect("Failed to apply");

    // 3. Verify on disk
    assert!(file_path.exists());
    let content = fs::read_to_string(&file_path).expect("Failed to read");
    assert_eq!(content, "Hello from DSL!");
}

#[tokio::test]
async fn test_file_resource_absent() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("delete_me.txt");
    let file_path_str = file_path.to_str().unwrap();
    fs::write(&file_path, "Goodbye!").expect("Failed to write initial file");
    
    let manifest = pupoxide! [
        file!(file_path_str => { 
            ensure: Ensure::Absent 
        })
    ];

    let adapter = FsAdapter;
    let resource = &manifest[0];
    
    // 1. Initial state (it exists)
    let state = adapter.get_state(resource, false).await.expect("Failed to get state");
    assert_eq!(state, pupoxide::domain::resource::ResourceState::Ensure(Ensure::Present));

    // 2. Apply resource (delete)
    adapter.apply(resource).await.expect("Failed to apply");

    // 3. Verify on disk
    assert!(!file_path.exists());
}
