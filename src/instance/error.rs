use thiserror::Error;

/// Errors for instance manager and handle operations.
#[derive(Debug, Error)]
pub enum InstanceError {
    #[error("instance already exists")]
    AlreadyExists,
    #[error("instance not found")]
    NotFound,
    #[error("instance command channel closed")]
    CommandChannelClosed,
}
