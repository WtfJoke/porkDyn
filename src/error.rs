use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("Domain validation error: {0}")]
    DomainValidationError(String),
}

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Failed to create DNS record: {0}")]
    CreateRecordFailed(String),

    #[error("Failed to update DNS record: {0}")]
    UpdateRecordFailed(String),

    #[error("Failed to retrieve DNS record: {0}")]
    RetrieveRecordFailed(String),

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),
}
