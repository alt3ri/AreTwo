mod dpapi;
mod r2;
mod store;

use r2::{FolderStats, ListPage, ObjectInfo, R2, TextPreview};
use tauri::{AppHandle, State};

#[tauri::command]
fn list_profiles() -> Result<Vec<store::Profile>, String> {
    store::load_profiles()
}

#[tauri::command]
fn save_profile(profile: store::Profile, secret: Option<String>) -> Result<(), String> {
    store::save_profile(profile, secret)
}

#[tauri::command]
fn delete_profile(id: String) -> Result<(), String> {
    store::delete_profile(&id)
}

#[tauri::command]
async fn list_buckets(r2: State<'_, R2>, profile_id: String) -> Result<Vec<String>, String> {
    r2.list_buckets(&profile_id).await
}

#[tauri::command]
async fn list_objects(
    r2: State<'_, R2>,
    profile_id: String,
    bucket: String,
    prefix: String,
    token: Option<String>,
) -> Result<ListPage, String> {
    r2.list_objects(&profile_id, &bucket, &prefix, token).await
}

#[tauri::command]
async fn head_object(
    r2: State<'_, R2>,
    profile_id: String,
    bucket: String,
    key: String,
) -> Result<ObjectInfo, String> {
    r2.head_object(&profile_id, &bucket, &key).await
}

#[tauri::command]
async fn read_text(
    r2: State<'_, R2>,
    profile_id: String,
    bucket: String,
    key: String,
    max_bytes: i64,
) -> Result<TextPreview, String> {
    r2.read_text(&profile_id, &bucket, &key, max_bytes).await
}

#[tauri::command]
async fn read_image(
    r2: State<'_, R2>,
    profile_id: String,
    bucket: String,
    key: String,
    max_bytes: i64,
) -> Result<String, String> {
    r2.read_image(&profile_id, &bucket, &key, max_bytes).await
}

#[tauri::command]
async fn upload_object(
    app: AppHandle,
    r2: State<'_, R2>,
    profile_id: String,
    bucket: String,
    key: String,
    path: String,
    transfer_id: String,
    replace: bool,
) -> Result<(), String> {
    r2.upload(&app, &profile_id, &bucket, &key, &path, &transfer_id, replace)
        .await
}

#[tauri::command]
async fn download_object(
    app: AppHandle,
    r2: State<'_, R2>,
    profile_id: String,
    bucket: String,
    key: String,
    dest_path: String,
    transfer_id: String,
) -> Result<(), String> {
    r2.download(&app, &profile_id, &bucket, &key, &dest_path, &transfer_id)
        .await
}

#[tauri::command]
fn cancel_transfer(r2: State<'_, R2>, transfer_id: String) {
    r2.cancel_transfer(&transfer_id);
}

#[tauri::command]
async fn test_connection(r2: State<'_, R2>, profile_id: String) -> Result<String, String> {
    r2.test_connection(&profile_id).await
}

#[tauri::command]
async fn presign_get(
    r2: State<'_, R2>,
    profile_id: String,
    bucket: String,
    key: String,
    expires_secs: i64,
) -> Result<String, String> {
    r2.presign_get(&profile_id, &bucket, &key, expires_secs).await
}

#[tauri::command]
async fn folder_stats(
    r2: State<'_, R2>,
    profile_id: String,
    bucket: String,
    prefix: String,
) -> Result<FolderStats, String> {
    r2.folder_stats(&profile_id, &bucket, &prefix).await
}

#[tauri::command]
async fn delete_objects(
    r2: State<'_, R2>,
    profile_id: String,
    bucket: String,
    keys: Vec<String>,
) -> Result<(), String> {
    r2.delete_objects(&profile_id, &bucket, keys).await
}

#[tauri::command]
async fn delete_prefix(
    r2: State<'_, R2>,
    profile_id: String,
    bucket: String,
    prefix: String,
) -> Result<i64, String> {
    r2.delete_prefix(&profile_id, &bucket, &prefix).await
}

#[tauri::command]
async fn copy_object(
    r2: State<'_, R2>,
    profile_id: String,
    bucket: String,
    from_key: String,
    to_key: String,
    replace: bool,
) -> Result<(), String> {
    r2.copy_object(&profile_id, &bucket, &from_key, &to_key, replace)
        .await
}

#[tauri::command]
async fn copy_prefix(
    r2: State<'_, R2>,
    profile_id: String,
    bucket: String,
    from_prefix: String,
    to_prefix: String,
    replace: bool,
) -> Result<i64, String> {
    r2.copy_prefix(&profile_id, &bucket, &from_prefix, &to_prefix, replace)
        .await
}

#[tauri::command]
async fn create_folder(
    r2: State<'_, R2>,
    profile_id: String,
    bucket: String,
    prefix: String,
) -> Result<(), String> {
    r2.create_folder(&profile_id, &bucket, &prefix).await
}

#[tauri::command]
async fn create_file(
    r2: State<'_, R2>,
    profile_id: String,
    bucket: String,
    key: String,
) -> Result<(), String> {
    r2.create_file(&profile_id, &bucket, &key).await
}

#[tauri::command]
fn reveal_in_explorer(path: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("explorer")
            .raw_arg(format!("/select,\"{path}\""))
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("cannot open Explorer: {e}"))
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err("reveal is only implemented on Windows".to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(R2::default())
        .invoke_handler(tauri::generate_handler![
            list_profiles,
            save_profile,
            delete_profile,
            list_buckets,
            list_objects,
            head_object,
            read_text,
            read_image,
            upload_object,
            download_object,
            cancel_transfer,
            test_connection,
            presign_get,
            folder_stats,
            delete_objects,
            delete_prefix,
            copy_object,
            copy_prefix,
            create_folder,
            create_file,
            reveal_in_explorer
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
