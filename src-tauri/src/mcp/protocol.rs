use serde_json::{json, Value};
use tauri::AppHandle;

use super::tools;

const SUPPORTED_VERSIONS: [&str; 2] = ["2025-06-18", "2025-03-26"];

pub fn error_value(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}

fn result_value(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

pub async fn handle_message(
    app: Option<&AppHandle>,
    client: &str,
    message: Value,
) -> Option<Value> {
    let object = message.as_object()?;
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        let id = object.get("id").cloned().unwrap_or(Value::Null);
        return Some(error_value(id, -32600, "invalid request"));
    }
    let id = object.get("id").cloned();
    let Some(method) = object.get("method").and_then(Value::as_str) else {
        return id.map(|id| error_value(id, -32600, "missing method"));
    };
    let params = object.get("params").cloned().unwrap_or(Value::Null);

    match (method, id) {
        ("initialize", Some(id)) => {
            let requested = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let version = if SUPPORTED_VERSIONS.contains(&requested) {
                requested
            } else {
                SUPPORTED_VERSIONS[0]
            };
            Some(result_value(
                id,
                json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": {
                        "name": "sageport",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                    "instructions": "Use list_hosts to discover saved SSH hosts, run_command to execute a shell command on one or more hosts, and get_command_result to poll long-running commands. Write or dangerous commands may require the user to approve in the Sageport window.",
                }),
            ))
        }
        ("ping", Some(id)) => Some(result_value(id, json!({}))),
        ("tools/list", Some(id)) => {
            Some(result_value(id, json!({ "tools": tools::definitions() })))
        }
        ("tools/call", Some(id)) => match app {
            Some(app) => match tools::call(app, client, &params).await {
                Ok(result) => Some(result_value(id, result)),
                Err((code, message)) => Some(error_value(id, code, &message)),
            },
            None => Some(error_value(id, -32603, "server is not ready")),
        },
        (method, Some(id)) if method.starts_with("notifications/") => Some(error_value(
            id,
            -32601,
            "notifications do not produce responses",
        )),
        (method, Some(id)) => Some(error_value(id, -32601, &format!("unknown method {method}"))),
        (_, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn initialize_negotiates_protocol_version() {
        let reply = handle_message(
            None,
            "test",
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-03-26"}}),
        )
        .await
        .unwrap();
        assert_eq!(reply["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(reply["result"]["serverInfo"]["name"], "sageport");

        let fallback = handle_message(
            None,
            "test",
            json!({"jsonrpc": "2.0", "id": 2, "method": "initialize", "params": {"protocolVersion": "1999-01-01"}}),
        )
        .await
        .unwrap();
        assert_eq!(fallback["result"]["protocolVersion"], SUPPORTED_VERSIONS[0]);
    }

    #[tokio::test]
    async fn notifications_produce_no_response() {
        let reply = handle_message(
            None,
            "test",
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        )
        .await;
        assert!(reply.is_none());
    }

    #[tokio::test]
    async fn unknown_methods_are_method_not_found() {
        let reply = handle_message(
            None,
            "test",
            json!({"jsonrpc": "2.0", "id": 9, "method": "resources/list"}),
        )
        .await
        .unwrap();
        assert_eq!(reply["error"]["code"], -32601);
    }

    #[tokio::test]
    async fn ping_answers_with_empty_result() {
        let reply = handle_message(
            None,
            "test",
            json!({"jsonrpc": "2.0", "id": 3, "method": "ping"}),
        )
        .await
        .unwrap();
        assert_eq!(reply["result"], json!({}));
    }

    #[tokio::test]
    async fn tools_list_exposes_the_tool_set() {
        let reply = handle_message(
            None,
            "test",
            json!({"jsonrpc": "2.0", "id": 4, "method": "tools/list"}),
        )
        .await
        .unwrap();
        let names: Vec<&str> = reply["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["list_hosts", "run_command", "get_command_result"]);
    }
}
