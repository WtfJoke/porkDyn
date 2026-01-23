use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("Domain validation error: {0}")]
    DomainValidationError(String),
}

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Failed to create DNS record")]
    CreateRecordFailed,

    #[error("Failed to update DNS record")]
    UpdateRecordFailed,

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),
}
