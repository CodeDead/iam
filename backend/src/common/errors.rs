use std::collections::HashMap;

use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use thiserror::Error;
use tracing::error;

use crate::common::responses::{ErrorBody, ErrorResponse};

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{message}")]
    BadRequest { code: String, message: String },

    #[error("{message}")]
    Unauthorized { code: String, message: String },

    #[error("{message}")]
    Forbidden { code: String, message: String },

    #[error("{message}")]
    NotFound { code: String, message: String },

    #[error("{message}")]
    Conflict { code: String, message: String },

    #[error("{message}")]
    Validation {
        message: String,
        details: HashMap<String, Vec<String>>,
    },

    #[error("{message}")]
    TooManyRequests { message: String },

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn bad_request(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::BadRequest {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn unauthorized(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Unauthorized {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn forbidden(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Forbidden {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn not_found(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::NotFound {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn conflict(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Conflict {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn validation(message: impl Into<String>, details: HashMap<String, Vec<String>>) -> Self {
        Self::Validation {
            message: message.into(),
            details,
        }
    }

    pub fn too_many_requests(message: impl Into<String>) -> Self {
        Self::TooManyRequests {
            message: message.into(),
        }
    }

    pub fn internal(context: impl Into<String>) -> Self {
        Self::Internal(context.into())
    }

    pub fn error_code(&self) -> &str {
        match self {
            Self::BadRequest { code, .. }
            | Self::Unauthorized { code, .. }
            | Self::Forbidden { code, .. }
            | Self::NotFound { code, .. }
            | Self::Conflict { code, .. } => code.as_str(),
            Self::Validation { .. } => "VALIDATION_ERROR",
            Self::TooManyRequests { .. } => "TOO_MANY_REQUESTS",
            Self::Internal(_) => "INTERNAL_SERVER_ERROR",
        }
    }
}

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::BadRequest { .. } => StatusCode::BAD_REQUEST,
            Self::Unauthorized { .. } => StatusCode::UNAUTHORIZED,
            Self::Forbidden { .. } => StatusCode::FORBIDDEN,
            Self::NotFound { .. } => StatusCode::NOT_FOUND,
            Self::Conflict { .. } => StatusCode::CONFLICT,
            Self::Validation { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            Self::TooManyRequests { .. } => StatusCode::TOO_MANY_REQUESTS,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let status = self.status_code();

        let (code, message, details) = match self {
            Self::BadRequest { code, message }
            | Self::Unauthorized { code, message }
            | Self::Forbidden { code, message }
            | Self::NotFound { code, message }
            | Self::Conflict { code, message } => (code.clone(), message.clone(), None),
            Self::Validation { message, details } => (
                "VALIDATION_ERROR".to_string(),
                message.clone(),
                Some(details.clone()),
            ),
            Self::TooManyRequests { message } => {
                ("TOO_MANY_REQUESTS".to_string(), message.clone(), None)
            }
            Self::Internal(internal_msg) => {
                error!(internal_error = %internal_msg, "Unhandled internal server error");
                (
                    "INTERNAL_SERVER_ERROR".to_string(),
                    "An internal server error occurred".to_string(),
                    None,
                )
            }
        };

        let body = ErrorResponse {
            success: false,
            error: ErrorBody {
                code,
                message,
                details,
            },
        };

        HttpResponse::build(status).json(body)
    }
}

impl From<mongodb::error::Error> for AppError {
    fn from(err: mongodb::error::Error) -> Self {
        if let mongodb::error::ErrorKind::Write(mongodb::error::WriteFailure::WriteError(
            ref write_err,
        )) = *err.kind
            && write_err.code == 11000
        {
            return Self::conflict(
                "DUPLICATE_RESOURCE",
                "A resource with the provided unique value already exists",
            );
        }
        Self::Internal(format!("MongoDB error: {err}"))
    }
}
