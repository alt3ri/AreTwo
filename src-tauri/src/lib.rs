mod dpapi;
mod r2;
mod store;

use r2::{ListPage, ObjectInfo, R2, TextPreview};
use tauri::State;

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
    r2: State<'_, R2>,
    profile_id: String,
    bucket: String,
    key: String,
    path: String,
) -> Result<(), String> {
    r2.upload(&profile_id, &bucket, &key, &path).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
            upload_object
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
