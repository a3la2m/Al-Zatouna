//! Stable application error ladder.
//!
//! Every error serializes to JSON with a stable dotted `code` (see docs/API.md),
//! an Arabic user-facing `message`, and optional `errors`/`details` payloads.
//! Internal details must never leak to end users — keep them in `tracing` logs.

use serde::Serialize;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, AppError>;

/// Top-level error kinds matching docs/ARCHITECTURE.md §7.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("validation: {0}")]
    Validation(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("concurrency: {0}")]
    Concurrency(String),

    #[error("database: {0}")]
    Storage(String),

    #[error("internal: {0}")]
    Internal(String),
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match &e {
            sqlx::Error::Database(db_err) => {
                // 2067 = SQLITE_CONSTRAINT_UNIQUE, 2065 CHECK, 2064 FK...
                if db_err.code().as_deref() == Some("2067")
                    && db_err.message().contains("unique")
                {
                    AppError::Conflict(db_err.message().to_string())
                } else {
                    AppError::Storage(db_err.message().to_string())
                }
            }
            sqlx::Error::RowNotFound => AppError::NotFound("row not found".into()),
            _ => AppError::Storage(e.to_string()),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Storage(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Internal(format!("serialization: {e}"))
    }
}

/// Serialized contract for the UI (docs/API.md error shape).
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct AppErrorDto {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl From<&AppError> for AppErrorDto {
    fn from(e: &AppError) -> Self {
        let (code, message) = match e {
            AppError::Validation(m) => ("validation", m),
            AppError::PermissionDenied(m) => ("permission_denied", m),
            AppError::NotFound(m) => ("not_found", m),
            AppError::Conflict(m) => ("conflict", m),
            AppError::Concurrency(m) => ("concurrency", m),
            AppError::Storage(m) => ("storage_error", m),
            AppError::Internal(m) => ("internal", m),
        };
        AppErrorDto {
            code: code.to_string(),
            message: message.clone(),
            field: None,
            details: None,
        }
    }
}

impl From<AppError> for AppErrorDto {
    fn from(e: AppError) -> Self {
        (&e).into()
    }
}

impl AppError {
    /// Stable dotted error code ("conflict.room_clash" style is built later
    /// by wrapping at the service layer; base codes are class-level here).
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Validation(_) => "validation",
            AppError::PermissionDenied(_) => "permission_denied",
            AppError::NotFound(_) => "not_found",
            AppError::Conflict(_) => "conflict",
            AppError::Concurrency(_) => "concurrency",
            AppError::Storage(_) => "storage_error",
            AppError::Internal(_) => "internal",
        }
    }

    pub fn validation(msg: impl Into<String>) -> Self {
        AppError::Validation(msg.into())
    }
    pub fn conflict(msg: impl Into<String>) -> Self {
        AppError::Conflict(msg.into())
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        AppError::NotFound(msg.into())
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        AppError::Internal(msg.into())
    }
    pub fn storage(msg: impl Into<String>) -> Self {
        AppError::Storage(msg.into())
    }
    pub fn permission(msg: impl Into<String>) -> Self {
        AppError::PermissionDenied(msg.into())
    }
}
