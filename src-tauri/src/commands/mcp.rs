use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::error::{AppError, AppResult};
use crate::mcp::{self, McpConfig, Policy, TokenRecord};
use crate::state::AppState;

const MAX_TOKEN_NAME_CHARS: usize = 64;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStateView {
    enabled: bool,
    port: u16,
    policy: Policy,
    running: bool,
    listening_port: Option<u16>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConfigInput {
    enabled: bool,
    port: u16,
    policy: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTokenView {
    name: String,
    created_at: String,
}

#[tauri::command]
pub async fn mcp_get_state(app: AppHandle, state: State<'_, AppState>) -> AppResult<McpStateView> {
    let config = mcp::load_config(&state.db).await?;
    Ok(McpStateView {
        enabled: config.enabled,
        port: config.port,
        policy: config.policy,
        running: mcp::server::listening_port(&app).is_some(),
        listening_port: mcp::server::listening_port(&app),
    })
}

#[tauri::command]
pub async fn mcp_set_config(
    app: AppHandle,
    state: State<'_, AppState>,
    input: McpConfigInput,
) -> AppResult<McpStateView> {
    let policy = Policy::parse(&input.policy)
        .ok_or_else(|| AppError::Invalid(format!("unknown mcp policy {}", input.policy)))?;
    if input.enabled && input.port < 1024 {
        return Err(AppError::Invalid(
            "mcp port must be between 1024 and 65535".into(),
        ));
    }
    let config = McpConfig {
        enabled: input.enabled,
        port: input.port,
        policy,
    };
    mcp::save_config(&state.db, &config).await?;
    mcp::server::sync(&app, &config).await?;
    mcp_get_state(app, state).await
}

#[tauri::command]
pub async fn mcp_generate_token(state: State<'_, AppState>, name: String) -> AppResult<String> {
    let name = name.trim().to_string();
    if name.is_empty()
        || name.chars().count() > MAX_TOKEN_NAME_CHARS
        || name.chars().any(|c| c.is_control())
    {
        return Err(AppError::Invalid("invalid token name".into()));
    }
    let (token, hash) = mcp::generate_token();
    let mut tokens = mcp::load_tokens(&state.db).await?;
    tokens.retain(|record| record.name != name);
    tokens.push(TokenRecord {
        name,
        hash,
        created_at: crate::domain::now(),
    });
    mcp::save_tokens(&state.db, &tokens).await?;
    Ok(token)
}

#[tauri::command]
pub async fn mcp_revoke_token(state: State<'_, AppState>, name: String) -> AppResult<()> {
    let mut tokens = mcp::load_tokens(&state.db).await?;
    let before = tokens.len();
    tokens.retain(|record| record.name != name);
    if tokens.len() == before {
        return Err(AppError::NotFound(format!("no token named {name}")));
    }
    mcp::save_tokens(&state.db, &tokens).await
}

#[tauri::command]
pub async fn mcp_list_tokens(state: State<'_, AppState>) -> AppResult<Vec<McpTokenView>> {
    Ok(mcp::load_tokens(&state.db)
        .await?
        .into_iter()
        .map(|record| McpTokenView {
            name: record.name,
            created_at: record.created_at,
        })
        .collect())
}

#[tauri::command]
pub async fn mcp_resolve_approval(
    app: AppHandle,
    request_id: String,
    approved: bool,
) -> AppResult<()> {
    if request_id.trim().is_empty() || request_id.len() > 128 {
        return Err(AppError::Invalid("invalid request id".into()));
    }
    mcp::resolve_approval(&app, &request_id, approved)
}
