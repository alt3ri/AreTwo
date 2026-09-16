//! Profile + credential storage: %APPDATA%\R2Explorer\{profiles.json,credentials.dat}
//! Secrets are DPAPI-sealed before they ever touch disk.

use crate::dpapi;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub account_id: String,
    pub access_key_id: String,
    pub endpoint: Option<String>,
    pub default_bucket: Option<String>,
    pub read_only: bool,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct ProfileFile {
    profiles: Vec<Profile>,
}

pub fn data_dir() -> Result<PathBuf, String> {
    let base = std::env::var("APPDATA").map_err(|_| "APPDATA is not set".to_string())?;
    let dir = PathBuf::from(base).join("R2Explorer");
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    Ok(dir)
}

fn profiles_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("profiles.json"))
}

fn secrets_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("credentials.dat"))
}

/// id -> sealed secret, base64 so the file stays valid JSON.
fn load_secrets() -> Result<BTreeMap<String, String>, String> {
    let path = secrets_path()?;
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("read credentials: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("parse credentials: {e}"))
}

fn write_secrets(map: &BTreeMap<String, String>) -> Result<(), String> {
    let json = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    std::fs::write(secrets_path()?, json).map_err(|e| format!("write credentials: {e}"))
}

pub fn write_atomic(path: &PathBuf, contents: &str) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, contents).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("replace {}: {e}", path.display()))
}

pub fn load_profiles() -> Result<Vec<Profile>, String> {
    let path = profiles_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("read profiles: {e}"))?;
    let file: ProfileFile =
        serde_json::from_str(&raw).map_err(|e| format!("parse profiles.json: {e}"))?;
    Ok(file.profiles)
}

pub fn save_profile(profile: Profile, secret: Option<String>) -> Result<(), String> {
    if profile.id.trim().is_empty() {
        return Err("profile id is required".into());
    }
    let mut profiles = load_profiles()?;
    match profiles.iter_mut().find(|p| p.id == profile.id) {
        Some(existing) => *existing = profile.clone(),
        None => profiles.push(profile.clone()),
    }
    let json = serde_json::to_string_pretty(&ProfileFile { profiles }).map_err(|e| e.to_string())?;
    write_atomic(&profiles_path()?, &json)?;

    if let Some(secret) = secret {
        let sealed = dpapi::protect(secret.as_bytes())?;
        let mut secrets = load_secrets()?;
        secrets.insert(profile.id, base64::engine::general_purpose::STANDARD.encode(sealed));
        write_secrets(&secrets)?;
    }
    Ok(())
}

pub fn delete_profile(id: &str) -> Result<(), String> {
    let mut profiles = load_profiles()?;
    profiles.retain(|p| p.id != id);
    let json = serde_json::to_string_pretty(&ProfileFile { profiles }).map_err(|e| e.to_string())?;
    write_atomic(&profiles_path()?, &json)?;

    let mut secrets = load_secrets()?;
    if secrets.remove(id).is_some() {
        write_secrets(&secrets)?;
    }
    Ok(())
}

pub fn secret_for(id: &str) -> Result<String, String> {
    let encoded = load_secrets()?
        .get(id)
        .cloned()
        .ok_or_else(|| format!("no saved secret for profile {id}"))?;
    let sealed = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| format!("credentials.dat is corrupt: {e}"))?;
    let plain = dpapi::unprotect(&sealed)?;
    String::from_utf8(plain).map_err(|_| "stored secret is not valid UTF-8".to_string())
}

pub fn profile_by_id(id: &str) -> Result<Profile, String> {
    load_profiles()?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("unknown profile {id}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_file_roundtrips_with_camel_case_json() {
        let p = Profile {
            id: "production".into(),
            name: "Production".into(),
            account_id: "acct".into(),
            access_key_id: "key".into(),
            read_only: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&ProfileFile { profiles: vec![p] }).unwrap();
        assert!(json.contains("\"accountId\""), "UI and file share camelCase: {json}");
        assert!(json.contains("\"accessKeyId\""));
        let back: ProfileFile = serde_json::from_str(&json).unwrap();
        assert_eq!(back.profiles[0].account_id, "acct");
        assert!(back.profiles[0].read_only);
    }
}
