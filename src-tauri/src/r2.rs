//! R2 (S3-compatible) client. Keys are flat: folders are just prefixes.

use crate::store;
use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::primitives::{ByteStream, DateTime};
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart, Delete, ObjectIdentifier};
use aws_sdk_s3::{Client, Config};
use base64::Engine;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub const MAX_KEYS: i32 = 1000;

/// S3 caps a single DeleteObjects request at 1000 keys.
const DELETE_BATCH: usize = 1000;

/// Pages one search may walk (50k objects) before it reports `truncated`.
const SEARCH_PAGES: usize = 50;

/// Multipart chunk size. 8 MiB keeps the part count low (a 10,000-part upload
/// still reaches 80 GB) while making progress move on multi-GB files.
const PART_SIZE: u64 = 8 * 1024 * 1024;

#[derive(Default)]
pub struct R2 {
    clients: Mutex<HashMap<String, Client>>,
    canceled: Mutex<HashSet<String>>,
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderStats {
    pub files: i64,
    pub folders: i64,
    pub size: i64,
    pub scanned_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub files: Vec<ObjectEntry>,
    /// True when the scan stopped at the page cap before the bucket ended.
    pub truncated: bool,
    pub scanned: i64,
}

/// Payload of the `transfer://progress` event. Field names already match the TS side.
#[derive(Clone, Serialize)]
pub struct Progress {
    pub id: String,
    pub transferred: i64,
    pub total: i64,
}

// ---------- pure helpers (unit tested) ----------

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

/// Folders are zero-byte objects whose key ends in `/`.
pub fn folder_key(prefix: &str) -> String {
    if prefix.ends_with('/') {
        prefix.to_string()
    } else {
        format!("{prefix}/")
    }
}

/// `CopyObject` needs the source percent-encoded and the SDK will not do it for us.
/// Unreserved bytes and `/` pass through; everything else becomes `%XX`.
pub fn encode_copy_source(bucket: &str, key: &str) -> String {
    let raw = format!("{bucket}/{key}");
    let mut out = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// The single choke point for every mutating call — commands never check this themselves.
fn writable(profile: &store::Profile) -> Result<(), String> {
    if profile.read_only {
        return Err(format!(
            "profile \"{}\" is read-only — turn that off in Profiles to make changes",
            profile.name
        ));
    }
    Ok(())
}

/// Bucket search (§17). `*` is a wildcard, everything else is a case-insensitive
/// substring match, so `*.webp`, `chapter 12` and `cover` all behave.
pub fn key_matches(query: &str, key: &str) -> bool {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }
    let key = key.to_lowercase();
    if !query.contains('*') {
        return key.contains(&query);
    }
    // Split on `*` and require the parts in order.
    let parts: Vec<&str> = query.split('*').filter(|p| !p.is_empty()).collect();
    let anchored_start = !query.starts_with('*');
    let anchored_end = !query.ends_with('*');
    let mut cursor = 0usize;
    for (i, part) in parts.iter().enumerate() {
        let Some(found) = key[cursor..].find(part) else {
            return false;
        };
        if i == 0 && anchored_start && found != 0 {
            return false;
        }
        cursor += found + part.len();
    }
    if anchored_end {
        if let Some(last) = parts.last() {
            if !key.ends_with(last) {
                return false;
            }
        }
    }
    true
}

async fn delete_keys(client: &Client, bucket: &str, keys: &[String]) -> Result<(), String> {
    for chunk in keys.chunks(DELETE_BATCH) {
        let mut objects = Vec::with_capacity(chunk.len());
        for key in chunk {
            objects.push(ObjectIdentifier::builder().key(key).build().map_err(err)?);
        }
        let delete = Delete::builder().set_objects(Some(objects)).build().map_err(err)?;
        let out = client
            .delete_objects()
            .bucket(bucket)
            .delete(delete)
            .send()
            .await
            .map_err(err)?;
        if let Some(failure) = out.errors().first() {
            return Err(format!(
                "{} could not be deleted: {}",
                failure.key().unwrap_or("?"),
                failure.message().unwrap_or("unknown error")
            ));
        }
    }
    Ok(())
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

    /// Same as `conn`, but refuses when the profile is read-only.
    fn write_conn(&self, profile_id: &str) -> Result<(store::Profile, Client), String> {
        let (profile, client) = self.conn(profile_id)?;
        writable(&profile)?;
        Ok((profile, client))
    }

    /// `None` when there is no UI attached (live tests): progress is just dropped.
    fn emit(&self, app: Option<&AppHandle>, id: &str, transferred: i64, total: i64) {
        if let Some(app) = app {
            let _ = app.emit(
                "transfer://progress",
                Progress {
                    id: id.to_string(),
                    transferred,
                    total,
                },
            );
        }
    }

    pub fn cancel_transfer(&self, id: &str) {
        self.canceled.lock().unwrap().insert(id.to_string());
    }

    fn is_canceled(&self, id: &str) -> bool {
        self.canceled.lock().unwrap().contains(id)
    }

    fn clear_cancel(&self, id: &str) {
        self.canceled.lock().unwrap().remove(id);
    }

    async fn exists(client: &Client, bucket: &str, key: &str) -> Result<bool, String> {
        match client.head_object().bucket(bucket).key(key).send().await {
            Ok(_) => Ok(true),
            Err(e) if e.as_service_error().map(|s| s.is_not_found()).unwrap_or(false) => Ok(false),
            Err(e) => Err(err(e)),
        }
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

    pub async fn test_connection(&self, profile_id: &str) -> Result<String, String> {
        let buckets = self.list_buckets(profile_id).await?;
        Ok(format!("OK — {} buckets", buckets.len()))
    }

    pub async fn presign_get(
        &self,
        profile_id: &str,
        bucket: &str,
        key: &str,
        expires_secs: i64,
    ) -> Result<String, String> {
        let (_, client) = self.conn(profile_id)?;
        let config = PresigningConfig::expires_in(Duration::from_secs(
            expires_secs.clamp(1, 604_800) as u64,
        ))
        .map_err(err)?;
        let request = client
            .get_object()
            .bucket(bucket)
            .key(key)
            .presigned(config)
            .await
            .map_err(err)?;
        Ok(request.uri().to_string())
    }

    /// Recursive scan behind the folder-size column. Counts the immediate child
    /// folders, every file underneath, and their total size.
    /// Search walks the bucket (or `prefix`) and stops at `limit` hits.
    /// ponytail: rescans on every query instead of indexing; task-005's SQLite
    /// index is what makes this instant on million-object buckets.
    pub async fn search_objects(
        &self,
        profile_id: &str,
        bucket: &str,
        prefix: &str,
        query: &str,
        limit: i64,
    ) -> Result<SearchResult, String> {
        let (_, client) = self.conn(profile_id)?;
        let limit = limit.clamp(1, 5000);
        let mut token: Option<String> = None;
        let mut files: Vec<ObjectEntry> = Vec::new();
        let mut scanned = 0i64;
        let mut truncated = false;

        for _ in 0..SEARCH_PAGES {
            let out = client
                .list_objects_v2()
                .bucket(bucket)
                .prefix(prefix)
                .set_continuation_token(token)
                .max_keys(MAX_KEYS)
                .send()
                .await
                .map_err(err)?;

            for object in out.contents() {
                let Some(raw) = object.key() else { continue };
                if raw.ends_with('/') {
                    continue;
                }
                scanned += 1;
                if !key_matches(query, raw) {
                    continue;
                }
                files.push(ObjectEntry {
                    name: file_name(raw),
                    key: raw.to_string(),
                    size: object.size().unwrap_or_default(),
                    last_modified: object.last_modified().map(|t| t.to_string()).unwrap_or_default(),
                    etag: object.e_tag().unwrap_or_default().trim_matches('"').to_string(),
                    storage_class: object
                        .storage_class()
                        .map(|s| s.as_str().to_string())
                        .unwrap_or_default(),
                });
                if files.len() as i64 >= limit {
                    return Ok(SearchResult {
                        files,
                        truncated: true,
                        scanned,
                    });
                }
            }

            if !out.is_truncated().unwrap_or(false) {
                return Ok(SearchResult { files, truncated, scanned });
            }
            token = out.next_continuation_token().map(str::to_string);
            if token.is_none() {
                return Ok(SearchResult { files, truncated, scanned });
            }
        }

        truncated = true;
        Ok(SearchResult { files, truncated, scanned })
    }

    pub async fn folder_stats(
        &self,
        profile_id: &str,
        bucket: &str,
        prefix: &str,
    ) -> Result<FolderStats, String> {
        let (_, client) = self.conn(profile_id)?;
        let mut token: Option<String> = None;
        let mut files = 0i64;
        let mut size = 0i64;
        let mut folders: HashSet<String> = HashSet::new();

        loop {
            let out = client
                .list_objects_v2()
                .bucket(bucket)
                .prefix(prefix)
                .set_continuation_token(token)
                .max_keys(MAX_KEYS)
                .send()
                .await
                .map_err(err)?;

            for object in out.contents() {
                let Some(key) = object.key() else { continue };
                if key.ends_with('/') {
                    continue;
                }
                files += 1;
                size += object.size().unwrap_or_default();
                let rest = key.strip_prefix(prefix).unwrap_or(key);
                if let Some(slash) = rest.find('/') {
                    folders.insert(rest[..=slash].to_string());
                }
            }

            if !out.is_truncated().unwrap_or(false) {
                break;
            }
            token = out.next_continuation_token().map(str::to_string);
            if token.is_none() {
                break;
            }
        }

        Ok(FolderStats {
            files,
            folders: folders.len() as i64,
            size,
            scanned_at: DateTime::from(SystemTime::now()).to_string(),
        })
    }

    pub async fn delete_objects(
        &self,
        profile_id: &str,
        bucket: &str,
        keys: Vec<String>,
    ) -> Result<(), String> {
        let (_, client) = self.write_conn(profile_id)?;
        if keys.is_empty() {
            return Ok(());
        }
        delete_keys(&client, bucket, &keys).await
    }

    /// Re-lists from the start each pass. Continuation tokens would skip objects
    /// as we delete underneath them; re-listing converges and is cheap enough.
    pub async fn delete_prefix(
        &self,
        profile_id: &str,
        bucket: &str,
        prefix: &str,
    ) -> Result<i64, String> {
        let (_, client) = self.write_conn(profile_id)?;
        let mut deleted = 0i64;

        loop {
            let out = client
                .list_objects_v2()
                .bucket(bucket)
                .prefix(prefix)
                .max_keys(MAX_KEYS)
                .send()
                .await
                .map_err(err)?;
            let keys: Vec<String> = out
                .contents()
                .iter()
                .filter_map(|o| o.key().map(str::to_string))
                .collect();
            if keys.is_empty() {
                break;
            }
            deleted += keys.len() as i64;
            delete_keys(&client, bucket, &keys).await?;
        }

        Ok(deleted)
    }

    pub async fn copy_object(
        &self,
        profile_id: &str,
        bucket: &str,
        from_key: &str,
        to_key: &str,
        replace: bool,
    ) -> Result<(), String> {
        let (_, client) = self.write_conn(profile_id)?;
        if !replace && Self::exists(&client, bucket, to_key).await? {
            return Err(format!("{to_key} already exists — pass replace to overwrite"));
        }
        client
            .copy_object()
            .bucket(bucket)
            .key(to_key)
            .copy_source(encode_copy_source(bucket, from_key))
            .send()
            .await
            .map_err(err)?;
        Ok(())
    }

    pub async fn copy_prefix(
        &self,
        profile_id: &str,
        bucket: &str,
        from_prefix: &str,
        to_prefix: &str,
        replace: bool,
    ) -> Result<i64, String> {
        let (_, client) = self.write_conn(profile_id)?;
        let mut token: Option<String> = None;
        let mut copied = 0i64;

        loop {
            let out = client
                .list_objects_v2()
                .bucket(bucket)
                .prefix(from_prefix)
                .set_continuation_token(token)
                .max_keys(MAX_KEYS)
                .send()
                .await
                .map_err(err)?;

            for object in out.contents() {
                let Some(key) = object.key() else { continue };
                let suffix = key.strip_prefix(from_prefix).unwrap_or(key);
                let to_key = format!("{to_prefix}{suffix}");
                if !replace && Self::exists(&client, bucket, &to_key).await? {
                    return Err(format!("{to_key} already exists — pass replace to overwrite"));
                }
                client
                    .copy_object()
                    .bucket(bucket)
                    .key(&to_key)
                    .copy_source(encode_copy_source(bucket, key))
                    .send()
                    .await
                    .map_err(err)?;
                copied += 1;
            }

            if !out.is_truncated().unwrap_or(false) {
                break;
            }
            token = out.next_continuation_token().map(str::to_string);
            if token.is_none() {
                break;
            }
        }

        Ok(copied)
    }

    pub async fn create_folder(
        &self,
        profile_id: &str,
        bucket: &str,
        prefix: &str,
    ) -> Result<(), String> {
        self.create_file(profile_id, bucket, &folder_key(prefix)).await
    }

    pub async fn create_file(
        &self,
        profile_id: &str,
        bucket: &str,
        key: &str,
    ) -> Result<(), String> {
        let (_, client) = self.write_conn(profile_id)?;
        client
            .put_object()
            .bucket(bucket)
            .key(key)
            .body(ByteStream::from(Vec::new()))
            .send()
            .await
            .map_err(err)?;
        Ok(())
    }

    /// One PutObject below `PART_SIZE`, multipart above it so progress moves and a
    /// cancel can abort the upload instead of leaving a half-written object.
    pub async fn upload(
        &self,
        app: Option<&AppHandle>,
        profile_id: &str,
        bucket: &str,
        key: &str,
        path: &str,
        transfer_id: &str,
        replace: bool,
    ) -> Result<(), String> {
        let (_, client) = self.write_conn(profile_id)?;
        if !replace && Self::exists(&client, bucket, key).await? {
            return Err(format!("{key} already exists — pass replace to overwrite"));
        }

        self.clear_cancel(transfer_id);
        let total = std::fs::metadata(path).map(|m| m.len()).unwrap_or_default();
        self.emit(app, transfer_id, 0, total as i64);

        if total <= PART_SIZE {
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
            self.emit(app, transfer_id, total as i64, total as i64);
            return Ok(());
        }

        self.upload_multipart(app, &client, bucket, key, path, transfer_id, total)
            .await
    }

    /// Uploads in `PART_SIZE` chunks, emitting real byte progress after each part.
    /// Only one part is in memory at a time.
    async fn upload_multipart(
        &self,
        app: Option<&AppHandle>,
        client: &Client,
        bucket: &str,
        key: &str,
        path: &str,
        transfer_id: &str,
        total: u64,
    ) -> Result<(), String> {
        let created = client
            .create_multipart_upload()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map_err(err)?;
        let upload_id = created.upload_id().unwrap_or_default().to_string();

        let mut file = tokio::fs::File::open(path)
            .await
            .map_err(|e| format!("cannot read {path}: {e}"))?;
        let mut parts = Vec::new();
        let mut buffer = vec![0u8; PART_SIZE as usize];
        let mut transferred = 0u64;
        let mut number = 1i32;

        loop {
            let read = match read_chunk(&mut file, &mut buffer).await {
                Ok(read) => read,
                Err(e) => return self.abort_multipart(client, bucket, key, &upload_id, e).await,
            };
            if read == 0 {
                break;
            }
            if self.is_canceled(transfer_id) {
                return self
                    .abort_multipart(client, bucket, key, &upload_id, "upload canceled".into())
                    .await;
            }

            let part = match client
                .upload_part()
                .bucket(bucket)
                .key(key)
                .upload_id(&upload_id)
                .part_number(number)
                .body(ByteStream::from(buffer[..read].to_vec()))
                .send()
                .await
            {
                Ok(part) => part,
                Err(e) => return self.abort_multipart(client, bucket, key, &upload_id, err(e)).await,
            };
            parts.push(
                CompletedPart::builder()
                    .part_number(number)
                    .set_e_tag(part.e_tag().map(str::to_string))
                    .build(),
            );

            transferred += read as u64;
            number += 1;
            self.emit(app, transfer_id, transferred as i64, total as i64);
        }

        client
            .complete_multipart_upload()
            .bucket(bucket)
            .key(key)
            .upload_id(&upload_id)
            .multipart_upload(CompletedMultipartUpload::builder().set_parts(Some(parts)).build())
            .send()
            .await
            .map_err(err)?;
        self.emit(app, transfer_id, total as i64, total as i64);
        Ok(())
    }

    /// Aborts a partial multipart upload so R2 does not keep paying for orphaned
    /// parts. Best effort: the caller's real error is the one reported.
    async fn abort_multipart(
        &self,
        client: &Client,
        bucket: &str,
        key: &str,
        upload_id: &str,
        error: String,
    ) -> Result<(), String> {
        let _ = client
            .abort_multipart_upload()
            .bucket(bucket)
            .key(key)
            .upload_id(upload_id)
            .send()
            .await;
        Err(error)
    }

    /// Streams to disk in chunks so a multi-GB download never lands in memory,
    /// reporting real byte progress and honouring cancellation.
    #[allow(clippy::too_many_arguments)]
    pub async fn download(
        &self,
        app: Option<&AppHandle>,
        profile_id: &str,
        bucket: &str,
        key: &str,
        dest_path: &str,
        transfer_id: &str,
    ) -> Result<(), String> {
        let (_, client) = self.conn(profile_id)?;
        self.clear_cancel(transfer_id);

        let mut out = client
            .get_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map_err(err)?;
        let total = out.content_length().unwrap_or_default();

        // Folder downloads hand us nested paths ("C:\dump\a\b\c.webp").
        if let Some(parent) = std::path::Path::new(dest_path).parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
            }
        }
        let mut file = tokio::fs::File::create(dest_path)
            .await
            .map_err(|e| format!("cannot write {dest_path}: {e}"))?;
        self.emit(app, transfer_id, 0, total);

        let mut written = 0i64;
        loop {
            let Some(chunk) = out.body.next().await else { break };
            let chunk = chunk.map_err(err)?;
            if self.is_canceled(transfer_id) {
                self.clear_cancel(transfer_id);
                drop(file);
                let _ = tokio::fs::remove_file(dest_path).await;
                return Err("transfer canceled".into());
            }
            file.write_all(&chunk)
                .await
                .map_err(|e| format!("write {dest_path}: {e}"))?;
            written += chunk.len() as i64;
            self.emit(app, transfer_id, written, total);
        }

        file.flush().await.map_err(err)?;
        self.clear_cancel(transfer_id);
        Ok(())
    }
}

/// Fills `buffer` up to its length; a short read only happens at end of file.
async fn read_chunk(file: &mut tokio::fs::File, buffer: &mut [u8]) -> Result<usize, String> {
    let mut filled = 0;
    while filled < buffer.len() {
        let read = file
            .read(&mut buffer[filled..])
            .await
            .map_err(|e| format!("upload read failed: {e}"))?;
        if read == 0 {
            break;
        }
        filled += read;
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_chunk_fills_across_short_reads() {
        let path = std::env::temp_dir().join(format!("r2-chunk-{}", std::process::id()));
        std::fs::write(&path, vec![7u8; 20]).unwrap();

        tauri::async_runtime::block_on(async {
            let mut file = tokio::fs::File::open(&path).await.unwrap();
            let mut buffer = [0u8; 8];
            assert_eq!(read_chunk(&mut file, &mut buffer).await.unwrap(), 8);
            assert_eq!(read_chunk(&mut file, &mut buffer).await.unwrap(), 8);
            // Only 4 bytes are left, and the next call reports end of file.
            assert_eq!(read_chunk(&mut file, &mut buffer).await.unwrap(), 4);
            assert_eq!(read_chunk(&mut file, &mut buffer).await.unwrap(), 0);
        });

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn folders_always_end_in_a_slash() {
        assert_eq!(folder_key("manga/one-piece"), "manga/one-piece/");
        assert_eq!(folder_key("manga/one-piece/"), "manga/one-piece/");
        assert_eq!(folder_key(""), "/");
    }

    #[test]
    fn copy_source_escapes_what_sigv4_would_otherwise_mangle() {
        assert_eq!(encode_copy_source("bucket", "a/b.webp"), "bucket/a/b.webp");
        assert_eq!(encode_copy_source("bucket", "a b+c.webp"), "bucket/a%20b%2Bc.webp");
        assert_eq!(encode_copy_source("bucket", "日本語.webp"), "bucket/%E6%97%A5%E6%9C%AC%E8%AA%9E.webp");
        assert_eq!(encode_copy_source("bucket", "50%.txt"), "bucket/50%25.txt");
    }

    fn profile(read_only: bool) -> store::Profile {
        store::Profile {
            name: "Production".into(),
            read_only,
            ..Default::default()
        }
    }

    #[test]
    fn read_only_profiles_are_refused_at_the_one_choke_point() {
        assert!(writable(&profile(false)).is_ok());
        let denied = writable(&profile(true)).unwrap_err();
        assert!(denied.contains("read-only"), "{denied}");
        assert!(denied.contains("Production"), "{denied}");
    }

    #[test]
    fn names_drop_the_trailing_slash() {
        assert_eq!(folder_name("manga/one-piece/"), "one-piece");
        assert_eq!(folder_name("manga/"), "manga");
        assert_eq!(file_name("manga/001/01.webp"), "01.webp");
        assert_eq!(file_name("top.webp"), "top.webp");
    }

    #[test]
    fn search_matches_substrings_and_wildcards() {
        assert!(key_matches("", "anything/at/all.webp"));
        assert!(key_matches("CHAPTER", "manga/Chapter 12/01.webp"));
        assert!(key_matches("*.webp", "manga/001/01.webp"));
        assert!(!key_matches("*.jpg", "manga/001/01.webp"));
        assert!(key_matches("manga/*/01.webp", "manga/one-piece/01.webp"));
        assert!(key_matches("manga/*/01.*", "manga/one-piece/01.webp"));
        assert!(!key_matches("chapter*", "manga/001/01.webp"));
        assert!(key_matches("日本語", "manga/日本語/01.webp"));
    }

    /// Real R2 against a saved profile. Skipped unless asked for:
    ///   $env:R2_PROFILE_ID="your-profile-id"; $env:R2_BUCKET="your-test-bucket"
    ///   cargo test --manifest-path src-tauri/Cargo.toml -- --ignored --nocapture live_
    /// Everything it creates lives under `_r2explorer-smoketest/` and is deleted again.
    #[tokio::test]
    #[ignore = "needs live R2 credentials; run with --ignored"]
    async fn live_smoke_round_trip() {
        let profile_id = std::env::var("R2_PROFILE_ID").expect("set R2_PROFILE_ID to your saved test profile ID");
        let bucket = std::env::var("R2_BUCKET").expect("set R2_BUCKET to your test bucket name");
        let r2 = R2::default();

        let buckets = r2.list_buckets(&profile_id).await.expect("list_buckets");
        assert!(buckets.iter().any(|b| b == &bucket), "bucket {bucket} not visible: {buckets:?}");
        println!("buckets: {buckets:?}");

        let root = r2.list_objects(&profile_id, &bucket, "", None).await.expect("list root");
        println!(
            "root: {} folders, {} files, {} bytes, more={}",
            root.folder_count, root.file_count, root.total_size, root.truncated
        );

        if let Some(folder) = root.folders.first() {
            let stats = r2
                .folder_stats(&profile_id, &bucket, &folder.prefix)
                .await
                .expect("folder_stats");
            println!(
                "stats {}: {} folders, {} files, {} bytes",
                folder.prefix, stats.folders, stats.files, stats.size
            );
            assert!(stats.files >= 0);
        }

        let hits = r2
            .search_objects(&profile_id, &bucket, "", "*.webp", 500)
            .await
            .expect("search");
        println!(
            "search *.webp: {} hits, {} scanned, truncated={}",
            hits.files.len(),
            hits.scanned,
            hits.truncated
        );
        assert!(
            hits.files.iter().all(|f| f.key.to_lowercase().ends_with(".webp")),
            "wildcard search returned non-webp keys"
        );

        // The preview/gallery path: a capped GetObject turned into a data URL.
        if let Some(image) = hits.files.first() {
            let url = r2
                .read_image(&profile_id, &bucket, &image.key, 4_000_000)
                .await
                .expect("read_image");
            assert!(url.starts_with("data:image/"), "unexpected data URL: {}", &url[..30.min(url.len())]);
            println!("read_image {} -> {} KB data URL", image.name, url.len() / 1024);
        }
        if let Some(textlike) = hits.files.iter().find(|f| f.key.ends_with(".txt") || f.key.ends_with(".json")) {
            let preview = r2.read_text(&profile_id, &bucket, &textlike.key, 4096).await.expect("read_text");
            println!("read_text {} -> {} bytes", textlike.name, preview.text.len());
        }
        let none = r2
            .search_objects(&profile_id, &bucket, "", "no-such-key-xyzzy", 50)
            .await
            .expect("search miss");
        assert!(none.files.is_empty(), "substring search matched nothing expected");

        // Round trip: upload -> head -> download -> byte compare -> delete.
        let stamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let key = format!("_r2explorer-smoketest/{stamp}/hello.txt");
        let body = format!("r2 explorer live smoke {stamp} — 日本語 ünïcode\n").into_bytes();
        let local = std::env::temp_dir().join(format!("r2-smoke-{stamp}.txt"));
        std::fs::write(&local, &body).unwrap();

        r2.upload(None, &profile_id, &bucket, &key, &local.to_string_lossy(), "live-1", true)
            .await
            .expect("upload");

        let info = r2.head_object(&profile_id, &bucket, &key).await.expect("head");
        assert_eq!(info.size, body.len() as i64);

        let back = std::env::temp_dir().join(format!("r2-smoke-{stamp}-back.txt"));
        r2.download(None, &profile_id, &bucket, &key, &back.to_string_lossy(), "live-2")
            .await
            .expect("download");
        assert_eq!(std::fs::read(&back).unwrap(), body, "downloaded bytes differ");

        let url = r2.presign_get(&profile_id, &bucket, &key, 300).await.expect("presign");
        println!("presigned: {url}");

        // The URL is only worth anything if R2 accepts the signature.
        let code = std::process::Command::new("curl")
            .args(["-s", "-o", "NUL", "-w", "%{http_code}", &url])
            .output()
            .expect("curl");
        let code = String::from_utf8_lossy(&code.stdout).trim().to_string();
        println!("presigned GET -> {code}");
        assert_eq!(code, "200", "presigned URL rejected");

        // Keys with spaces and non-ASCII are where CopyObject's source encoding bites.
        let odd = format!("_r2explorer-smoketest/{stamp}/日本語 ünïcode + space.txt");
        let odd_copy = format!("_r2explorer-smoketest/{stamp}/日本語 ünïcode + space (copy).txt");
        r2.create_file(&profile_id, &bucket, &odd).await.expect("create unicode key");
        r2.copy_object(&profile_id, &bucket, &odd, &odd_copy, false).await.expect("copy unicode key");
        let client = r2.client_for(&store::profile_by_id(&profile_id).unwrap()).unwrap();
        assert!(R2::exists(&client, &bucket, &odd_copy).await.unwrap(), "copy missing");
        r2.delete_objects(&profile_id, &bucket, vec![odd, odd_copy]).await.expect("delete pair");
        println!("unicode copy OK");

        // Multipart path: > PART_SIZE goes through CreateMultipartUpload/UploadPart.
        let big = std::env::temp_dir().join(format!("r2-smoke-{stamp}-big.bin"));
        let payload: Vec<u8> = (0..10 * 1024 * 1024u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&big, &payload).unwrap();
        let big_key = format!("_r2explorer-smoketest/{stamp}/big.bin");
        r2.upload(None, &profile_id, &bucket, &big_key, &big.to_string_lossy(), "live-3", true)
            .await
            .expect("multipart upload");
        assert_eq!(
            r2.head_object(&profile_id, &bucket, &big_key).await.unwrap().size,
            payload.len() as i64
        );
        let big_back = std::env::temp_dir().join(format!("r2-smoke-{stamp}-big-back.bin"));
        r2.download(None, &profile_id, &bucket, &big_key, &big_back.to_string_lossy(), "live-4")
            .await
            .expect("download multipart");
        assert_eq!(std::fs::read(&big_back).unwrap().len(), payload.len());
        r2.delete_objects(&profile_id, &bucket, vec![big_key]).await.expect("delete big");
        println!("multipart 10 MiB OK");

        r2.delete_objects(&profile_id, &bucket, vec![key.clone()]).await.expect("delete");
        assert!(
            !R2::exists(&r2.client_for(&store::profile_by_id(&profile_id).unwrap()).unwrap(), &bucket, &key)
                .await
                .unwrap(),
            "object survived delete"
        );
        r2.delete_prefix(&profile_id, &bucket, "_r2explorer-smoketest/").await.ok();
        let _ = std::fs::remove_file(&local);
        let _ = std::fs::remove_file(&back);
        let _ = std::fs::remove_file(&big);
        let _ = std::fs::remove_file(&big_back);
        println!("live smoke OK");
    }
}
