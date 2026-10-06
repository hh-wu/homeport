use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::repository::settings_repo;
use crate::state::AppState;

const KEY_LOCAL_ROOTS: &str = "ai.localRoots";

const MAX_LIST_ENTRIES: usize = 500;
const MAX_READ_FILE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_WRITE_BYTES: usize = 1024 * 1024;
const DEFAULT_READ_LIMIT: usize = 400;
const MAX_READ_LIMIT: usize = 2000;
const MAX_SEARCH_RESULTS: usize = 200;
const MAX_SEARCH_FILE_BYTES: u64 = 1024 * 1024;
const MAX_SEARCH_DEPTH: usize = 12;
const MAX_COMMAND_CHARS: usize = 8192;
const MAX_OUTPUT_CHARS: usize = 64 * 1024;
const DEFAULT_TIMEOUT_SECS: u64 = 120;
const MAX_TIMEOUT_SECS: u64 = 600;

const DENIED_SEGMENTS: [&str; 4] = [".ssh", ".aliyun", ".gnupg", ".aws"];

fn home_dir() -> String {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default()
}

fn expand_tilde(input: &str) -> String {
    if let Some(rest) = input.strip_prefix('~') {
        let home = home_dir();
        if !home.is_empty() {
            return format!("{home}{rest}");
        }
    }
    input.to_string()
}

fn default_roots() -> Vec<PathBuf> {
    let home = home_dir();
    if home.is_empty() {
        Vec::new()
    } else {
        vec![PathBuf::from(home)]
    }
}

async fn allowed_roots(db: &SqlitePool) -> AppResult<Vec<PathBuf>> {
    let raw = settings_repo::get(db, KEY_LOCAL_ROOTS)
        .await?
        .unwrap_or_default();
    let mut roots: Vec<PathBuf> = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| PathBuf::from(expand_tilde(line)))
        .collect();
    if roots.is_empty() {
        roots = default_roots();
    }
    Ok(roots)
}

fn denied_path(path: &Path) -> bool {
    path.components().any(|component| {
        let text = component.as_os_str().to_string_lossy();
        DENIED_SEGMENTS
            .iter()
            .any(|denied| text.eq_ignore_ascii_case(denied))
    })
}

fn canonical_or_self(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn is_within(candidate: &Path, roots: &[PathBuf]) -> bool {
    let resolved = canonical_or_self(candidate);
    roots
        .iter()
        .any(|root| resolved.starts_with(canonical_or_self(root)))
}

fn resolve_path(input: &str, roots: &[PathBuf], must_exist: bool) -> AppResult<PathBuf> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 512 {
        return Err(AppError::Invalid("path is invalid".into()));
    }
    let candidate = PathBuf::from(expand_tilde(trimmed));
    if !candidate.is_absolute() {
        return Err(AppError::Invalid("path must be absolute".into()));
    }
    let resolved = if must_exist {
        fs::canonicalize(&candidate)
            .map_err(|_| AppError::NotFound(candidate.display().to_string()))?
    } else {
        match fs::canonicalize(&candidate) {
            Ok(value) => value,
            Err(_) => {
                let parent = candidate
                    .parent()
                    .ok_or_else(|| AppError::Invalid("path is invalid".into()))?;
                let base = fs::canonicalize(parent)
                    .map_err(|_| AppError::NotFound(parent.display().to_string()))?;
                let name = candidate
                    .file_name()
                    .ok_or_else(|| AppError::Invalid("path is invalid".into()))?;
                base.join(name)
            }
        }
    };
    if !is_within(&resolved, roots) {
        return Err(AppError::Invalid(format!(
            "path is outside the allowed directories: {trimmed}"
        )));
    }
    if denied_path(&resolved) {
        return Err(AppError::Invalid(format!(
            "path is in a protected directory: {trimmed}"
        )));
    }
    Ok(resolved)
}

fn read_text_file(path: &Path) -> AppResult<String> {
    let meta = fs::metadata(path)
        .map_err(|_| AppError::NotFound(path.display().to_string()))?;
    if meta.len() > MAX_READ_FILE_BYTES {
        return Err(AppError::Invalid(format!(
            "file is too large ({} bytes, limit {})",
            meta.len(),
            MAX_READ_FILE_BYTES
        )));
    }
    let bytes =
        fs::read(path).map_err(|e| AppError::Other(format!("cannot read {}: {e}", path.display())))?;
    if bytes.iter().take(8192).any(|byte| *byte == 0) {
        return Err(AppError::Invalid("file looks binary".into()));
    }
    String::from_utf8(bytes).map_err(|_| AppError::Invalid("file is not utf-8 text".into()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalEntry {
    name: String,
    kind: String,
    size: u64,
    modified: String,
}

#[tauri::command]
pub async fn local_list_directory(
    state: State<'_, AppState>,
    path: String,
) -> AppResult<Vec<LocalEntry>> {
    let roots = allowed_roots(&state.db).await?;
    let dir = resolve_path(&path, &roots, true)?;
    let read = fs::read_dir(&dir)
        .map_err(|e| AppError::Other(format!("cannot list {}: {e}", dir.display())))?;
    let mut entries = Vec::new();
    for item in read.flatten().take(MAX_LIST_ENTRIES + 1) {
        let meta = item.metadata().ok();
        let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
        entries.push(LocalEntry {
            name: item.file_name().to_string_lossy().to_string(),
            kind: if is_dir { "dir" } else { "file" }.into(),
            size: if is_dir {
                0
            } else {
                meta.as_ref().map_or(0, |m| m.len())
            },
            modified: meta
                .and_then(|m| m.modified().ok())
                .map(|t| {
                    chrono::DateTime::<chrono::Local>::from(t)
                        .format("%Y-%m-%d %H:%M")
                        .to_string()
                })
                .unwrap_or_default(),
        });
    }
    entries.sort_by(|a, b| {
        (a.kind != "dir")
            .cmp(&(b.kind != "dir"))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    entries.truncate(MAX_LIST_ENTRIES);
    Ok(entries)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalFileContent {
    content: String,
    total_lines: usize,
    truncated: bool,
}

#[tauri::command]
pub async fn local_read_file(
    state: State<'_, AppState>,
    path: String,
    offset: Option<usize>,
    limit: Option<usize>,
) -> AppResult<LocalFileContent> {
    let roots = allowed_roots(&state.db).await?;
    let file = resolve_path(&path, &roots, true)?;
    let text = read_text_file(&file)?;
    let lines: Vec<&str> = text.lines().collect();
    let start = offset.unwrap_or(0).min(lines.len());
    let count = limit.unwrap_or(DEFAULT_READ_LIMIT).clamp(1, MAX_READ_LIMIT);
    let end = (start + count).min(lines.len());
    Ok(LocalFileContent {
        content: lines[start..end].join("\n"),
        total_lines: lines.len(),
        truncated: end < lines.len(),
    })
}

#[tauri::command]
pub async fn local_write_file(
    state: State<'_, AppState>,
    path: String,
    content: String,
) -> AppResult<()> {
    if content.len() > MAX_WRITE_BYTES {
        return Err(AppError::Invalid(format!(
            "content is too large ({} bytes, limit {MAX_WRITE_BYTES})",
            content.len()
        )));
    }
    let roots = allowed_roots(&state.db).await?;
    let file = resolve_path(&path, &roots, false)?;
    fs::write(&file, content)
        .map_err(|e| AppError::Other(format!("cannot write {}: {e}", file.display())))
}

#[tauri::command]
pub async fn local_edit_file(
    state: State<'_, AppState>,
    path: String,
    old_string: String,
    new_string: String,
    replace_all: Option<bool>,
) -> AppResult<usize> {
    if old_string.is_empty() {
        return Err(AppError::Invalid("old_string must not be empty".into()));
    }
    if old_string.len() > MAX_WRITE_BYTES || new_string.len() > MAX_WRITE_BYTES {
        return Err(AppError::Invalid("edit payload is too large".into()));
    }
    let roots = allowed_roots(&state.db).await?;
    let file = resolve_path(&path, &roots, true)?;
    let text = read_text_file(&file)?;
    let matches = text.matches(&old_string).count();
    if matches == 0 {
        return Err(AppError::Invalid("old_string was not found".into()));
    }
    let all = replace_all.unwrap_or(false);
    if matches > 1 && !all {
        return Err(AppError::Invalid(format!(
            "old_string matches {matches} times; pass replaceAll to change them all"
        )));
    }
    let updated = if all {
        text.replace(&old_string, &new_string)
    } else {
        text.replacen(&old_string, &new_string, 1)
    };
    fs::write(&file, updated)
        .map_err(|e| AppError::Other(format!("cannot write {}: {e}", file.display())))?;
    Ok(if all { matches } else { 1 })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSearchHit {
    path: String,
    line: usize,
    text: String,
}

fn walk_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>, cap: usize) {
    if depth > MAX_SEARCH_DEPTH || out.len() >= cap {
        return;
    }
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    for item in read.flatten() {
        if out.len() >= cap {
            return;
        }
        let name = item.file_name();
        let name_text = name.to_string_lossy();
        if name_text.starts_with('.') {
            continue;
        }
        let path = item.path();
        let Ok(meta) = item.metadata() else {
            continue;
        };
        if meta.is_dir() {
            if !denied_path(&path) {
                walk_files(&path, depth + 1, out, cap);
            }
        } else if meta.len() <= MAX_SEARCH_FILE_BYTES {
            out.push(path);
        }
    }
}

#[tauri::command]
pub async fn local_search_files(
    state: State<'_, AppState>,
    root: String,
    pattern: String,
    max: Option<usize>,
) -> AppResult<Vec<LocalSearchHit>> {
    let needle = pattern.trim();
    if needle.is_empty() || needle.chars().count() > 256 {
        return Err(AppError::Invalid("pattern is invalid".into()));
    }
    let roots = allowed_roots(&state.db).await?;
    let dir = resolve_path(&root, &roots, true)?;
    let cap = max.unwrap_or(50).clamp(1, MAX_SEARCH_RESULTS);
    let lowered = needle.to_lowercase();
    let mut files = Vec::new();
    walk_files(&dir, 0, &mut files, 4000);
    let mut hits = Vec::new();
    'files: for file in files {
        let Ok(text) = read_text_file(&file) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            if line.to_lowercase().contains(&lowered) {
                hits.push(LocalSearchHit {
                    path: file.display().to_string(),
                    line: index + 1,
                    text: line.chars().take(200).collect(),
                });
                if hits.len() >= cap {
                    break 'files;
                }
            }
        }
    }
    Ok(hits)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalCommandResult {
    exit_code: i32,
    output: String,
    timed_out: bool,
}

#[tauri::command]
pub async fn local_run_command(
    state: State<'_, AppState>,
    command: String,
    timeout_secs: Option<u64>,
) -> AppResult<LocalCommandResult> {
    let trimmed = command.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_COMMAND_CHARS {
        return Err(AppError::Invalid("command is invalid".into()));
    }
    let _ = state;
    let timeout = timeout_secs
        .unwrap_or(DEFAULT_TIMEOUT_SECS)
        .clamp(1, MAX_TIMEOUT_SECS);
    let mut cmd = tokio::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", trimmed])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let run = cmd.output();
    let output = match tokio::time::timeout(Duration::from_secs(timeout), run).await {
        Ok(result) => result.map_err(|e| AppError::Other(format!("cannot run command: {e}")))?,
        Err(_) => {
            return Ok(LocalCommandResult {
                exit_code: -1,
                output: format!("command timed out after {timeout}s"),
                timed_out: true,
            });
        }
    };
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    if !stderr.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(stderr);
    }
    if text.chars().count() > MAX_OUTPUT_CHARS {
        text = text.chars().take(MAX_OUTPUT_CHARS).collect();
        text.push_str("\n…(output truncated)");
    }
    Ok(LocalCommandResult {
        exit_code: output.status.code().unwrap_or(-1),
        output: text,
        timed_out: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_tilde_against_the_home_dir() {
        let home = home_dir();
        if home.is_empty() {
            return;
        }
        assert_eq!(
            expand_tilde("~\\Documents"),
            format!("{home}\\Documents")
        );
        assert_eq!(expand_tilde("C:\\x"), "C:\\x");
    }

    #[test]
    fn denies_protected_segments() {
        assert!(denied_path(Path::new(r"C:\Users\me\.ssh\id_ed25519")));
        assert!(denied_path(Path::new(r"C:\Users\me\.aliyun\credentials.json")));
        assert!(!denied_path(Path::new(r"C:\Users\me\Documents\notes.txt")));
    }

    #[test]
    fn containment_uses_roots() {
        let roots = vec![PathBuf::from(r"C:\Users\me")];
        assert!(is_within(Path::new(r"C:\Users\me\Documents"), &roots));
        assert!(!is_within(Path::new(r"C:\Windows\System32"), &roots));
        assert!(!is_within(Path::new(r"C:\Users\me2"), &roots));
    }

    #[test]
    fn rejects_invalid_paths_before_touching_the_disk() {
        let roots = vec![PathBuf::from(r"C:\Users\me")];
        assert!(resolve_path("", &roots, false).is_err());
        assert!(resolve_path("relative\\path", &roots, false).is_err());
        assert!(resolve_path(r"C:\Users\me\.ssh\config", &roots, false).is_err());
        assert!(resolve_path(r"D:\elsewhere\file.txt", &roots, false).is_err());
    }
}
