// SPDX-License-Identifier: AGPL-3.0-or-later

//! Why a command did not happen. The API turns each into a status and the `Error` schema of `api/openapi.yaml`.

#[derive(Debug)]
pub enum AppError {
    /// Nobody is signed in (or the session is gone).
    Unauthorized,
    /// The caller's role does not allow it.
    Forbidden,
    TooManyRequests,
    NotFound,
    /// The input is not valid; the message says why.
    Invalid(String),
    /// It clashes with what is there.
    Conflict(String),
    TooLarge(String),
    /// Anything else: logged, not shown.
    Internal(anyhow::Error),
}

pub type AppResult<T> = Result<T, AppError>;

impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(error: E) -> Self {
        AppError::Internal(error.into())
    }
}

/// Maps a unique-constraint violation to a conflict with `message`; anything else stays an internal error.
pub fn unique_violation(error: sqlx::Error, message: &str) -> AppError {
    match &error {
        sqlx::Error::Database(db) if db.is_unique_violation() => AppError::Conflict(message.to_string()),
        _ => error.into(),
    }
}
