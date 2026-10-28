// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// An API error, rendered as the `Error` schema of `api/openapi.yaml`.
#[derive(Debug)]
pub enum ApiError {
    Unauthorized,
    Forbidden,
    TooManyRequests,
    NotFound,
    Invalid(String),
    Conflict(String),
    TooLarge(String),
    Internal(anyhow::Error),
}

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Serialize)]
struct Body<'a> {
    error: &'a str,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error, message) = match self {
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized", "please sign in".to_string()),
            ApiError::Forbidden => (
                StatusCode::FORBIDDEN,
                "forbidden",
                "your role does not allow this".to_string(),
            ),
            ApiError::TooManyRequests => (
                StatusCode::TOO_MANY_REQUESTS,
                "too_many_requests",
                "too many attempts, please try again later".to_string(),
            ),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not_found", "not found".to_string()),
            ApiError::Invalid(message) => (StatusCode::UNPROCESSABLE_ENTITY, "invalid", message),
            ApiError::Conflict(message) => (StatusCode::CONFLICT, "conflict", message),
            ApiError::TooLarge(message) => (StatusCode::PAYLOAD_TOO_LARGE, "too_large", message),
            ApiError::Internal(error) => {
                tracing::error!("{error:#}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "internal server error".to_string(),
                )
            }
        };
        (status, Json(Body { error, message })).into_response()
    }
}

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(error: E) -> Self {
        ApiError::Internal(error.into())
    }
}

/// Maps a unique-constraint violation to 409 with `message`; anything else stays an internal error.
pub fn unique_violation(error: sqlx::Error, message: &str) -> ApiError {
    match &error {
        sqlx::Error::Database(db) if db.is_unique_violation() => ApiError::Conflict(message.to_string()),
        _ => error.into(),
    }
}
