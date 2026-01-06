#[macro_export]
macro_rules! pupoxide {
    ( $( $resource:expr ),* $(,)? ) => {
        vec![ $( $resource ),* ]
    };
}

#[macro_export]
macro_rules! file {
    ($path:expr => { ensure: $ensure:expr, content: $content:expr }) => {
        $crate::domain::Resource::File($crate::domain::FileResource {
            id: format!("File[{}]", $path),
            path: std::path::PathBuf::from($path),
            ensure: $ensure,
            content: Some($content.to_string()),
            dependencies: vec![],
            backup: true,
            max_backup_size: None,
            owner: None,
            group: None,
            mode: None,
        })
    };
    ($path:expr => { ensure: $ensure:expr }) => {
        $crate::domain::Resource::File($crate::domain::FileResource {
            id: format!("File[{}]", $path),
            path: std::path::PathBuf::from($path),
            ensure: $ensure,
            content: None,
            dependencies: vec![],
            backup: true,
            max_backup_size: None,
            owner: None,
            group: None,
            mode: None,
        })
    };
}
