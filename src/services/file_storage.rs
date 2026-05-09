use std::{path::{Path, PathBuf}, sync::Arc, time::{SystemTime, UNIX_EPOCH}};

use anyhow::{Context, Result};
use reqwest::multipart;
use serde::Deserialize;
use sha1::{Digest, Sha1};
use tokio::fs;
use uuid::Uuid;

use crate::config::Settings;

#[derive(Debug, Clone)]
pub struct StoredFile {
    pub file_name: String,
    pub storage_path: String,
    pub public_url: Option<String>,
}

#[derive(Debug, Clone)]
pub enum StorageProvider {
    Local(LocalStorage),
    Cloudinary(CloudinaryStorage),
}

impl StorageProvider {
    pub fn from_settings(settings: &Settings) -> Self {
        let cloudinary_ready = settings.cloudinary_cloud_name.is_some()
            && settings.cloudinary_api_key.is_some()
            && settings.cloudinary_api_secret.is_some();

        if cloudinary_ready {
            return Self::Cloudinary(CloudinaryStorage::from_settings(settings));
        }

        Self::Local(LocalStorage::new(settings.storage_path.clone()))
    }

    pub async fn store_bytes(
        &self,
        original_file_name: &str,
        content_type: &str,
        bytes: Vec<u8>,
    ) -> Result<StoredFile> {
        match self {
            Self::Local(local) => local.store_bytes(original_file_name, bytes).await,
            Self::Cloudinary(cloudinary) => {
                cloudinary
                    .upload_bytes(original_file_name, content_type, bytes)
                    .await
            }
        }
    }

    pub async fn read_bytes(&self, storage_path: &str) -> Result<Vec<u8>> {
        match self {
            Self::Local(local) => local.read_bytes(storage_path).await,
            Self::Cloudinary(cloudinary) => cloudinary.download_bytes(storage_path).await,
        }
    }

    pub async fn delete(&self, storage_path: &str) -> Result<()> {
        match self {
            Self::Local(local) => local.delete(storage_path).await,
            Self::Cloudinary(cloudinary) => cloudinary.delete_resource(storage_path).await,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LocalStorage {
    base_path: Arc<PathBuf>,
}

impl LocalStorage {
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            base_path: Arc::new(base_path),
        }
    }

    pub async fn store_bytes(&self, original_file_name: &str, bytes: Vec<u8>) -> Result<StoredFile> {
        fs::create_dir_all(self.base_path.as_path())
            .await
            .context("Failed to create local upload directory")?;

        let sanitized_name = sanitize_file_name(original_file_name);
        let unique_name = format!("{}-{}", Uuid::new_v4(), sanitized_name);
        let full_path = self.base_path.join(&unique_name);

        fs::write(&full_path, bytes)
            .await
            .with_context(|| format!("Failed to write file to {}", full_path.display()))?;

        Ok(StoredFile {
            file_name: unique_name.clone(),
            storage_path: full_path.to_string_lossy().to_string(),
            public_url: None,
        })
    }

    pub async fn read_bytes(&self, storage_path: &str) -> Result<Vec<u8>> {
        let path = Path::new(storage_path);
        let bytes = fs::read(path)
            .await
            .with_context(|| format!("Failed to read stored file: {}", path.display()))?;
        Ok(bytes)
    }

    pub async fn delete(&self, storage_path: &str) -> Result<()> {
        let path = Path::new(storage_path);
        if fs::metadata(path).await.is_ok() {
            fs::remove_file(path)
                .await
                .with_context(|| format!("Failed to delete stored file: {}", path.display()))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CloudinaryStorage {
    cloud_name: String,
    api_key: String,
    api_secret: String,
    client: reqwest::Client,
}

impl CloudinaryStorage {
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            cloud_name: settings.cloudinary_cloud_name.clone().unwrap_or_default(),
            api_key: settings.cloudinary_api_key.clone().unwrap_or_default(),
            api_secret: settings.cloudinary_api_secret.clone().unwrap_or_default(),
            client: reqwest::Client::new(),
        }
    }

    pub async fn upload_bytes(
        &self,
        original_file_name: &str,
        content_type: &str,
        bytes: Vec<u8>,
    ) -> Result<StoredFile> {
        let timestamp = now_unix_timestamp();
        let public_id = format!("uploads/{}-{}", Uuid::new_v4(), sanitize_file_name(original_file_name));

        let signature_payload = format!("public_id={}&timestamp={}{}", public_id, timestamp, self.api_secret);
        let signature = hex::encode(Sha1::digest(signature_payload.as_bytes()));

        let endpoint = format!(
            "https://api.cloudinary.com/v1_1/{}/raw/upload",
            self.cloud_name
        );

        let part = multipart::Part::bytes(bytes)
            .file_name(original_file_name.to_string())
            .mime_str(content_type)
            .context("Invalid upload content type")?;

        let form = multipart::Form::new()
            .part("file", part)
            .text("api_key", self.api_key.clone())
            .text("timestamp", timestamp.to_string())
            .text("public_id", public_id.clone())
            .text("signature", signature);

        let response = self
            .client
            .post(&endpoint)
            .multipart(form)
            .send()
            .await
            .context("Cloudinary upload request failed")?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_else(|_| "<no body>".to_string());
            anyhow::bail!("Cloudinary upload failed: {}", body);
        }

        let upload: CloudinaryUploadResponse = response
            .json()
            .await
            .context("Failed to parse Cloudinary upload response")?;

        Ok(StoredFile {
            file_name: original_file_name.to_string(),
            storage_path: upload.public_id,
            public_url: Some(upload.secure_url),
        })
    }

    pub async fn download_bytes(&self, storage_path: &str) -> Result<Vec<u8>> {
        let resource_url = format!(
            "https://res.cloudinary.com/{}/raw/upload/{}",
            self.cloud_name, storage_path
        );

        let response = self
            .client
            .get(resource_url)
            .send()
            .await
            .context("Cloudinary download request failed")?;

        if !response.status().is_success() {
            anyhow::bail!("Cloudinary download failed with status {}", response.status());
        }

        let bytes = response.bytes().await.context("Failed to read Cloudinary bytes")?;
        Ok(bytes.to_vec())
    }

    pub async fn delete_resource(&self, storage_path: &str) -> Result<()> {
        let timestamp = now_unix_timestamp();
        let signature_payload = format!("public_id={}&timestamp={}{}", storage_path, timestamp, self.api_secret);
        let signature = hex::encode(Sha1::digest(signature_payload.as_bytes()));

        let endpoint = format!(
            "https://api.cloudinary.com/v1_1/{}/raw/destroy",
            self.cloud_name
        );

        let params = [
            ("public_id", storage_path.to_string()),
            ("timestamp", timestamp.to_string()),
            ("api_key", self.api_key.clone()),
            ("signature", signature),
        ];

        let response = self
            .client
            .post(endpoint)
            .form(&params)
            .send()
            .await
            .context("Cloudinary delete request failed")?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_else(|_| "<no body>".to_string());
            anyhow::bail!("Cloudinary delete failed: {}", body);
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct CloudinaryUploadResponse {
    public_id: String,
    secure_url: String,
}

fn sanitize_file_name(file_name: &str) -> String {
    file_name
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => c,
            _ => '_',
        })
        .collect()
}

fn now_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
