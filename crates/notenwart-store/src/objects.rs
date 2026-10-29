// SPDX-License-Identifier: AGPL-3.0-or-later

//! Where files live: an S3-compatible object store (Garage, MinIO, AWS, …) or a local directory.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use object_store::ObjectStore;
use object_store::aws::AmazonS3Builder;
use object_store::local::LocalFileSystem;
use uuid::Uuid;

pub type Store = Arc<dyn ObjectStore>;

/// Which object store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    S3,
    Fs,
}

impl Backend {
    /// As the system page names it: "s3" or "fs".
    pub fn name(self) -> &'static str {
        match self {
            Backend::S3 => "s3",
            Backend::Fs => "fs",
        }
    }
}

/// Where files are kept (the command line fills it from `STORAGE`, `STORAGE_PATH` and `S3_*`).
#[derive(Clone)]
pub struct StorageConfig {
    pub storage: Backend,
    /// Directory for `Backend::Fs`.
    pub storage_path: PathBuf,
    /// S3 endpoint, e.g. http://garage:3900 (none: AWS).
    pub s3_endpoint: Option<String>,
    pub s3_region: String,
    pub s3_bucket: String,
    pub s3_access_key_id: Option<String>,
    pub s3_secret_access_key: Option<String>,
}

impl std::fmt::Debug for StorageConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StorageConfig")
            .field("storage", &self.storage)
            .field("storage_path", &self.storage_path)
            .field("s3_endpoint", &self.s3_endpoint)
            .field("s3_region", &self.s3_region)
            .field("s3_bucket", &self.s3_bucket)
            .finish_non_exhaustive()
    }
}

impl StorageConfig {
    pub fn open(&self) -> anyhow::Result<Store> {
        Ok(match self.storage {
            Backend::Fs => {
                std::fs::create_dir_all(&self.storage_path)
                    .with_context(|| format!("cannot create {}", self.storage_path.display()))?;
                Arc::new(LocalFileSystem::new_with_prefix(&self.storage_path)?)
            }
            Backend::S3 => {
                let mut builder = AmazonS3Builder::new()
                    .with_region(&self.s3_region)
                    .with_bucket_name(&self.s3_bucket)
                    .with_access_key_id(self.s3_access_key_id.as_deref().context("S3_ACCESS_KEY_ID is not set")?)
                    .with_secret_access_key(
                        self.s3_secret_access_key.as_deref().context("S3_SECRET_ACCESS_KEY is not set")?,
                    );
                if let Some(endpoint) = &self.s3_endpoint {
                    builder = builder
                        .with_endpoint(endpoint)
                        .with_allow_http(endpoint.starts_with("http://"))
                        .with_virtual_hosted_style_request(false);
                }
                Arc::new(builder.build()?)
            }
        })
    }
}

pub fn source_key(organization: Uuid, source: Uuid) -> object_store::path::Path {
    object_store::path::Path::from(format!("{organization}/sources/{source}.pdf"))
}

pub fn part_key(organization: Uuid, part: Uuid) -> object_store::path::Path {
    object_store::path::Path::from(format!("{organization}/parts/{part}.pdf"))
}

pub fn recording_key(organization: Uuid, recording: Uuid) -> object_store::path::Path {
    object_store::path::Path::from(format!("{organization}/recordings/{recording}"))
}

pub fn image_key(organization: Uuid, image: Uuid) -> object_store::path::Path {
    object_store::path::Path::from(format!("{organization}/images/{image}"))
}
