use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("Domain validation error: {0}")]
    DomainValidationError(String),
}
