use serde::Serialize;
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

const MAX_KEY_CHARS: usize = 128;
const MAX_VALUE_CHARS: usize = 4000;
const MAX_LIST: usize = 100;

const SENSITIVE_WORDS: [&str; 6] = [
    "password",
    "passwd",
    "secret",
    "token",
    "passphrase",
    "credential",
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiMemoryEntry {
    key: String,
    value: String,
    updated_at: String,
}

fn validate_entry(key: &str, value: &str) -> AppResult<(String, String)> {
    let key = key.trim();
    let value = value.trim();
    if key.is_empty() || key.chars().count() > MAX_KEY_CHARS {
        return Err(AppError::Invalid("memory key is invalid".into()));
    }
    if value.is_empty() || value.chars().count() > MAX_VALUE_CHARS {
        return Err(AppError::Invalid("memory value is invalid".into()));
    }
    let lowered = key.to_lowercase();
    if SENSITIVE_WORDS
        .iter()
        .any(|word| lowered.contains(word))
    {
        return Err(AppError::Invalid(
            "memory keys must not reference secrets; use the credential vault for those".into(),
        ));
    }
    Ok((key.to_string(), value.to_string()))
}

#[tauri::command]
pub async fn ai_memory_set(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> AppResult<()> {
    let (key, value) = validate_entry(&key, &value)?;
    sqlx::query(
        "INSERT INTO ai_memory (key, value, updated_at) VALUES (?, ?, ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )
    .bind(&key)
    .bind(&value)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(&state.db)
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn ai_memory_recall(
    state: State<'_, AppState>,
    query: Option<String>,
    limit: Option<usize>,
) -> AppResult<Vec<AiMemoryEntry>> {
    let cap = limit.unwrap_or(20).clamp(1, MAX_LIST) as i64;
    let rows = match query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        Some(needle) => {
            let like = format!("%{}%", needle.replace(['%', '_'], " "));
            sqlx::query_as::<_, (String, String, String)>(
                "SELECT key, value, updated_at FROM ai_memory \
                 WHERE key LIKE ? OR value LIKE ? ORDER BY updated_at DESC LIMIT ?",
            )
            .bind(&like)
            .bind(&like)
            .bind(cap)
            .fetch_all(&state.db)
            .await?
        }
        None => {
            sqlx::query_as::<_, (String, String, String)>(
                "SELECT key, value, updated_at FROM ai_memory \
                 ORDER BY updated_at DESC LIMIT ?",
            )
            .bind(cap)
            .fetch_all(&state.db)
            .await?
        }
    };
    Ok(rows
        .into_iter()
        .map(|(key, value, updated_at)| AiMemoryEntry {
            key,
            value,
            updated_at,
        })
        .collect())
}

#[tauri::command]
pub async fn ai_memory_delete(state: State<'_, AppState>, key: String) -> AppResult<()> {
    sqlx::query("DELETE FROM ai_memory WHERE key = ?")
        .bind(key.trim())
        .execute(&state.db)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_memory_entries() {
        assert!(validate_entry("fleet/vps", "frps container 1Panel-frps-Q8T9").is_ok());
        assert!(validate_entry("", "value").is_err());
        assert!(validate_entry("key", "").is_err());
        assert!(validate_entry(&"k".repeat(200), "value").is_err());
        assert!(validate_entry("key", &"v".repeat(5000)).is_err());
    }

    #[test]
    fn rejects_secret_flavored_keys() {
        for key in [
            "vps root password",
            "aliyun_secret",
            "frp token",
            "ssh passphrase",
            "db credential",
        ] {
            assert!(validate_entry(key, "x").is_err(), "accepted {key}");
        }
        assert!(validate_entry("vps ssh port", "22").is_ok());
    }
}
