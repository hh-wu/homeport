pub mod protocol;
pub mod registry;
pub mod risk;
pub mod server;
pub mod tools;

use std::collections::HashMap;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::oneshot;

use crate::error::{AppError, AppResult};
use crate::repository::settings_repo;
use crate::state::AppState;

pub const DEFAULT_PORT: u16 = 61307;
pub const APPROVAL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(110);
pub const EVENT_APPROVAL_REQUEST: &str = "mcp:approval-request";
pub const EVENT_APPROVAL_CLOSED: &str = "mcp:approval-closed";

const CONFIG_KEY: &str = "mcp.config";
const TOKENS_KEY: &str = "mcp.tokens";
const AUDIT_FILE: &str = "mcp-audit.log";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Policy {
    #[serde(rename = "confirmAll")]
    ConfirmAll,
    #[serde(rename = "confirmWrite")]
    ConfirmWrite,
    #[serde(rename = "confirmDangerous")]
    ConfirmDangerous,
    #[serde(rename = "bypass")]
    Bypass,
}

impl Policy {
    pub fn requires_approval(self, risk: risk::Risk) -> bool {
        match self {
            Policy::ConfirmAll => true,
            Policy::ConfirmWrite => risk >= risk::Risk::Write,
            Policy::ConfirmDangerous => risk >= risk::Risk::Dangerous,
            Policy::Bypass => false,
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "confirmAll" => Some(Policy::ConfirmAll),
            "confirmWrite" => Some(Policy::ConfirmWrite),
            "confirmDangerous" => Some(Policy::ConfirmDangerous),
            "bypass" => Some(Policy::Bypass),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConfig {
    pub enabled: bool,
    pub port: u16,
    pub policy: Policy,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_PORT,
            policy: Policy::ConfirmWrite,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenRecord {
    pub name: String,
    pub hash: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRequestEvent {
    pub request_id: String,
    pub client: String,
    pub command: String,
    pub hosts: Vec<String>,
    pub risk: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalClosedEvent {
    pub request_id: String,
}

#[derive(Default)]
pub struct McpState {
    pub runtime: Mutex<Option<server::ServerRuntime>>,
    pub approvals: Mutex<HashMap<String, oneshot::Sender<bool>>>,
    pub registry: registry::CommandRegistry,
}

pub async fn load_config(db: &SqlitePool) -> AppResult<McpConfig> {
    match settings_repo::get(db, CONFIG_KEY).await? {
        Some(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
        None => Ok(McpConfig::default()),
    }
}

pub async fn save_config(db: &SqlitePool, config: &McpConfig) -> AppResult<()> {
    settings_repo::set(db, CONFIG_KEY, &serde_json::to_string(config)?).await
}

pub async fn load_tokens(db: &SqlitePool) -> AppResult<Vec<TokenRecord>> {
    match settings_repo::get(db, TOKENS_KEY).await? {
        Some(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
        None => Ok(Vec::new()),
    }
}

pub async fn save_tokens(db: &SqlitePool, tokens: &[TokenRecord]) -> AppResult<()> {
    settings_repo::set(db, TOKENS_KEY, &serde_json::to_string(tokens)?).await
}

pub fn generate_token() -> (String, String) {
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let hash = hash_token(&token);
    (token, hash)
}

pub fn hash_token(token: &str) -> String {
    let digest = sha2::Sha256::digest(token.as_bytes());
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

pub async fn resolve_token(db: &SqlitePool, token: &str) -> AppResult<Option<String>> {
    if token.is_empty() {
        return Ok(None);
    }
    let hash = hash_token(token);
    Ok(load_tokens(db)
        .await?
        .into_iter()
        .find(|record| record.hash == hash)
        .map(|record| record.name))
}

pub async fn request_approval(
    app: &AppHandle,
    client: &str,
    command: &str,
    hosts: Vec<String>,
    risk: risk::Risk,
) -> AppResult<()> {
    let request_id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = oneshot::channel();
    app.state::<AppState>()
        .mcp
        .approvals
        .lock()
        .insert(request_id.clone(), tx);

    let risk_label = match risk {
        risk::Risk::Read => "read",
        risk::Risk::Write => "write",
        risk::Risk::Dangerous => "dangerous",
    };
    let _ = app.emit(
        EVENT_APPROVAL_REQUEST,
        ApprovalRequestEvent {
            request_id: request_id.clone(),
            client: client.to_string(),
            command: command.to_string(),
            hosts,
            risk: risk_label.into(),
        },
    );
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }

    let verdict = tokio::time::timeout(APPROVAL_TIMEOUT, rx).await;
    {
        let state = app.state::<AppState>();
        state.mcp.approvals.lock().remove(&request_id);
    }
    let _ = app.emit(
        EVENT_APPROVAL_CLOSED,
        ApprovalClosedEvent {
            request_id: request_id.clone(),
        },
    );

    match verdict {
        Ok(Ok(true)) => {
            audit("approval", command, Some(true), None);
            Ok(())
        }
        Ok(Ok(false)) => {
            audit("approval", command, Some(false), None);
            Err(AppError::Invalid("denied by user".into()))
        }
        Ok(Err(_)) => Err(AppError::Invalid("approval channel closed".into())),
        Err(_) => Err(AppError::Timeout(
            "approval timed out waiting for the user".into(),
        )),
    }
}

pub fn resolve_approval(app: &AppHandle, request_id: &str, approved: bool) -> AppResult<()> {
    let sender = app
        .state::<AppState>()
        .mcp
        .approvals
        .lock()
        .remove(request_id);
    match sender {
        Some(tx) => {
            let _ = tx.send(approved);
            Ok(())
        }
        None => Err(AppError::NotFound(format!(
            "no pending approval {request_id}"
        ))),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuditEntry<'a> {
    time: String,
    tool: &'a str,
    command: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    approved: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'a str>,
}

pub fn audit(tool: &str, command: &str, approved: Option<bool>, error: Option<&str>) {
    let Some(dir) = crate::paths::data_dir() else {
        return;
    };
    let entry = AuditEntry {
        time: chrono::Utc::now().to_rfc3339(),
        tool,
        command,
        approved,
        error,
    };
    let Ok(mut line) = serde_json::to_string(&entry) else {
        return;
    };
    line.push('\n');
    let path = dir.join(AUDIT_FILE);
    std::thread::spawn(move || {
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            use std::io::Write;
            let _ = file.write_all(line.as_bytes());
        }
    });
}

pub async fn sync_server(app: &AppHandle) -> AppResult<()> {
    let config = load_config(&app.state::<AppState>().db).await?;
    server::sync(app, &config).await
}
