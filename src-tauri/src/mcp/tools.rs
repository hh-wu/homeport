use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tokio::sync::Semaphore;

use super::registry::{CommandEntry, HostResult};
use super::risk;
use crate::commands::ssh::build_hops;
use crate::error::{AppError, AppResult};
use crate::repository::{group_repo, host_repo};
use crate::ssh::establish;
use crate::ssh::exec::exec_capture_limited;
use crate::state::AppState;

const MAX_HOSTS: usize = 100;
const MAX_COMMAND_CHARS: usize = 32 * 1024;
const MAX_OUTPUT_BYTES: usize = 256 * 1024;
const EXEC_CONCURRENCY: usize = 4;
const HARD_COMMAND_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const DEFAULT_SYNC_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_SYNC_TIMEOUT: Duration = Duration::from_secs(100);

pub fn definitions() -> Value {
    json!([
        {
            "name": "list_hosts",
            "description": "List saved Sageport SSH hosts (id, label, address, port, username, group). Credentials are never returned. Use host ids with run_command.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            },
        },
        {
            "name": "run_command",
            "description": "Run a shell command on one or more saved SSH hosts (non-interactive; separate stdout/stderr; real exit code). Long-running commands return a commandId to poll with get_command_result. Write or dangerous commands may require user approval in the Sageport window.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "hostIds": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "host ids from list_hosts (1 to 100)",
                    },
                    "command": {
                        "type": "string",
                        "description": "shell command to execute on each host",
                    },
                    "timeoutMs": {
                        "type": "integer",
                        "description": "sync wait budget in ms before going async (default 20000, max 100000)",
                    },
                },
                "required": ["hostIds", "command"],
                "additionalProperties": false,
            },
        },
        {
            "name": "get_command_result",
            "description": "Fetch the per-host status and output of a command started by run_command, by commandId.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "commandId": {
                        "type": "string",
                        "description": "command id returned by run_command",
                    },
                },
                "required": ["commandId"],
                "additionalProperties": false,
            },
        },
    ])
}

pub async fn call(app: &AppHandle, client: &str, params: &Value) -> Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or((-32602, "missing tool name".to_string()))?;
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    let outcome = match name {
        "list_hosts" => tool_list_hosts(app).await,
        "run_command" => tool_run_command(app, client, &arguments).await,
        "get_command_result" => tool_get_command_result(app, &arguments).await,
        other => return Err((-32602, format!("unknown tool {other}"))),
    };

    match outcome {
        Ok(structured) => Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&structured).unwrap_or_default(),
            }],
            "structuredContent": structured,
        })),
        Err(error) => Ok(json!({
            "content": [{ "type": "text", "text": error.to_string() }],
            "isError": true,
        })),
    }
}

async fn tool_list_hosts(app: &AppHandle) -> AppResult<Value> {
    let state = app.state::<AppState>();
    let hosts = host_repo::list(&state.db).await?;
    let groups = group_repo::list(&state.db).await?;
    let group_label = |group_id: &Option<String>| {
        group_id
            .as_ref()
            .and_then(|id| groups.iter().find(|group| &group.id == id))
            .map(|group| group.name.clone())
    };
    let items: Vec<Value> = hosts
        .into_iter()
        .filter(|host| host.deleted_at.is_none())
        .map(|host| {
            json!({
                "id": host.id,
                "label": host.label,
                "address": host.address,
                "port": host.port,
                "username": host.username,
                "group": group_label(&host.group_id),
                "os": host.os_hint,
                "jumpHostId": host.jump_host_id,
            })
        })
        .collect();
    super::audit("list_hosts", "", None, None);
    Ok(json!({ "hosts": items }))
}

async fn tool_run_command(app: &AppHandle, client: &str, arguments: &Value) -> AppResult<Value> {
    let host_ids = parse_host_ids(arguments)?;
    let command = arguments
        .get("command")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .ok_or_else(|| AppError::Invalid("command is required".into()))?
        .to_string();
    if command.chars().count() > MAX_COMMAND_CHARS || command.contains('\0') {
        return Err(AppError::Invalid("invalid command".into()));
    }
    let sync_timeout = arguments
        .get("timeoutMs")
        .and_then(Value::as_u64)
        .map(Duration::from_millis)
        .map(|timeout| timeout.min(MAX_SYNC_TIMEOUT))
        .unwrap_or(DEFAULT_SYNC_TIMEOUT);

    let state = app.state::<AppState>();
    let mut targets = Vec::with_capacity(host_ids.len());
    for host_id in &host_ids {
        match host_repo::get(&state.db, host_id).await {
            Ok(host) => targets.push(host),
            Err(error) => {
                return Err(AppError::NotFound(format!("host {host_id}: {error}")));
            }
        }
    }

    let risk = if risk::is_download_pipe(&command) {
        risk::Risk::Dangerous
    } else {
        risk::classify_command(&command)
    };
    let policy = super::load_config(&state.db).await?.policy;
    if policy.requires_approval(risk) {
        let labels = targets.iter().map(|host| host.label.clone()).collect();
        if let Err(error) = super::request_approval(app, client, &command, labels, risk).await {
            let message = error.to_string();
            super::audit("run_command", &command, Some(false), Some(&message));
            return Err(error);
        }
    }

    let command_id = uuid::Uuid::new_v4().to_string();
    let pending: Vec<HostResult> = targets
        .iter()
        .map(|host| HostResult::running(host.id.clone(), host.label.clone()))
        .collect();
    let entry = Arc::new(CommandEntry::new(command.clone(), pending));
    state.mcp.registry.insert(command_id.clone(), entry.clone());

    let limit = Arc::new(Semaphore::new(EXEC_CONCURRENCY));
    for host in targets {
        let app = app.clone();
        let entry = entry.clone();
        let limit = limit.clone();
        let command = command.clone();
        let command_id = command_id.clone();
        tokio::spawn(async move {
            let Ok(_permit) = limit.acquire_owned().await else {
                return;
            };
            let result = execute_on_host(&app, &host, &command, &command_id).await;
            entry.complete(&host.id, result);
        });
    }

    let finished = tokio::time::timeout(sync_timeout, entry.wait_finished())
        .await
        .is_ok();
    let results = entry.snapshot();
    super::audit(
        "run_command",
        &command,
        None,
        (!finished).then_some("still running after the sync wait budget"),
    );
    Ok(json!({
        "commandId": command_id,
        "async": !finished,
        "results": results,
    }))
}

async fn execute_on_host(
    app: &AppHandle,
    host: &crate::domain::Host,
    command: &str,
    command_id: &str,
) -> HostResult {
    let state = app.state::<AppState>();
    let run = async {
        let hops = build_hops(&state, host).await?;
        let session_id = format!("mcp:{command_id}:{}", host.id);
        let conn = establish(app, &state.connection_prompts, &session_id, &hops).await?;
        tokio::time::timeout(
            HARD_COMMAND_TIMEOUT,
            exec_capture_limited(&conn.handle, command, MAX_OUTPUT_BYTES),
        )
        .await
        .map_err(|_| AppError::Timeout("command timed out".into()))?
    };
    match run.await {
        Ok(output) => HostResult::done(
            host.id.clone(),
            host.label.clone(),
            output.stdout,
            output.stderr,
            output.code,
        ),
        Err(error) => HostResult::error(host.id.clone(), host.label.clone(), error.to_string()),
    }
}

async fn tool_get_command_result(app: &AppHandle, arguments: &Value) -> AppResult<Value> {
    let command_id = arguments
        .get("commandId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| AppError::Invalid("commandId is required".into()))?;
    let state = app.state::<AppState>();
    let entry =
        state.mcp.registry.get(command_id).ok_or_else(|| {
            AppError::NotFound(format!("command {command_id} not found or expired"))
        })?;
    Ok(json!({
        "commandId": command_id,
        "command": entry.command,
        "finished": entry.is_finished(),
        "results": entry.snapshot(),
    }))
}

fn parse_host_ids(arguments: &Value) -> AppResult<Vec<String>> {
    let values = arguments
        .get("hostIds")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Invalid("hostIds must be an array".into()))?;
    if values.is_empty() || values.len() > MAX_HOSTS {
        return Err(AppError::Invalid(format!(
            "hostIds needs between 1 and {MAX_HOSTS} entries"
        )));
    }
    let mut unique = std::collections::HashSet::new();
    let mut host_ids = Vec::with_capacity(values.len());
    for value in values {
        let id = value
            .as_str()
            .map(str::trim)
            .filter(|id| !id.is_empty() && id.len() <= 128 && !id.contains('\0'))
            .ok_or_else(|| AppError::Invalid("invalid host id".into()))?;
        if unique.insert(id.to_string()) {
            host_ids.push(id.to_string());
        }
    }
    Ok(host_ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_host_id_lists() {
        let args = json!({"hostIds": [" a ", "a", "b"]});
        assert_eq!(parse_host_ids(&args).unwrap(), ["a", "b"]);
        assert!(parse_host_ids(&json!({"hostIds": []})).is_err());
        assert!(parse_host_ids(&json!({"hostIds": [""]})).is_err());
        assert!(parse_host_ids(&json!({})).is_err());
        let too_many: Vec<String> = (0..=MAX_HOSTS).map(|i| format!("h{i}")).collect();
        assert!(parse_host_ids(&json!({"hostIds": too_many})).is_err());
    }

    #[test]
    fn tool_schemas_are_objects_with_required_fields() {
        let tools = definitions();
        for tool in tools.as_array().unwrap() {
            assert_eq!(tool["inputSchema"]["type"], "object");
            assert!(tool["description"].as_str().unwrap().len() > 10);
        }
        let run = &tools.as_array().unwrap()[1];
        let required = run["inputSchema"]["required"].as_array().unwrap();
        assert!(required.contains(&json!("hostIds")));
        assert!(required.contains(&json!("command")));
    }
}
