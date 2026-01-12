use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Stash error: {0}")]
    Stash(String),
}

pub type Result<T> = std::result::Result<T, DomainError>;
