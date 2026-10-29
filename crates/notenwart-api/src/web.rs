// SPDX-License-Identifier: AGPL-3.0-or-later

//! The web app, embedded at compile time and served with a strict Content-Security-Policy.

use axum::http::{HeaderValue, StatusCode, Uri, header};
use axum::response::{Html, IntoResponse, Response};
use rust_embed::RustEmbed;

/// The built web app. Run `pnpm --dir web build` before a release build; a build without it serves the API and a
/// short page that says so. The folder is named here only.
#[derive(RustEmbed)]
#[folder = "../../web/build"]
#[allow_missing = true]
struct Assets;

/// Build output that carries its hash in its name and never changes: `assets/` (Vite), `_app/immutable/`
/// (SvelteKit, until the web app has moved to Vite).
const IMMUTABLE: [&str; 2] = ["assets/", "_app/immutable/"];

/// What the web app may load: itself only – no script, style, font or connection elsewhere, no frames. The
/// exceptions, all made in the browser itself: `data:` images (small inline images and icons), `blob:` images
/// (pictures scaled and PDF pages drawn before they are shown or uploaded), `blob:` media (an audio file is
/// measured before it is uploaded) and `blob:` workers (pdf.js may start its worker from a blob).
const POLICY: &str = "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data: blob:; \
media-src 'self' blob:; worker-src 'self' blob:; font-src 'self'; connect-src 'self'; frame-ancestors 'none'; \
base-uri 'none'; form-action 'self'";

/// The page shown where the web app is not built into the binary – without styles, which the policy would block.
const NO_WEB_APP: &str = "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>notenwart</title>\
<body><h1>notenwart</h1>\
<p>The API runs at <code>/api</code>, described in <code>api/openapi.yaml</code>. This build has no web app \
– run <code>pnpm --dir web build</code> and build again.</p>";

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let response = match asset(path) {
        Some(response) => response,
        // Client-side routes: unknown paths without a file extension get the SPA shell.
        None if !path.rsplit('/').next().unwrap_or_default().contains('.') => {
            asset("index.html").unwrap_or_else(|| Html(NO_WEB_APP).into_response())
        }
        None => StatusCode::NOT_FOUND.into_response(),
    };
    secure(response)
}

fn asset(path: &str) -> Option<Response> {
    let path = if path.is_empty() { "index.html" } else { path };
    let file = Assets::get(path)?;
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    // Hashed build output never changes; everything else must be revalidated (index.html, service worker).
    let cache = if IMMUTABLE.iter().any(|prefix| path.starts_with(prefix)) {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    Some(([(header::CONTENT_TYPE, mime.as_ref()), (header::CACHE_CONTROL, cache)], file.data).into_response())
}

/// The security headers of every answer of the web app.
fn secure(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(POLICY));
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    response
}
