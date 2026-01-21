use actix_web::{HttpResponse, ResponseError};
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    /// Job not found
    JobNotFound(String),
    /// File not found
    FileNotFound(String),
    /// Invalid file type
    InvalidFileType(String),
    /// File too large
    FileTooLarge(usize, usize),
    /// gpop connection error
    GpopConnection(String),
    /// gpop protocol error
    GpopProtocol(String),
    /// Pipeline creation failed
    PipelineCreation(String),
    /// Storage error
    Storage(String),
    /// Internal error
    Internal(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::JobNotFound(id) => write!(f, "Job not found: {}", id),
            AppError::FileNotFound(path) => write!(f, "File not found: {}", path),
            AppError::InvalidFileType(ext) => write!(f, "Invalid file type: {}", ext),
            AppError::FileTooLarge(size, max) => {
                write!(f, "File too large: {} bytes (max: {} bytes)", size, max)
            }
            AppError::GpopConnection(msg) => write!(f, "gpop connection error: {}", msg),
            AppError::GpopProtocol(msg) => write!(f, "gpop protocol error: {}", msg),
            AppError::PipelineCreation(msg) => write!(f, "Pipeline creation failed: {}", msg),
            AppError::Storage(msg) => write!(f, "Storage error: {}", msg),
            AppError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        // Log the detailed error server-side
        tracing::error!("Request error: {}", self);

        // Return generic messages to clients to avoid leaking internal details
        match self {
            AppError::JobNotFound(_) => HttpResponse::NotFound().json(serde_json::json!({
                "error": "Job not found"
            })),
            AppError::FileNotFound(_) => HttpResponse::NotFound().json(serde_json::json!({
                "error": "Resource not found"
            })),
            AppError::InvalidFileType(_) | AppError::FileTooLarge(_, _) => {
                HttpResponse::BadRequest().json(serde_json::json!({
                    "error": self.to_string()
                }))
            }
            AppError::GpopConnection(_) | AppError::GpopProtocol(_) => {
                HttpResponse::ServiceUnavailable().json(serde_json::json!({
                    "error": "Backend service unavailable"
                }))
            }
            _ => HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Internal server error"
            })),
        }
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
