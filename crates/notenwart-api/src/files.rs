// SPDX-License-Identifier: AGPL-3.0-or-later

//! Files over HTTP: reading uploads from multipart forms, and sending stored PDFs.

use axum::body::Body;
use axum::extract::Multipart;
use axum::extract::multipart::MultipartError;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use notenwart_app::error::AppError;
use notenwart_app::files::Upload;
use notenwart_app::library::PdfFile;

/// The first field `file` of a form, if there is one; a read that fails is told by `error`.
pub async fn file_field(
    mut multipart: Multipart,
    error: fn(MultipartError) -> AppError,
) -> Result<Option<Upload>, AppError> {
    while let Some(field) = multipart.next_field().await.map_err(error)? {
        if field.name() == Some("file") {
            let file_name = field.file_name().map(str::to_string);
            let bytes = field.bytes().await.map_err(error)?;
            return Ok(Some(Upload { file_name, bytes }));
        }
    }
    Ok(None)
}

/// A large file: too large is 413, anything else 422.
pub fn large_upload_error(error: MultipartError) -> AppError {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        AppError::TooLarge("the file is larger than this server allows".into())
    } else {
        AppError::Invalid(error.body_text())
    }
}

/// A small file (images, recordings): any error is 422.
pub fn upload_error(error: MultipartError) -> AppError {
    AppError::Invalid(format!("upload: {error}"))
}

/// A stored PDF, shown in the browser.
pub fn pdf_response(file: PdfFile) -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/pdf".to_string()),
            (header::CONTENT_LENGTH, file.size.to_string()),
            (header::CONTENT_DISPOSITION, content_disposition(&file.file_name)),
            (header::CACHE_CONTROL, "private, no-cache".to_string()),
        ],
        Body::from_stream(file.content),
    )
        .into_response()
}

/// `inline` with an ASCII fallback and the full UTF-8 name (RFC 6266).
fn content_disposition(file_name: &str) -> String {
    let ascii: String = file_name
        .chars()
        .map(|c| if c.is_ascii_graphic() && c != '"' && c != '\\' || c == ' ' { c } else { '_' })
        .collect();
    let encoded = percent_encoding::utf8_percent_encode(file_name, percent_encoding::NON_ALPHANUMERIC);
    format!("inline; filename=\"{ascii}\"; filename*=UTF-8''{encoded}")
}

/// `inline` with the UTF-8 name only (annotated prints, recordings).
pub fn utf8_disposition(file_name: &str) -> String {
    format!(
        "inline; filename*=UTF-8''{}",
        percent_encoding::utf8_percent_encode(file_name, percent_encoding::NON_ALPHANUMERIC)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_disposition_keeps_unicode() {
        assert_eq!(
            content_disposition("Flöte 1.pdf"),
            "inline; filename=\"Fl_te 1.pdf\"; filename*=UTF-8''Fl%C3%B6te%201%2Epdf"
        );
    }
}
