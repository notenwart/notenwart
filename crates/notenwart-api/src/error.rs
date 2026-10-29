// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use notenwart_app::error::AppError;
use serde::Serialize;

/// An error of the app layer as the API answers it: a status and the `Error` schema of `api/openapi.json`.
#[derive(Debug)]
pub struct ApiError(pub AppError);

pub type ApiResult<T> = Result<T, ApiError>;

impl<E: Into<AppError>> From<E> for ApiError {
    fn from(error: E) -> Self {
        ApiError(error.into())
    }
}

/// What went wrong (the `Error` schema).
#[derive(Debug, Serialize, utoipa::ToSchema)]
#[schema(as = Error)]
pub struct Problem {
    /// Machine-readable code, e.g. `unauthorized`, `forbidden`, `not_found`, `invalid`, `conflict`.
    error: &'static str,
    /// Human-readable explanation (English).
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error, message) = match self.0 {
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized", "please sign in".to_string()),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "forbidden", "your role does not allow this".to_string()),
            AppError::TooManyRequests => (
                StatusCode::TOO_MANY_REQUESTS,
                "too_many_requests",
                "too many attempts, please try again later".to_string(),
            ),
            AppError::NotFound => (StatusCode::NOT_FOUND, "not_found", "not found".to_string()),
            AppError::Invalid(message) => (StatusCode::UNPROCESSABLE_ENTITY, "invalid", message),
            AppError::Conflict(message) => (StatusCode::CONFLICT, "conflict", message),
            AppError::TooLarge(message) => (StatusCode::PAYLOAD_TOO_LARGE, "too_large", message),
            AppError::Internal(error) => {
                tracing::error!("{error:#}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal", "internal server error".to_string())
            }
        };
        (status, Json(Problem { error, message })).into_response()
    }
}
