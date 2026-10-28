// SPDX-License-Identifier: AGPL-3.0-or-later

//! Where PDFs live: an S3-compatible object store (Garage, MinIO, AWS, …) or a local directory.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use clap::{Args, ValueEnum};
use object_store::ObjectStore;
use object_store::aws::AmazonS3Builder;
use object_store::local::LocalFileSystem;
use uuid::Uuid;

pub type Store = Arc<dyn ObjectStore>;

#[derive(Clone, Copy, ValueEnum)]
pub enum Backend {
    S3,
    Fs,
}

#[derive(Args)]
pub struct StorageConfig {
    /// Where to keep PDFs.
    #[arg(long, env = "STORAGE", value_enum, default_value = "fs")]
    pub storage: Backend,
    /// Directory for `--storage fs`.
    #[arg(long, env = "STORAGE_PATH", default_value = "data")]
    pub storage_path: PathBuf,
    /// S3 endpoint, e.g. http://garage:3900 (empty: AWS).
    #[arg(long, env = "S3_ENDPOINT")]
    pub s3_endpoint: Option<String>,
    #[arg(long, env = "S3_REGION", default_value = "garage")]
    pub s3_region: String,
    #[arg(long, env = "S3_BUCKET", default_value = "notenwart")]
    pub s3_bucket: String,
    #[arg(long, env = "S3_ACCESS_KEY_ID")]
    pub s3_access_key_id: Option<String>,
    #[arg(long, env = "S3_SECRET_ACCESS_KEY", hide_env_values = true)]
    pub s3_secret_access_key: Option<String>,
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
                    .with_access_key_id(
                        self.s3_access_key_id
                            .as_deref()
                            .context("S3_ACCESS_KEY_ID is not set")?,
                    )
                    .with_secret_access_key(
                        self.s3_secret_access_key
                            .as_deref()
                            .context("S3_SECRET_ACCESS_KEY is not set")?,
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
