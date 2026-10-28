// SPDX-License-Identifier: AGPL-3.0-or-later

use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

/// The built web app (`web/build`), embedded at compile time.
/// Run `pnpm --dir web build` before a release build; without it the server only answers the API.
#[derive(RustEmbed)]
#[folder = "../web/build"]
#[allow_missing = true]
struct Assets;

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if let Some(response) = asset(path) {
        return response;
    }
    // Client-side routes: unknown paths without a file extension get the SPA shell.
    if !path.rsplit('/').next().unwrap_or_default().contains('.')
        && let Some(response) = asset("index.html")
    {
        return response;
    }
    StatusCode::NOT_FOUND.into_response()
}

fn asset(path: &str) -> Option<Response> {
    let path = if path.is_empty() { "index.html" } else { path };
    let file = Assets::get(path)?;
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    // Hashed build output never changes; everything else must be revalidated (index.html, service worker).
    let cache = if path.starts_with("_app/immutable/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    Some(
        (
            [(header::CONTENT_TYPE, mime.as_ref()), (header::CACHE_CONTROL, cache)],
            file.data,
        )
            .into_response(),
    )
}
