// SPDX-License-Identifier: AGPL-3.0-or-later
#![allow(dead_code)]

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use lopdf::{Document, Object, Stream, dictionary};
use object_store::memory::InMemory;
use sqlx::PgPool;
use tower::ServiceExt;

use notenwart_api::router;
use notenwart_app::auth::{Role, new_token};
use notenwart_app::{App, Config};
use uuid::Uuid;

/// A PDF whose page n is `100 * n` points wide, so tests can tell pages apart. The page tree is nested and page 1
/// inherits its MediaBox, as in real-world files.
pub fn sample_pdf(pages: u32) -> Vec<u8> {
    let mut document = Document::with_version("1.7");
    let root = document.new_object_id();
    let middle = document.new_object_id();
    let mut kids = Vec::new();
    for n in 1..=pages {
        let content = document.add_object(Stream::new(dictionary! {}, b"0 0 m".to_vec()));
        let mut page = dictionary! { "Type" => "Page", "Parent" => middle, "Contents" => content };
        if n > 1 {
            page.set("MediaBox", vec![0.into(), 0.into(), (100 * n as i64).into(), 800.into()]);
        }
        kids.push(Object::Reference(document.add_object(page)));
    }
    document.objects.insert(
        middle,
        Object::Dictionary(
            dictionary! { "Type" => "Pages", "Parent" => root, "Kids" => kids, "Count" => pages as i64 },
        ),
    );
    document.objects.insert(
        root,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(middle)],
            "Count" => pages as i64,
            "MediaBox" => vec![0.into(), 0.into(), 100.into(), 800.into()],
        }),
    );
    let catalog = document.add_object(dictionary! { "Type" => "Catalog", "Pages" => root });
    document.trailer.set("Root", catalog);
    let mut out = Vec::new();
    document.save_to(&mut out).unwrap();
    out
}

/// Width of every page in order, following inherited MediaBoxes.
pub fn page_widths(bytes: &[u8]) -> Vec<i64> {
    let document = Document::load_mem(bytes).unwrap();
    document
        .get_pages()
        .values()
        .map(|&page| {
            let mut node = document.get_dictionary(page).unwrap();
            loop {
                if let Ok(media_box) = node.get(b"MediaBox") {
                    return media_box.as_array().unwrap()[2].as_i64().unwrap();
                }
                node = document.get_dictionary(node.get(b"Parent").unwrap().as_reference().unwrap()).unwrap();
            }
        })
        .collect()
}

/// A browser-like client: the router plus an optional session cookie.
#[derive(Clone)]
pub struct Client {
    pub router: Router,
    pub cookie: Option<String>,
}

impl Client {
    pub async fn send(&self, request: Request<Body>) -> (StatusCode, Vec<u8>, axum::http::HeaderMap) {
        let mut request = request;
        if let Some(cookie) = &self.cookie {
            request.headers_mut().insert("cookie", format!("notenwart_session={cookie}").parse().unwrap());
        }
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        (status, response.into_body().collect().await.unwrap().to_bytes().to_vec(), headers)
    }

    pub async fn get_bytes(&self, uri: &str) -> (StatusCode, Vec<u8>) {
        let (status, body, _) = self.send(Request::get(uri).body(Body::empty()).unwrap()).await;
        (status, body)
    }

    pub async fn json(
        &self,
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        let (status, value, _) = self.json_with_headers(method, uri, body).await;
        (status, value)
    }

    pub async fn json_with_headers(
        &self,
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value, axum::http::HeaderMap) {
        let request = Request::builder().method(method).uri(uri);
        let request = match body {
            Some(body) => request.header("content-type", "application/json").body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        }
        .unwrap();
        let (status, bytes, headers) = self.send(request).await;
        let value = if bytes.is_empty() { serde_json::Value::Null } else { serde_json::from_slice(&bytes).unwrap() };
        (status, value, headers)
    }

    pub async fn upload(&self, work_id: &str, file_name: &str, bytes: &[u8]) -> (StatusCode, serde_json::Value) {
        let boundary = "notenwart-test-boundary";
        let mut body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\n\
             Content-Type: application/pdf\r\n\r\n"
        )
        .into_bytes();
        body.extend_from_slice(bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        let request = Request::post(format!("/api/works/{work_id}/sources"))
            .header("content-type", format!("multipart/form-data; boundary={boundary}"))
            .body(Body::from(body))
            .unwrap();
        let (status, bytes, _) = self.send(request).await;
        (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
    }

    /// The session cookie a response set, if any.
    pub fn session_from(headers: &axum::http::HeaderMap) -> Option<String> {
        headers.get_all("set-cookie").iter().find_map(|value| {
            let value = value.to_str().ok()?;
            let token = value.strip_prefix("notenwart_session=")?.split(';').next()?;
            (!token.is_empty()).then(|| token.to_string())
        })
    }
}

pub struct TestApp {
    pub pool: PgPool,
    /// For calling server functions directly (background jobs).
    pub state: App,
    pub store: Arc<InMemory>,
    pub organization_id: Uuid,
    router: Router,
    /// Signed in as an admin.
    pub admin: Client,
}

impl TestApp {
    pub async fn new(pool: PgPool) -> Self {
        let mut app = Self::without_users(pool).await;
        app.admin = app.user(Role::Admin, "Admin").await.0;
        app
    }

    /// A fresh server that still needs setup; `admin` is not signed in.
    pub async fn without_users(pool: PgPool) -> Self {
        Self::with_sso(pool, None).await
    }

    /// A fresh server that still needs setup, signing in through `sso` if given.
    pub async fn with_sso(pool: PgPool, sso: Option<Arc<notenwart_app::sso::Sso>>) -> Self {
        let organization_id = notenwart_store::organization::ensure_default(&pool, "Test").await.unwrap();
        notenwart_app::catalogue::ensure(&pool, organization_id, notenwart_app::mail::Language::En).await.unwrap();
        let store = Arc::new(InMemory::new());
        let state = App {
            pool: pool.clone(),
            store: store.clone(),
            config: Arc::new(Config {
                public_url: "http://notenwart.test".into(),
                secure_cookies: false,
                max_upload: 1024 * 1024,
                language: notenwart_app::mail::Language::En,
                storage: "memory",
            }),
            mailer: None,
            limiter: Arc::default(),
            recognizer: notenwart_app::recognize::Recognizer::new("deu+eng", None).await,
            sso,
        };
        let router = router(state.clone());
        TestApp { state, pool, store, organization_id, router: router.clone(), admin: Client { router, cookie: None } }
    }

    pub fn anonymous(&self) -> Client {
        Client { router: self.router.clone(), cookie: None }
    }

    /// Creates a user with a session directly in the database (no password hashing, so tests stay fast).
    pub async fn user(&self, role: Role, name: &str) -> (Client, String) {
        let email = format!("{}@example.org", name.to_lowercase().replace(' ', "."));
        let user_id: Uuid = sqlx::query_scalar("INSERT INTO users (email, display_name) VALUES ($1, $2) RETURNING id")
            .bind(&email)
            .bind(name)
            .fetch_one(&self.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO memberships (organization_id, user_id, role) VALUES ($1, $2, $3)")
            .bind(self.organization_id)
            .bind(user_id)
            .bind(role)
            .execute(&self.pool)
            .await
            .unwrap();
        let (token, hash) = new_token();
        sqlx::query(
            "INSERT INTO sessions (token_hash, user_id, organization_id, expires_at)
             VALUES ($1, $2, $3, now() + interval '1 day')",
        )
        .bind(hash)
        .bind(user_id)
        .bind(self.organization_id)
        .execute(&self.pool)
        .await
        .unwrap();
        (Client { router: self.router.clone(), cookie: Some(token) }, user_id.to_string())
    }

    pub async fn object_count(&self) -> usize {
        use futures_util::TryStreamExt;
        use object_store::ObjectStore;
        self.store.list(None).try_collect::<Vec<_>>().await.unwrap().len()
    }
}
