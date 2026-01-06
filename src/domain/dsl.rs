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
            path: std::path::PathBuf::from($path),
            ensure: $ensure,
            content: Some($content.to_string()),
        })
    };
    ($path:expr => { ensure: $ensure:expr }) => {
        $crate::domain::Resource::File($crate::domain::FileResource {
            path: std::path::PathBuf::from($path),
            ensure: $ensure,
            content: None,
        })
    };
}
