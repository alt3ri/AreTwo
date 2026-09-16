//! R2 (S3-compatible) client. Keys are flat: folders are just prefixes.

use crate::store;
use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::{Client, Config};
use base64::Engine;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;

pub const MAX_KEYS: i32 = 1000;

#[derive(Default)]
pub struct R2 {
    clients: Mutex<HashMap<String, Client>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderEntry {
    pub prefix: String,
    pub name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectEntry {
    pub key: String,
    pub name: String,
    pub size: i64,
    pub last_modified: String,
    pub etag: String,
    pub storage_class: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPage {
    pub folders: Vec<FolderEntry>,
    pub files: Vec<ObjectEntry>,
    pub next_token: Option<String>,
    pub truncated: bool,
    pub folder_count: usize,
    pub file_count: usize,
    pub total_size: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectInfo {
    pub size: i64,
    pub content_type: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub storage_class: Option<String>,
    pub cache_control: Option<String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPreview {
    pub text: String,
    pub size: i64,
    pub truncated: bool,
}

// ---------- pure helpers (unit tested) ----------

/// "a/b/" -> "a/", "a/" -> "", "" -> ""
pub fn parent_prefix(prefix: &str) -> String {
    let trimmed = prefix.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(i) => trimmed[..=i].to_string(),
        None => String::new(),
    }
}

/// "a/b/" -> "b"
pub fn folder_name(prefix: &str) -> String {
    let trimmed = prefix.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(i) => trimmed[i + 1..].to_string(),
        None => trimmed.to_string(),
    }
}

/// "a/b/c.webp" -> "c.webp"
pub fn file_name(key: &str) -> String {
    match key.rfind('/') {
        Some(i) => key[i + 1..].to_string(),
        None => key.to_string(),
    }
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

// ---------- client plumbing ----------

fn endpoint_for(profile: &store::Profile) -> String {
    match profile.endpoint.as_deref().map(str::trim) {
        Some(e) if !e.is_empty() => e.trim_end_matches('/').to_string(),
        _ => format!("https://{}.r2.cloudflarestorage.com", profile.account_id.trim()),
    }
}

impl R2 {
    pub fn client_for(&self, profile: &store::Profile) -> Result<Client, String> {
        let cache_key = format!(
            "{}|{}|{}",
            profile.id, profile.access_key_id, endpoint_for(profile)
        );
        if let Some(hit) = self.clients.lock().unwrap().get(&cache_key) {
            return Ok(hit.clone());
        }
        let secret = store::secret_for(&profile.id)?;
        let conf = Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new("auto"))
            .endpoint_url(endpoint_for(profile))
            .credentials_provider(Credentials::new(
                profile.access_key_id.clone(),
                secret,
                None,
                None,
                "r2-explorer",
            ))
            .force_path_style(true)
            .build();
        let client = Client::from_conf(conf);
        self.clients
            .lock()
            .unwrap()
            .insert(cache_key, client.clone());
        Ok(client)
    }

    fn conn(&self, profile_id: &str) -> Result<(store::Profile, Client), String> {
        let profile = store::profile_by_id(profile_id)?;
        let client = self.client_for(&profile)?;
        Ok((profile, client))
    }

    pub async fn list_buckets(&self, profile_id: &str) -> Result<Vec<String>, String> {
        let (_, client) = self.conn(profile_id)?;
        let out = client.list_buckets().send().await.map_err(err)?;
        let mut names: Vec<String> = out
            .buckets()
            .iter()
            .filter_map(|b| b.name().map(str::to_string))
            .collect();
        names.sort();
        Ok(names)
    }

    pub async fn list_objects(
        &self,
        profile_id: &str,
        bucket: &str,
        prefix: &str,
        token: Option<String>,
    ) -> Result<ListPage, String> {
        let (_, client) = self.conn(profile_id)?;
        let out = client
            .list_objects_v2()
            .bucket(bucket)
            .prefix(prefix)
            .delimiter("/")
            .set_continuation_token(token)
            .max_keys(MAX_KEYS)
            .send()
            .await
            .map_err(err)?;

        let folders = out
            .common_prefixes()
            .iter()
            .filter_map(|p| p.prefix().map(str::to_string))
            .map(|prefix| FolderEntry {
                name: folder_name(&prefix),
                prefix,
            })
            .collect::<Vec<_>>();

        let files = out
            .contents()
            .iter()
            .filter(|o| !o.key().unwrap_or_default().ends_with('/'))
            .map(|o| {
                let key = o.key().unwrap_or_default().to_string();
                ObjectEntry {
                    name: file_name(&key),
                    key,
                    size: o.size().unwrap_or_default(),
                    last_modified: o
                        .last_modified()
                        .map(|t| t.to_string())
                        .unwrap_or_default(),
                    etag: o.e_tag().unwrap_or_default().trim_matches('"').to_string(),
                    storage_class: o
                        .storage_class()
                        .map(|s| s.as_str().to_string())
                        .unwrap_or_default(),
                }
            })
            .collect::<Vec<_>>();

        Ok(ListPage {
            folder_count: folders.len(),
            file_count: files.len(),
            total_size: files.iter().map(|f| f.size).sum(),
            next_token: out.next_continuation_token().map(str::to_string),
            truncated: out.is_truncated().unwrap_or(false),
            folders,
            files,
        })
    }

    pub async fn head_object(
        &self,
        profile_id: &str,
        bucket: &str,
        key: &str,
    ) -> Result<ObjectInfo, String> {
        let (_, client) = self.conn(profile_id)?;
        let out = client
            .head_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map_err(err)?;
        Ok(ObjectInfo {
            size: out.content_length().unwrap_or_default(),
            content_type: out.content_type().map(str::to_string),
            etag: out.e_tag().map(|e| e.trim_matches('"').to_string()),
            last_modified: out.last_modified().map(|t| t.to_string()),
            storage_class: out.storage_class().map(|s| s.as_str().to_string()),
            cache_control: out.cache_control().map(str::to_string),
            metadata: out.metadata().cloned().unwrap_or_default(),
        })
    }

    /// Range GET so a 4 GB log never lands in memory.
    pub async fn read_text(
        &self,
        profile_id: &str,
        bucket: &str,
        key: &str,
        max_bytes: i64,
    ) -> Result<TextPreview, String> {
        let (_, client) = self.conn(profile_id)?;
        let info = self.head_object(profile_id, bucket, key).await?;
        let want = max_bytes.min(info.size.max(0));
        let mut req = client.get_object().bucket(bucket).key(key);
        if want > 0 {
            req = req.range(format!("bytes=0-{}", want - 1));
        }
        let out = req.send().await.map_err(err)?;
        let bytes = out.body.collect().await.map_err(err)?.into_bytes();
        Ok(TextPreview {
            text: String::from_utf8_lossy(&bytes).into_owned(),
            size: info.size,
            truncated: info.size > want,
        })
    }

    /// Small images only — a preview click must not pull a 2 GB file.
    pub async fn read_image(
        &self,
        profile_id: &str,
        bucket: &str,
        key: &str,
        max_bytes: i64,
    ) -> Result<String, String> {
        let (_, client) = self.conn(profile_id)?;
        let info = self.head_object(profile_id, bucket, key).await?;
        if info.size > max_bytes {
            return Err(format!(
                "image is {} — over the {} preview cap, download it instead",
                info.size, max_bytes
            ));
        }
        let out = client
            .get_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map_err(err)?;
        let content_type = out
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();
        let bytes = out.body.collect().await.map_err(err)?.into_bytes();
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        Ok(format!("data:{content_type};base64,{b64}"))
    }

    pub async fn upload(
        &self,
        profile_id: &str,
        bucket: &str,
        key: &str,
        path: &str,
    ) -> Result<(), String> {
        let (_, client) = self.conn(profile_id)?;
        let body = ByteStream::from_path(path)
            .await
            .map_err(|e| format!("cannot read {path}: {e}"))?;
        client
            .put_object()
            .bucket(bucket)
            .key(key)
            .body(body)
            .send()
            .await
            .map_err(err)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_walks_up_one_level() {
        assert_eq!(parent_prefix("manga/one-piece/001/"), "manga/one-piece/");
        assert_eq!(parent_prefix("manga/"), "");
        assert_eq!(parent_prefix(""), "");
    }

    #[test]
    fn names_drop_the_trailing_slash() {
        assert_eq!(folder_name("manga/one-piece/"), "one-piece");
        assert_eq!(folder_name("manga/"), "manga");
        assert_eq!(file_name("manga/001/01.webp"), "01.webp");
        assert_eq!(file_name("top.webp"), "top.webp");
    }
}
