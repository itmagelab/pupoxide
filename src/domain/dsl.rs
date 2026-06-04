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
            notify: vec![],
            subscribe: vec![],
            owner: None,
            group: None,
            mode: None,
            source_context: None,
            mutex: None,
        })
    };
    ($path:expr => { ensure: $ensure:expr }) => {
        $crate::domain::Resource::File($crate::domain::FileResource {
            id: format!("File[{}]", $path),
            path: std::path::PathBuf::from($path),
            ensure: $ensure,
            content: None,
            dependencies: vec![],
            notify: vec![],
            subscribe: vec![],
            owner: None,
            group: None,
            mode: None,
            source_context: None,
            mutex: None,
        })
    };
}
