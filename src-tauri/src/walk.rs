//! Turns a dropped/picked path into an upload list.
//!
//! R2 has no folders, so a dropped directory becomes one object per file with the
//! directory name kept as the key prefix (`photos/2026/a.jpg`).
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadFile {
    /// Absolute path on disk.
    pub path: String,
    /// `/`-separated path relative to the dropped root — the R2 key suffix.
    pub rel: String,
}

/// Expands paths into uploadable files. Directories are walked recursively.
/// Symlinks are skipped: a junction loop would otherwise never terminate the walk.
#[tauri::command]
pub async fn expand_upload_paths(paths: Vec<String>) -> Result<Vec<UploadFile>, String> {
    let mut out = Vec::new();
    for raw in paths {
        let path = PathBuf::from(&raw);
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };
        if path.is_dir() {
            walk(&path, &name, &mut out)?;
        } else if path.is_file() {
            out.push(UploadFile { path: raw, rel: name });
        }
    }
    Ok(out)
}

fn walk(dir: &Path, rel: &str, out: &mut Vec<UploadFile>) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
        // file_type() does not follow symlinks, unlike metadata().
        let kind = entry
            .file_type()
            .map_err(|e| format!("cannot stat {}: {e}", entry.path().display()))?;
        if kind.is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let child = format!("{rel}/{name}");
        let path = entry.path();
        if kind.is_dir() {
            walk(&path, &child, out)?;
        } else if kind.is_file() {
            out.push(UploadFile { path: path.to_string_lossy().to_string(), rel: child });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_a_tree_into_prefixed_relative_paths() {
        let root = std::env::temp_dir().join(format!("r2-walk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("sub/deeper")).unwrap();
        std::fs::write(root.join("a.txt"), b"a").unwrap();
        std::fs::write(root.join("sub/b webp.webp"), b"b").unwrap();
        std::fs::write(root.join("sub/deeper/c.txt"), b"c").unwrap();

        let name = root.file_name().unwrap().to_string_lossy().to_string();
        let mut files =
            tauri::async_runtime::block_on(expand_upload_paths(vec![root.to_string_lossy().to_string()]))
                .unwrap();
        files.sort_by(|a, b| a.rel.cmp(&b.rel));
        let rels: Vec<String> = files.iter().map(|f| f.rel.clone()).collect();
        assert_eq!(
            rels,
            vec![
                format!("{name}/a.txt"),
                format!("{name}/sub/b webp.webp"),
                format!("{name}/sub/deeper/c.txt"),
            ]
        );

        // A single file drops in as just its own name.
        let one = tauri::async_runtime::block_on(expand_upload_paths(vec![
            root.join("a.txt").to_string_lossy().to_string(),
        ]))
        .unwrap();
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].rel, "a.txt");

        std::fs::remove_dir_all(&root).unwrap();
    }
}
