use std::convert::Infallible;
use std::net::Ipv4Addr;

use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde_json::Value;
use tauri::{AppHandle, Manager};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use super::protocol;
use super::McpConfig;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

const MAX_BODY_BYTES: usize = 1024 * 1024;
const MCP_PATH: &str = "/mcp";

pub struct ServerRuntime {
    pub port: u16,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl ServerRuntime {
    pub async fn stop(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
}

pub async fn sync(app: &AppHandle, config: &McpConfig) -> AppResult<()> {
    let existing = app.state::<AppState>().mcp.runtime.lock().take();
    if let Some(runtime) = existing {
        if config.enabled && runtime.port == config.port {
            *app.state::<AppState>().mcp.runtime.lock() = Some(runtime);
            return Ok(());
        }
        runtime.stop().await;
    }
    if config.enabled {
        let runtime = start(app.clone(), config.port).await?;
        *app.state::<AppState>().mcp.runtime.lock() = Some(runtime);
    }
    Ok(())
}

pub fn listening_port(app: &AppHandle) -> Option<u16> {
    app.state::<AppState>()
        .mcp
        .runtime
        .lock()
        .as_ref()
        .map(|runtime| runtime.port)
}

async fn start(app: AppHandle, port: u16) -> AppResult<ServerRuntime> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .await
        .map_err(|error| AppError::Other(format!("mcp listen on 127.0.0.1:{port}: {error}")))?;
    let actual_port = listener.local_addr()?.port();
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => break,
                accepted = listener.accept() => {
                    match accepted {
                        Ok((stream, _)) => {
                            tokio::spawn(serve_connection(app.clone(), stream));
                        }
                        Err(_) => {
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                    }
                }
            }
        }
    });
    Ok(ServerRuntime {
        port: actual_port,
        shutdown: Some(shutdown_tx),
        task: Some(task),
    })
}

async fn serve_connection(app: AppHandle, stream: TcpStream) {
    let io = TokioIo::new(stream);
    let service = service_fn(move |request| handle_request(app.clone(), request));
    let _ = http1::Builder::new().serve_connection(io, service).await;
}

async fn handle_request(
    app: AppHandle,
    request: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    Ok(match route(app, request).await {
        Ok(response) => response,
        Err(error) => json_response(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    })
}

async fn route(app: AppHandle, request: Request<Incoming>) -> AppResult<Response<Full<Bytes>>> {
    let path = request.uri().path().to_string();
    let method = request.method().clone();
    if path != MCP_PATH {
        return Ok(json_response(StatusCode::NOT_FOUND, "not found".into()));
    }
    if method == hyper::Method::GET {
        return Ok(Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .header("allow", "POST")
            .body(Full::new(Bytes::new()))
            .expect("static response"));
    }
    if method != hyper::Method::POST {
        return Ok(json_response(
            StatusCode::METHOD_NOT_ALLOWED,
            "method not allowed".into(),
        ));
    }

    let token = bearer_token(request.headers());
    let client = match resolve_client(&app, &token).await {
        Some(name) => name,
        None => {
            return Ok(Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header("www-authenticate", "Bearer realm=\"sageport\"")
                .body(Full::new(Bytes::from("invalid or missing token")))
                .expect("static response"));
        }
    };

    let body = match Limited::new(request.into_body(), MAX_BODY_BYTES)
        .collect()
        .await
    {
        Ok(collected) => collected.to_bytes(),
        Err(_) => {
            return Ok(json_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "request body is too large".into(),
            ));
        }
    };

    let parsed: Result<Value, _> = serde_json::from_slice(&body);
    let message = match parsed {
        Ok(value) => value,
        Err(_) => {
            return Ok(json_response(
                StatusCode::BAD_REQUEST,
                protocol::error_value(Value::Null, -32700, "parse error").to_string(),
            ));
        }
    };

    match message {
        Value::Array(batch) => {
            let mut replies = Vec::new();
            for item in batch {
                if let Some(reply) = protocol::handle_message(Some(&app), &client, item).await {
                    replies.push(reply);
                }
            }
            if replies.is_empty() {
                Ok(Response::builder()
                    .status(StatusCode::ACCEPTED)
                    .body(Full::new(Bytes::new()))
                    .expect("static response"))
            } else {
                Ok(json_response(
                    StatusCode::OK,
                    Value::Array(replies).to_string(),
                ))
            }
        }
        single => match protocol::handle_message(Some(&app), &client, single).await {
            Some(reply) => Ok(json_response(StatusCode::OK, reply.to_string())),
            None => Ok(Response::builder()
                .status(StatusCode::ACCEPTED)
                .body(Full::new(Bytes::new()))
                .expect("static response")),
        },
    }
}

async fn resolve_client(app: &AppHandle, token: &str) -> Option<String> {
    let db = app.state::<AppState>().db.clone();
    super::resolve_token(&db, token).await.ok().flatten()
}

fn bearer_token(headers: &hyper::HeaderMap) -> String {
    let Some(value) = headers.get("authorization") else {
        return String::new();
    };
    let Ok(text) = value.to_str() else {
        return String::new();
    };
    const PREFIX: &str = "bearer ";
    if text.len() > PREFIX.len() && text[..PREFIX.len()].eq_ignore_ascii_case(PREFIX) {
        text[PREFIX.len()..].trim().to_string()
    } else {
        String::new()
    }
}

fn json_response(status: StatusCode, body: String) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(body)))
        .expect("static response")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers_with_auth(value: Option<&str>) -> hyper::HeaderMap {
        let mut headers = hyper::HeaderMap::new();
        if let Some(value) = value {
            headers.insert("authorization", value.parse().unwrap());
        }
        headers
    }

    #[test]
    fn extracts_bearer_token_case_insensitively() {
        assert_eq!(
            bearer_token(&headers_with_auth(Some("Bearer abc123"))),
            "abc123"
        );
        assert_eq!(
            bearer_token(&headers_with_auth(Some("bearer abc123"))),
            "abc123"
        );
        assert!(bearer_token(&headers_with_auth(Some("Basic abc"))).is_empty());
        assert!(bearer_token(&headers_with_auth(None)).is_empty());
    }
}
