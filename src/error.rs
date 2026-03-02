use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("missing required environment variable: {0}")]
    MissingEnv(&'static str),
    #[error("invalid environment variable {name}: {reason}")]
    InvalidEnv { name: &'static str, reason: String },
    #[error("wa-rs error: {0}")]
    Wa(String),
}

impl AppError {
    pub fn wa<E>(error: E) -> Self
    where
        E: std::fmt::Display,
    {
        Self::Wa(error.to_string())
    }
}
