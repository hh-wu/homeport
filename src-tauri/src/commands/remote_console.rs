use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::repository::settings_repo;
use crate::state::AppState;

static WATCHDOG: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
fn hide_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0800_0000);
}

#[cfg(not(windows))]
fn hide_console(_command: &mut Command) {}

const KEY_FRPC_PATH: &str = "remote.frpcPath";
const KEY_FRPC_CONFIG: &str = "remote.frpcConfigPath";
const KEY_LOG_PATH: &str = "remote.logPath";
const KEY_RELAY_HOST: &str = "remote.relayHost";
const KEY_SSH_USER: &str = "remote.sshUser";
const KEY_SSH_KEY: &str = "remote.sshKeyPath";
const KEY_DASHBOARD_URL: &str = "remote.dashboardUrl";
const KEY_DASHBOARD_AUTH: &str = "remote.dashboardAuth";
const KEY_RUSTDESK_DOMAIN: &str = "remote.rustdeskDomain";
const KEY_RUSTDESK_KEY: &str = "remote.rustdeskKey";
const KEY_INSTANCE_ID: &str = "remote.instanceId";
const KEY_REGION: &str = "remote.region";
const KEY_PRICE: &str = "remote.pricePerGb";
const KEY_PORT_MAP: &str = "remote.portMap";
const KEY_VISITOR_CONFIG: &str = "remote.visitorConfigPath";
const KEY_ALIYUN_AK: &str = "remote.aliyunAccessKeyId";
const KEY_ALIYUN_SK: &str = "remote.aliyunAccessKeySecret";

#[cfg(test)]
const KEYS: [&str; 17] = [
    KEY_FRPC_PATH,
    KEY_FRPC_CONFIG,
    KEY_LOG_PATH,
    KEY_RELAY_HOST,
    KEY_SSH_USER,
    KEY_SSH_KEY,
    KEY_DASHBOARD_URL,
    KEY_DASHBOARD_AUTH,
    KEY_RUSTDESK_DOMAIN,
    KEY_RUSTDESK_KEY,
    KEY_INSTANCE_ID,
    KEY_REGION,
    KEY_PRICE,
    KEY_PORT_MAP,
    KEY_VISITOR_CONFIG,
    KEY_ALIYUN_AK,
    KEY_ALIYUN_SK,
];

const BSS_HOST: &str = "business.aliyuncs.com";

const DEFAULT_PORT_MAP: &str = "home-rdp = 13389
home-ssh = 10022";

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RcConfig {
    frpc_path: String,
    frpc_config_path: String,
    log_path: String,
    relay_host: String,
    ssh_user: String,
    ssh_key_path: String,
    dashboard_url: String,
    dashboard_auth: String,
    rustdesk_domain: String,
    rustdesk_key: String,
    instance_id: String,
    region: String,
    price_per_gb: f64,
    port_map: String,
    visitor_config_path: String,
    aliyun_access_key_id: String,
    aliyun_access_key_secret: String,
    #[serde(skip_deserializing)]
    aliyun_key_id_set: bool,
    #[serde(skip_deserializing)]
    aliyun_secret_set: bool,
}

impl Default for RcConfig {
    fn default() -> Self {
        Self {
            frpc_path: r"D:\Programs\frp\frpc.exe".into(),
            frpc_config_path: r"D:\Programs\frp\frpc.toml".into(),
            log_path: r"D:\Programs\frp\frpc.log".into(),
            relay_host: "vps.nick0x01.com".into(),
            ssh_user: "root".into(),
            ssh_key_path: r"~\.ssh\id_ed25519".into(),
            dashboard_url: "http://127.0.0.1:7500/api/proxy/stcp".into(),
            dashboard_auth: String::new(),
            rustdesk_domain: "rustdesk.nick0x01.com".into(),
            rustdesk_key: String::new(),
            instance_id: String::new(),
            region: "cn-shanghai".into(),
            price_per_gb: 0.8,
            port_map: DEFAULT_PORT_MAP.into(),
            visitor_config_path: String::new(),
            aliyun_access_key_id: String::new(),
            aliyun_access_key_secret: String::new(),
            aliyun_key_id_set: false,
            aliyun_secret_set: false,
        }
    }
}

impl RcConfig {
    fn entries(&self) -> Vec<(String, String)> {
        vec![
            (KEY_FRPC_PATH.into(), self.frpc_path.clone()),
            (KEY_FRPC_CONFIG.into(), self.frpc_config_path.clone()),
            (KEY_LOG_PATH.into(), self.log_path.clone()),
            (KEY_RELAY_HOST.into(), self.relay_host.clone()),
            (KEY_SSH_USER.into(), self.ssh_user.clone()),
            (KEY_SSH_KEY.into(), self.ssh_key_path.clone()),
            (KEY_DASHBOARD_URL.into(), self.dashboard_url.clone()),
            (KEY_DASHBOARD_AUTH.into(), self.dashboard_auth.clone()),
            (KEY_RUSTDESK_DOMAIN.into(), self.rustdesk_domain.clone()),
            (KEY_RUSTDESK_KEY.into(), self.rustdesk_key.clone()),
            (KEY_INSTANCE_ID.into(), self.instance_id.clone()),
            (KEY_REGION.into(), self.region.clone()),
            (KEY_PRICE.into(), format!("{}", self.price_per_gb)),
            (KEY_PORT_MAP.into(), self.port_map.clone()),
            (
                KEY_VISITOR_CONFIG.into(),
                self.visitor_config_path.clone(),
            ),
            (KEY_ALIYUN_AK.into(), self.aliyun_access_key_id.clone()),
            (KEY_ALIYUN_SK.into(), self.aliyun_access_key_secret.clone()),
        ]
    }
}

fn text_field(value: &str, field: &str, allow_empty: bool) -> AppResult<String> {
    let trimmed = value.trim();
    if !allow_empty && trimmed.is_empty() {
        return Err(AppError::Invalid(format!("{field} is required")));
    }
    if trimmed.chars().count() > 512 || trimmed.chars().any(char::is_control) {
        return Err(AppError::Invalid(format!("{field} is invalid")));
    }
    Ok(trimmed.to_string())
}

fn validate(config: &RcConfig) -> AppResult<RcConfig> {
    let frpc_path = text_field(&config.frpc_path, "frpc path", false)?;
    if !frpc_path.to_ascii_lowercase().ends_with(".exe") {
        return Err(AppError::Invalid("frpc path must point to an exe".into()));
    }
    let frpc_config_path = text_field(&config.frpc_config_path, "frpc config", false)?;
    let log_path = text_field(&config.log_path, "log path", false)?;
    let relay_host = text_field(&config.relay_host, "relay host", false)?;
    let ssh_user = text_field(&config.ssh_user, "ssh user", false)?;
    let ssh_key_path = text_field(&config.ssh_key_path, "ssh key", false)?;
    let dashboard_url = text_field(&config.dashboard_url, "dashboard url", true)?;
    if !dashboard_url.is_empty() && !dashboard_url.starts_with("http") {
        return Err(AppError::Invalid(
            "dashboard url must start with http".into(),
        ));
    }
    let dashboard_auth = text_field(&config.dashboard_auth, "dashboard auth", true)?;
    if !dashboard_auth.is_empty() && !dashboard_auth.contains(':') {
        return Err(AppError::Invalid(
            "dashboard auth must be user:password".into(),
        ));
    }
    let rustdesk_domain = text_field(&config.rustdesk_domain, "RustDesk server", true)?;
    let rustdesk_key = text_field(&config.rustdesk_key, "RustDesk key", true)?;
    let instance_id = text_field(&config.instance_id, "instance id", true)?;
    let region = text_field(&config.region, "region", false)?;
    if !(0.0..=1000.0).contains(&config.price_per_gb) {
        return Err(AppError::Invalid("price per GB is out of range".into()));
    }
    parse_port_map(&config.port_map, true)?;

    Ok(RcConfig {
        frpc_path,
        frpc_config_path,
        log_path,
        relay_host,
        ssh_user,
        ssh_key_path,
        dashboard_url,
        dashboard_auth,
        rustdesk_domain,
        rustdesk_key,
        instance_id,
        region,
        price_per_gb: config.price_per_gb,
        port_map: config.port_map.clone(),
        visitor_config_path: text_field(
            &config.visitor_config_path,
            "visitor config",
            true,
        )?,
        aliyun_access_key_id: text_field(
            &config.aliyun_access_key_id,
            "aliyun access key id",
            true,
        )?,
        aliyun_access_key_secret: text_field(
            &config.aliyun_access_key_secret,
            "aliyun access key secret",
            true,
        )?,
        aliyun_key_id_set: config.aliyun_key_id_set,
        aliyun_secret_set: config.aliyun_secret_set,
    })
}

async fn load_config(db: &SqlitePool) -> AppResult<RcConfig> {
    let fallback = RcConfig::default();
    let mut missing: Vec<(String, String)> = Vec::new();

    macro_rules! field {
        ($key:expr, $default:expr) => {{
            let default: String = $default;
            match settings_repo::get(db, $key).await? {
                Some(value) => value,
                None => {
                    missing.push(($key.to_string(), default.clone()));
                    default
                }
            }
        }};
    }

    let config = RcConfig {
        frpc_path: field!(KEY_FRPC_PATH, fallback.frpc_path.clone()),
        frpc_config_path: field!(KEY_FRPC_CONFIG, fallback.frpc_config_path.clone()),
        log_path: field!(KEY_LOG_PATH, fallback.log_path.clone()),
        relay_host: field!(KEY_RELAY_HOST, fallback.relay_host.clone()),
        ssh_user: field!(KEY_SSH_USER, fallback.ssh_user.clone()),
        ssh_key_path: field!(KEY_SSH_KEY, fallback.ssh_key_path.clone()),
        dashboard_url: field!(KEY_DASHBOARD_URL, fallback.dashboard_url.clone()),
        dashboard_auth: field!(KEY_DASHBOARD_AUTH, fallback.dashboard_auth.clone()),
        rustdesk_domain: field!(KEY_RUSTDESK_DOMAIN, fallback.rustdesk_domain.clone()),
        rustdesk_key: field!(KEY_RUSTDESK_KEY, fallback.rustdesk_key.clone()),
        instance_id: field!(KEY_INSTANCE_ID, fallback.instance_id.clone()),
        region: field!(KEY_REGION, fallback.region.clone()),
        price_per_gb: match settings_repo::get(db, KEY_PRICE).await? {
            Some(value) => value.parse().unwrap_or(fallback.price_per_gb),
            None => {
                missing.push((KEY_PRICE.to_string(), format!("{}", fallback.price_per_gb)));
                fallback.price_per_gb
            }
        },
        port_map: field!(KEY_PORT_MAP, fallback.port_map.clone()),
        visitor_config_path: field!(
            KEY_VISITOR_CONFIG,
            fallback.visitor_config_path.clone()
        ),
        aliyun_access_key_id: field!(KEY_ALIYUN_AK, fallback.aliyun_access_key_id.clone()),
        aliyun_access_key_secret: field!(
            KEY_ALIYUN_SK,
            fallback.aliyun_access_key_secret.clone()
        ),
        aliyun_key_id_set: false,
        aliyun_secret_set: false,
    };

    if !missing.is_empty() {
        settings_repo::set_many(db, &missing).await?;
    }

    Ok(config)
}

async fn save_config(db: &SqlitePool, config: &RcConfig) -> AppResult<RcConfig> {
    let config = validate(config)?;
    settings_repo::set_many(db, &config.entries()).await?;
    Ok(config)
}

fn redacted(config: RcConfig) -> RcConfig {
    RcConfig {
        aliyun_key_id_set: !config.aliyun_access_key_id.is_empty(),
        aliyun_secret_set: !config.aliyun_access_key_secret.is_empty(),
        aliyun_access_key_id: String::new(),
        aliyun_access_key_secret: String::new(),
        ..config
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RcStatus {
    running: bool,
    pids: Vec<u32>,
    uptime: Option<String>,
    last_log: String,
    watchdog: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RcRustDesk {
    domain: String,
    key: String,
    relay: String,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RcCloud {
    instance_status: String,
    instance_spec: String,
    public_ip: String,
    zone: String,
    out_rate: Option<f64>,
    in_rate: Option<f64>,
    out_gb: Option<f64>,
    in_gb: Option<f64>,
    est_fee: Option<f64>,
    balance: Option<String>,
    bill: Option<String>,
    cycle: String,
    errors: Vec<String>,
}

#[cfg(windows)]
fn frpc_pids() -> Vec<u32> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let mut pids = Vec::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return pids;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let end = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                if String::from_utf16_lossy(&entry.szExeFile[..end])
                    .eq_ignore_ascii_case("frpc.exe")
                {
                    pids.push(entry.th32ProcessID);
                }
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
    }
    pids
}

#[cfg(not(windows))]
fn frpc_pids() -> Vec<u32> {
    Vec::new()
}

fn log_lines(path: &str) -> Vec<String> {
    fs::read_to_string(path)
        .map(|text| text.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

fn uptime_text(path: &str) -> Option<String> {
    for line in log_lines(path).iter().rev() {
        if line.contains("start frpc service") {
            let stamp: Vec<&str> = line.split(' ').take(2).collect();
            if stamp.len() == 2 {
                let raw = stamp.join(" ");
                let parsed = chrono::NaiveDateTime::parse_from_str(&raw, "%Y-%m-%d %H:%M:%S%.f")
                    .or_else(|_| chrono::NaiveDateTime::parse_from_str(&raw, "%Y-%m-%d %H:%M:%S"))
                    .ok()?;
                let minutes = (chrono::Local::now().naive_local() - parsed)
                    .num_minutes()
                    .max(0);
                return Some(if minutes < 60 {
                    format!("{minutes} min")
                } else {
                    format!("{} h {} min", minutes / 60, minutes % 60)
                });
            }
        }
    }
    None
}

fn home_dir() -> String {
    std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into())
}

fn expand_home(path: &str) -> String {
    if let Some(rest) = path
        .strip_prefix("~\\")
        .or_else(|| path.strip_prefix("~/"))
    {
        format!("{}\\{}", home_dir(), rest)
    } else {
        path.to_string()
    }
}

fn start_frpc(config: &RcConfig) -> AppResult<()> {
    if !frpc_pids().is_empty() {
        return Ok(());
    }
    if !std::path::Path::new(&config.frpc_path).exists() {
        return Err(AppError::NotFound(config.frpc_path.clone()));
    }
    let dir = std::path::Path::new(&config.frpc_path)
        .parent()
        .map(|path| path.to_path_buf())
        .unwrap_or_default();
    let mut command = Command::new(&config.frpc_path);
    command
        .args(["-c", &config.frpc_config_path])
        .current_dir(dir);
    hide_console(&mut command);
    command.spawn()?;
    Ok(())
}

fn stop_frpc() -> AppResult<()> {
    if frpc_pids().is_empty() {
        return Ok(());
    }
    let mut command = Command::new("taskkill");
    command.args(["/IM", "frpc.exe", "/F"]);
    hide_console(&mut command);
    let status = command.output()?;
    if status.status.success() {
        Ok(())
    } else {
        Err(AppError::Other("frpc did not stop".into()))
    }
}

fn status_snapshot(config: &RcConfig) -> RcStatus {
    let pids = frpc_pids();
    let running = !pids.is_empty();
    let lines = log_lines(&config.log_path);
    RcStatus {
        running,
        pids,
        uptime: if running {
            uptime_text(&config.log_path)
        } else {
            None
        },
        last_log: lines.last().cloned().unwrap_or_default(),
        watchdog: WATCHDOG.load(Ordering::Relaxed),
    }
}

fn percent_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn hmac_sha256_hex(key: &[u8], data: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("hmac key");
    mac.update(data.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn credentials(config: &RcConfig) -> AppResult<(String, String)> {
    let ak = config.aliyun_access_key_id.trim();
    let sk = config.aliyun_access_key_secret.trim();
    if !ak.is_empty() && !sk.is_empty() {
        return Ok((ak.to_string(), sk.to_string()));
    }
    if let (Ok(ak), Ok(sk)) = (
        std::env::var("ALIBABA_CLOUD_ACCESS_KEY_ID"),
        std::env::var("ALIBABA_CLOUD_ACCESS_KEY_SECRET"),
    ) {
        return Ok((ak, sk));
    }
    let path = format!("{}\\.aliyun\\credentials.json", home_dir());
    let text = fs::read_to_string(&path).map_err(|_| AppError::NotFound(path.clone()))?;
    let value: serde_json::Value = serde_json::from_str(&text)?;
    let ak = value["accessKeyId"]
        .as_str()
        .ok_or_else(|| AppError::Invalid("accessKeyId missing".into()))?;
    let sk = value["accessKeySecret"]
        .as_str()
        .ok_or_else(|| AppError::Invalid("accessKeySecret missing".into()))?;
    Ok((ak.to_string(), sk.to_string()))
}

async fn aliyun_call(
    config: &RcConfig,
    host: &str,
    action: &str,
    version: &str,
    params: &[(String, String)],
) -> AppResult<serde_json::Value> {
    let (ak, sk) = credentials(config)?;
    let payload_hash = sha256_hex(b"");

    let mut headers: Vec<(String, String)> = vec![
        ("host".into(), host.into()),
        ("x-acs-action".into(), action.into()),
        ("x-acs-version".into(), version.into()),
        (
            "x-acs-date".into(),
            chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        ),
        (
            "x-acs-signature-nonce".into(),
            uuid::Uuid::new_v4().simple().to_string(),
        ),
        ("x-acs-content-sha256".into(), payload_hash.clone()),
    ];
    headers.sort();

    let mut sorted_params = params.to_vec();
    sorted_params.sort();
    let canonical_query = sorted_params
        .iter()
        .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
        .collect::<Vec<_>>()
        .join("&");

    let canonical_headers: String = headers
        .iter()
        .map(|(key, value)| format!("{key}:{value}\n"))
        .collect();
    let signed_headers = headers
        .iter()
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>()
        .join(";");

    let canonical_request = format!(
        "GET\n/\n{canonical_query}\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
    );
    let string_to_sign = format!(
        "ACS3-HMAC-SHA256\n{}",
        sha256_hex(canonical_request.as_bytes())
    );
    let signature = hmac_sha256_hex(sk.as_bytes(), &string_to_sign);
    let authorization = format!(
        "ACS3-HMAC-SHA256 Credential={ak},SignedHeaders={signed_headers},Signature={signature}"
    );

    let url = if canonical_query.is_empty() {
        format!("https://{host}/")
    } else {
        format!("https://{host}/?{canonical_query}")
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|error| AppError::Network(error.to_string()))?;

    let mut request = client.get(url).header("Authorization", authorization);
    for (key, value) in &headers {
        if key != "host" {
            request = request.header(key, value);
        }
    }

    let response = request
        .send()
        .await
        .map_err(|error| AppError::Network(error.to_string()))?;
    let body = response
        .text()
        .await
        .map_err(|error| AppError::Network(error.to_string()))?;
    serde_json::from_str(&body).map_err(AppError::Serde)
}

fn dimension(instance_id: &str) -> String {
    format!(r#"[{{"instanceId":"{instance_id}"}}]"#)
}

async fn latest_metric(config: &RcConfig, metric: &str) -> Option<f64> {
    let host = format!("metrics.{}.aliyuncs.com", config.region);
    let data = aliyun_call(
        config,
        &host,
        "DescribeMetricLast",
        "2019-01-01",
        &[
            ("Namespace".into(), "acs_ecs_dashboard".into()),
            ("MetricName".into(), metric.into()),
            ("Dimensions".into(), dimension(&config.instance_id)),
            ("Period".into(), "60".into()),
        ],
    )
    .await
    .ok()?;
    let points: serde_json::Value = serde_json::from_str(data["Datapoints"].as_str()?).ok()?;
    let last = points.as_array()?.last()?.clone();
    last["Average"]
        .as_f64()
        .or_else(|| last["Maximum"].as_f64())
        .or_else(|| last["Minimum"].as_f64())
}

async fn month_traffic(config: &RcConfig, metric: &str) -> Option<f64> {
    use chrono::Datelike;
    let now = chrono::Local::now();
    let start = chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), 1)?.and_hms_opt(0, 0, 0)?;
    let format = "%Y-%m-%d %H:%M:%S";
    let host = format!("metrics.{}.aliyuncs.com", config.region);
    let data = aliyun_call(
        config,
        &host,
        "DescribeMetricList",
        "2019-01-01",
        &[
            ("Namespace".into(), "acs_ecs_dashboard".into()),
            ("MetricName".into(), metric.into()),
            ("Dimensions".into(), dimension(&config.instance_id)),
            ("StartTime".into(), start.format(format).to_string()),
            (
                "EndTime".into(),
                now.naive_local().format(format).to_string(),
            ),
            ("Period".into(), "3600".into()),
            ("Length".into(), "2000".into()),
        ],
    )
    .await
    .ok()?;
    let points: serde_json::Value = serde_json::from_str(data["Datapoints"].as_str()?).ok()?;
    let mut bits = 0.0_f64;
    for point in points.as_array()? {
        if let Some(average) = point["Average"].as_f64() {
            bits += average * 3600.0;
        }
    }
    Some(bits / 8.0 / 1024.0 / 1024.0 / 1024.0)
}

async fn cloud_snapshot(config: &RcConfig) -> RcCloud {
    let mut snapshot = RcCloud {
        instance_status: "unknown".into(),
        cycle: chrono::Local::now().format("%Y-%m").to_string(),
        ..Default::default()
    };

    if config.instance_id.is_empty() {
        snapshot
            .errors
            .push("instance id is not configured".into());
        return snapshot;
    }

    let ecs_host = format!("ecs.{}.aliyuncs.com", config.region);
    match aliyun_call(
        config,
        &ecs_host,
        "DescribeInstances",
        "2014-05-26",
        &[
            ("RegionId".into(), config.region.clone()),
            (
                "InstanceIds".into(),
                format!(r#"["{}"]"#, config.instance_id),
            ),
        ],
    )
    .await
    {
        Ok(data) => {
            if let Some(instance) = data["Instances"]["Instance"]
                .as_array()
                .and_then(|items| items.first())
            {
                snapshot.instance_status = match instance["Status"].as_str().unwrap_or("") {
                    "Running" => "running".into(),
                    "Stopped" => "stopped".into(),
                    other => other.to_string(),
                };
                snapshot.instance_spec = instance["InstanceType"].as_str().unwrap_or("").into();
                snapshot.zone = instance["ZoneId"].as_str().unwrap_or("").into();
                if let Some(ip) = instance["PublicIpAddress"]["IpAddress"]
                    .as_array()
                    .and_then(|items| items.first())
                    .and_then(|value| value.as_str())
                {
                    snapshot.public_ip = ip.into();
                }
            }
        }
        Err(error) => snapshot.errors.push(format!("ecs: {error}")),
    }

    match aliyun_call(config, BSS_HOST, "QueryAccountBalance", "2017-12-14", &[]).await {
        Ok(data) => snapshot.balance = data["Data"]["AvailableAmount"].as_str().map(str::to_string),
        Err(error) => snapshot.errors.push(format!("balance: {error}")),
    }

    match aliyun_call(
        config,
        BSS_HOST,
        "DescribeInstanceBill",
        "2017-12-14",
        &[
            ("BillingCycle".into(), snapshot.cycle.clone()),
            ("InstanceID".into(), config.instance_id.clone()),
            ("Granularity".into(), "MONTHLY".into()),
        ],
    )
    .await
    {
        Ok(data) => {
            let items = data["Data"]["Items"].as_array().cloned().unwrap_or_default();
            let total: f64 = items
                .iter()
                .filter_map(|item| {
                    item["PretaxGrossAmount"]
                        .as_f64()
                        .or_else(|| item["PretaxGrossAmount"].as_str()?.parse().ok())
                })
                .sum();
            snapshot.bill = Some(format!("{total:.4}"));
        }
        Err(error) => snapshot.errors.push(format!("bill: {error}")),
    }

    snapshot.out_rate = latest_metric(config, "VPC_PublicIP_InternetOutRate").await;
    snapshot.in_rate = latest_metric(config, "VPC_PublicIP_InternetInRate").await;
    snapshot.out_gb = month_traffic(config, "VPC_PublicIP_InternetOutRate").await;
    snapshot.in_gb = month_traffic(config, "VPC_PublicIP_InternetInRate").await;
    snapshot.est_fee = snapshot.out_gb.map(|gb| gb * config.price_per_gb);

    snapshot
}

fn provider_local_ports(config_path: &str) -> HashMap<String, (String, u16)> {
    fs::read_to_string(expand_home(config_path))
        .map(|text| parse_provider_config(&text))
        .unwrap_or_default()
}

fn parse_provider_config(text: &str) -> HashMap<String, (String, u16)> {
    let mut ports = HashMap::new();
    let Ok(value) = text.parse::<toml::Value>() else {
        return ports;
    };
    let Some(proxies) = value.get("proxies").and_then(|item| item.as_array()) else {
        return ports;
    };
    for proxy in proxies {
        let Some(name) = proxy.get("name").and_then(|item| item.as_str()) else {
            continue;
        };
        let Some(port) = proxy.get("localPort").and_then(|item| item.as_integer()) else {
            continue;
        };
        if !(1..=65535).contains(&port) {
            continue;
        }
        let ip = proxy
            .get("localIP")
            .and_then(|item| item.as_str())
            .unwrap_or("127.0.0.1");
        ports.insert(name.to_string(), (ip.to_string(), port as u16));
    }
    ports
}

fn visitor_config_path(config: &RcConfig) -> &str {
    if config.visitor_config_path.trim().is_empty() {
        &config.frpc_config_path
    } else {
        &config.visitor_config_path
    }
}

fn visitor_access_ports(config_path: &str) -> HashMap<String, u16> {
    fs::read_to_string(expand_home(config_path))
        .map(|text| parse_visitor_config(&text))
        .unwrap_or_default()
}

fn parse_visitor_config(text: &str) -> HashMap<String, u16> {
    let mut ports = HashMap::new();
    let Ok(value) = text.parse::<toml::Value>() else {
        return ports;
    };
    let Some(visitors) = value.get("visitors").and_then(|item| item.as_array()) else {
        return ports;
    };
    for visitor in visitors {
        let Some(server_name) = visitor.get("serverName").and_then(|item| item.as_str()) else {
            continue;
        };
        let Some(port) = visitor.get("bindPort").and_then(|item| item.as_integer()) else {
            continue;
        };
        if !(1..=65535).contains(&port) {
            continue;
        }
        ports.insert(server_name.to_string(), port as u16);
    }
    ports
}

fn parse_port_map(text: &str, strict: bool) -> AppResult<HashMap<String, u16>> {
    let mut ports = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let split = line
            .split_once('=')
            .or_else(|| line.split_once(':'))
            .or_else(|| line.split_once(char::is_whitespace));
        let Some((name, port)) = split else {
            if strict {
                return Err(AppError::Invalid(format!("port map line is invalid: {line}")));
            }
            continue;
        };
        let name = name.trim();
        let port = port.trim();
        if name.is_empty() {
            if strict {
                return Err(AppError::Invalid("port map entry is missing a name".into()));
            }
            continue;
        }
        match port.parse::<u16>() {
            Ok(port) if port > 0 => {
                ports.insert(name.to_string(), port);
            }
            _ => {
                if strict {
                    return Err(AppError::Invalid(format!(
                        "port map entry needs a port between 1 and 65535: {line}"
                    )));
                }
            }
        }
    }
    Ok(ports)
}

fn dashboard_base(config: &RcConfig) -> String {
    match config.dashboard_url.find("/api/") {
        Some(index) => config.dashboard_url[..index].to_string(),
        None => config.dashboard_url.trim_end_matches('/').to_string(),
    }
}

const PROXY_TYPES: [&str; 8] = ["tcp", "udp", "http", "https", "tcpmux", "stcp", "sudp", "xtcp"];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RcProxy {
    name: String,
    kind: String,
    status: String,
    online: bool,
    cur_conns: i64,
    today_traffic_in: i64,
    today_traffic_out: i64,
    last_start_time: String,
    local_ip: String,
    local_port: Option<u16>,
    remote_port: Option<u16>,
    access_port: Option<u16>,
    access_local: bool,
    stale: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RcProxyType {
    kind: String,
    count: i64,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RcServerState {
    version: String,
    bind_port: u16,
    relay_host: String,
    client_counts: i64,
    cur_conns: i64,
    total_traffic_in: i64,
    total_traffic_out: i64,
    proxy_types: Vec<RcProxyType>,
    proxies: Vec<RcProxy>,
    errors: Vec<String>,
}

fn number(value: &serde_json::Value) -> i64 {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        .unwrap_or(0)
}

fn server_state_blocking(config: &RcConfig) -> AppResult<RcServerState> {
    if config.dashboard_url.is_empty() {
        return Err(AppError::Invalid("dashboard url is not configured".into()));
    }
    if config.dashboard_auth.is_empty() {
        return Err(AppError::Invalid(
            "dashboard credentials are not configured".into(),
        ));
    }

    let base = dashboard_base(config);
    let mut script = format!(
        "curl -s -u {} {}/api/serverinfo",
        config.dashboard_auth, base
    );
    for kind in PROXY_TYPES {
        script.push_str(&format!(
            "; echo '@@@'; curl -s -u {} {}/api/proxy/{}",
            config.dashboard_auth, base, kind
        ));
    }

    let mut command = Command::new("ssh");
    command.args([
        "-i",
        &expand_home(&config.ssh_key_path),
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=8",
        "-o",
        "StrictHostKeyChecking=accept-new",
        &format!("{}@{}", config.ssh_user, config.relay_host),
        &script,
    ]);
    hide_console(&mut command);
    let output = command.output()?;
    if !output.status.success() {
        return Err(AppError::Network("ssh failed".into()));
    }

    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let mut chunks = text.split("@@@");
    let mut state = RcServerState {
        relay_host: config.relay_host.clone(),
        ..Default::default()
    };
    let local_ports = provider_local_ports(&config.frpc_config_path);
    let access_ports = parse_port_map(&config.port_map, false)?;
    let visitor_ports = visitor_access_ports(visitor_config_path(config));

    if let Some(section) = chunks.next().and_then(parse_json) {
        state.version = section["version"].as_str().unwrap_or("").into();
        state.bind_port = number(&section["bindPort"]) as u16;
        state.client_counts = number(&section["clientCounts"]);
        state.cur_conns = number(&section["curConns"]);
        state.total_traffic_in = number(&section["totalTrafficIn"]);
        state.total_traffic_out = number(&section["totalTrafficOut"]);
        if let Some(counts) = section["proxyTypeCount"].as_object() {
            state.proxy_types = counts
                .iter()
                .map(|(kind, value)| RcProxyType {
                    kind: kind.clone(),
                    count: number(value),
                })
                .collect();
            state.proxy_types.sort_by(|left, right| left.kind.cmp(&right.kind));
        }
    } else {
        state.errors.push("server info unreadable".into());
    }

    for kind in PROXY_TYPES {
        let Some(section) = chunks.next().and_then(parse_json) else {
            continue;
        };
        let Some(items) = section["proxies"].as_array() else {
            continue;
        };
        for item in items {
            let status = item["status"].as_str().unwrap_or("").to_string();
            let name = item["name"].as_str().unwrap_or("").to_string();
            let local = local_ports.get(&name);
            let access = match access_ports.get(&name) {
                Some(port) => (Some(*port), visitor_ports.contains_key(&name)),
                None => match visitor_ports.get(&name) {
                    Some(port) => (Some(*port), true),
                    None => (None, false),
                },
            };
            state.proxies.push(RcProxy {
                stale: item["conf"].is_null(),
                local_ip: local
                    .map(|(ip, _)| ip.clone())
                    .or_else(|| {
                        item["conf"]["localIP"]
                            .as_str()
                            .map(|value| value.to_string())
                    })
                    .unwrap_or_default(),
                local_port: local.map(|(_, port)| *port),
                remote_port: item["conf"]["remotePort"]
                    .as_i64()
                    .filter(|port| (1..=65535).contains(port))
                    .map(|port| port as u16),
                access_port: access.0,
                access_local: access.1,
                name,
                kind: kind.into(),
                online: status == "online",
                status,
                cur_conns: number(&item["curConns"]),
                today_traffic_in: number(&item["todayTrafficIn"]),
                today_traffic_out: number(&item["todayTrafficOut"]),
                last_start_time: item["lastStartTime"].as_str().unwrap_or("").into(),
            });
        }
    }

    if state.proxies.is_empty() && state.errors.is_empty() {
        state.errors.push("no proxies registered".into());
    }

    Ok(state)
}

fn parse_json(chunk: &str) -> Option<serde_json::Value> {
    let start = chunk.find('{')?;
    let end = chunk.rfind('}')?;
    serde_json::from_str(&chunk[start..=end]).ok()
}

#[tauri::command]
pub async fn rc_server_state(state: tauri::State<'_, AppState>) -> AppResult<RcServerState> {
    let config = load_config(&state.db).await?;
    tauri::async_runtime::spawn_blocking(move || server_state_blocking(&config))
        .await
        .map_err(|error| AppError::Other(error.to_string()))?
}

#[tauri::command]
pub async fn rc_get_config(state: tauri::State<'_, AppState>) -> AppResult<RcConfig> {
    load_config(&state.db).await.map(redacted)
}

#[tauri::command]
pub async fn rc_set_config(
    state: tauri::State<'_, AppState>,
    config: RcConfig,
) -> AppResult<RcConfig> {
    let stored = load_config(&state.db).await?;
    let config = RcConfig {
        aliyun_access_key_id: if config.aliyun_access_key_id.is_empty() {
            stored.aliyun_access_key_id
        } else {
            config.aliyun_access_key_id
        },
        aliyun_access_key_secret: if config.aliyun_access_key_secret.is_empty() {
            stored.aliyun_access_key_secret
        } else {
            config.aliyun_access_key_secret
        },
        ..config
    };
    save_config(&state.db, &config).await.map(redacted)
}

#[tauri::command]
pub async fn rc_status(state: tauri::State<'_, AppState>) -> AppResult<RcStatus> {
    let config = load_config(&state.db).await?;
    Ok(status_snapshot(&config))
}

#[tauri::command]
pub async fn rc_start(state: tauri::State<'_, AppState>) -> AppResult<RcStatus> {
    let config = load_config(&state.db).await?;
    start_frpc(&config)?;
    tokio::time::sleep(std::time::Duration::from_millis(900)).await;
    Ok(status_snapshot(&config))
}

#[tauri::command]
pub async fn rc_stop(state: tauri::State<'_, AppState>) -> AppResult<RcStatus> {
    let config = load_config(&state.db).await?;
    stop_frpc()?;
    Ok(status_snapshot(&config))
}

#[tauri::command]
pub async fn rc_set_watchdog(enabled: bool) -> AppResult<bool> {
    WATCHDOG.store(enabled, Ordering::Relaxed);
    Ok(enabled)
}

#[tauri::command]
pub async fn rc_cloud(state: tauri::State<'_, AppState>) -> AppResult<RcCloud> {
    let config = load_config(&state.db).await?;
    Ok(cloud_snapshot(&config).await)
}

#[tauri::command]
pub async fn rc_log_tail(state: tauri::State<'_, AppState>, lines: usize) -> AppResult<String> {
    let config = load_config(&state.db).await?;
    let all = log_lines(&config.log_path);
    let start = all.len().saturating_sub(lines.clamp(1, 2000));
    Ok(all[start..].join("\n"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RcVerify {
    ok: bool,
    code: String,
    detail: String,
}

fn run_capture(command: &mut Command) -> (bool, String) {
    hide_console(command);
    match command.output() {
        Ok(output) => {
            let mut text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if !stderr.is_empty() {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&stderr);
            }
            (output.status.success(), text)
        }
        Err(error) => (false, error.to_string()),
    }
}

fn verify_result(ok: bool, success: &str, failure: &str, detail: String) -> RcVerify {
    RcVerify {
        ok,
        code: if ok { success } else { failure }.into(),
        detail,
    }
}

fn verify_config_parse(path: &str) -> Option<String> {
    if !std::path::Path::new(&expand_home(path)).exists() {
        return Some(format!("not found: {path}"));
    }
    None
}

fn verify_frpc(config: &RcConfig) -> RcVerify {
    if !std::path::Path::new(&config.frpc_path).exists() {
        return RcVerify {
            ok: false,
            code: "binaryMissing".into(),
            detail: config.frpc_path.clone(),
        };
    }

    let mut details = Vec::new();
    let mut ok = true;
    for (label, path) in [
        ("frpc", config.frpc_config_path.as_str()),
        ("visitor", visitor_config_path(config)),
    ] {
        let path = path.trim();
        if path.is_empty() {
            continue;
        }
        if let Some(message) = verify_config_parse(path) {
            ok = false;
            details.push(format!("{label}: {message}"));
            continue;
        }
        let mut command = Command::new(&config.frpc_path);
        command.args(["verify", "-c", &expand_home(path)]);
        let (passed, detail) = run_capture(&mut command);
        if !passed {
            ok = false;
        }
        details.push(format!("{label}: {detail}"));
    }

    if details.is_empty() {
        return RcVerify {
            ok: false,
            code: "pathsMissing".into(),
            detail: String::new(),
        };
    }

    verify_result(ok, "frpcOk", "frpcFailed", details.join("\n"))
}

fn verify_relay(config: &RcConfig) -> RcVerify {
    let key = expand_home(&config.ssh_key_path);
    if !std::path::Path::new(&key).exists() {
        return RcVerify {
            ok: false,
            code: "keyMissing".into(),
            detail: key,
        };
    }

    let mut command = Command::new("ssh");
    command.args([
        "-i",
        &key,
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=8",
        "-o",
        "StrictHostKeyChecking=accept-new",
        &format!("{}@{}", config.ssh_user, config.relay_host),
        "uname -sr",
    ]);
    let (ok, detail) = run_capture(&mut command);
    verify_result(ok, "relayOk", "relayFailed", detail)
}

fn verify_dashboard(config: &RcConfig) -> RcVerify {
    if config.dashboard_url.trim().is_empty() || config.dashboard_auth.trim().is_empty() {
        return RcVerify {
            ok: false,
            code: "pathsMissing".into(),
            detail: String::new(),
        };
    }

    let mut details = Vec::new();
    match parse_port_map(&config.port_map, true) {
        Ok(ports) => details.push(format!("port map: {} entries", ports.len())),
        Err(error) => {
            return RcVerify {
                ok: false,
                code: "portMapInvalid".into(),
                detail: error.to_string(),
            }
        }
    }

    let script = format!(
        "curl -s -u {} {}/api/serverinfo",
        config.dashboard_auth,
        dashboard_base(config)
    );
    let mut command = Command::new("ssh");
    command.args([
        "-i",
        &expand_home(&config.ssh_key_path),
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=8",
        "-o",
        "StrictHostKeyChecking=accept-new",
        &format!("{}@{}", config.ssh_user, config.relay_host),
        &script,
    ]);
    let (reachable, output) = run_capture(&mut command);
    let version = parse_json(&output)
        .and_then(|value| value["version"].as_str().map(str::to_string));

    match version {
        Some(version) => {
            details.push(format!("frps {version}"));
            verify_result(true, "dashboardOk", "dashboardFailed", details.join("\n"))
        }
        None => {
            details.push(if output.is_empty() {
                "no response".to_string()
            } else {
                output
            });
            RcVerify {
                ok: false,
                code: if reachable {
                    "dashboardFailed".into()
                } else {
                    "relayFailed".into()
                },
                detail: details.join("\n"),
            }
        }
    }
}

fn verify_rustdesk(config: &RcConfig) -> RcVerify {
    use std::net::{TcpStream, ToSocketAddrs};

    let domain = config.rustdesk_domain.trim();
    if domain.is_empty() {
        return RcVerify {
            ok: false,
            code: "pathsMissing".into(),
            detail: String::new(),
        };
    }

    let target = format!("{domain}:21116");
    let address = match target.to_socket_addrs().ok().and_then(|mut items| items.next()) {
        Some(address) => address,
        None => {
            return RcVerify {
                ok: false,
                code: "rustdeskFailed".into(),
                detail: format!("cannot resolve {domain}"),
            }
        }
    };

    match TcpStream::connect_timeout(&address, std::time::Duration::from_secs(8)) {
        Ok(_) => verify_result(
            true,
            "rustdeskOk",
            "rustdeskFailed",
            format!("{domain} → {address}"),
        ),
        Err(error) => RcVerify {
            ok: false,
            code: "rustdeskFailed".into(),
            detail: format!("{address}: {error}"),
        },
    }
}

async fn verify_cloud(config: &RcConfig) -> RcVerify {
    if config.instance_id.trim().is_empty() {
        return RcVerify {
            ok: false,
            code: "pathsMissing".into(),
            detail: String::new(),
        };
    }

    let host = format!("ecs.{}.aliyuncs.com", config.region);
    match aliyun_call(
        config,
        &host,
        "DescribeInstances",
        "2014-05-26",
        &[
            ("RegionId".into(), config.region.clone()),
            (
                "InstanceIds".into(),
                format!(r#"["{}"]"#, config.instance_id),
            ),
        ],
    )
    .await
    {
        Ok(data) => {
            let instance = data["Instances"]["Instance"]
                .as_array()
                .and_then(|items| items.first());
            match instance {
                Some(instance) => verify_result(
                    true,
                    "cloudOk",
                    "cloudFailed",
                    format!(
                        "{} · {} · {}",
                        instance["InstanceId"].as_str().unwrap_or(""),
                        instance["Status"].as_str().unwrap_or(""),
                        instance["InstanceType"].as_str().unwrap_or("")
                    ),
                ),
                None => RcVerify {
                    ok: false,
                    code: "cloudFailed".into(),
                    detail: "instance not found".into(),
                },
            }
        }
        Err(error) => RcVerify {
            ok: false,
            code: "cloudFailed".into(),
            detail: error.to_string(),
        },
    }
}

#[tauri::command]
pub async fn rc_verify(
    state: tauri::State<'_, AppState>,
    section: String,
) -> AppResult<RcVerify> {
    let config = load_config(&state.db).await?;

    let result = match section.as_str() {
        "frpc" => {
            tauri::async_runtime::spawn_blocking(move || verify_frpc(&config))
                .await
                .map_err(|error| AppError::Other(error.to_string()))?
        }
        "relay" => tauri::async_runtime::spawn_blocking(move || verify_relay(&config))
            .await
            .map_err(|error| AppError::Other(error.to_string()))?,
        "dashboard" => {
            tauri::async_runtime::spawn_blocking(move || verify_dashboard(&config))
                .await
                .map_err(|error| AppError::Other(error.to_string()))?
        }
        "rustdesk" => {
            tauri::async_runtime::spawn_blocking(move || verify_rustdesk(&config))
                .await
                .map_err(|error| AppError::Other(error.to_string()))?
        }
        "cloud" => verify_cloud(&config).await,
        other => {
            return Err(AppError::Invalid(format!(
                "unknown section for verification: {other}"
            )))
        }
    };

    Ok(result)
}

#[tauri::command]
pub async fn rc_open_rdp(port: u16) -> AppResult<()> {
    if port == 0 {
        return Err(AppError::Invalid("port is required".into()));
    }
    let mut command = Command::new("mstsc");
    command.arg(format!("/v:127.0.0.1:{port}"));
    hide_console(&mut command);
    command.spawn()?;
    Ok(())
}

#[tauri::command]
pub async fn rc_reveal_log(state: tauri::State<'_, AppState>) -> AppResult<()> {
    let config = load_config(&state.db).await?;
    if !std::path::Path::new(&config.log_path).exists() {
        return Err(AppError::NotFound(config.log_path));
    }
    let mut command = Command::new("explorer");
    command.arg(format!("/select,{}", config.log_path));
    hide_console(&mut command);
    command.spawn()?;
    Ok(())
}

#[tauri::command]
pub async fn rc_open_log(state: tauri::State<'_, AppState>) -> AppResult<()> {
    let config = load_config(&state.db).await?;
    if !std::path::Path::new(&config.log_path).exists() {
        return Err(AppError::NotFound(config.log_path));
    }
    let mut command = Command::new("notepad.exe");
    command.arg(&config.log_path);
    hide_console(&mut command);
    command.spawn()?;
    Ok(())
}

#[tauri::command]
pub async fn rc_rustdesk_info(state: tauri::State<'_, AppState>) -> AppResult<RcRustDesk> {
    let config = load_config(&state.db).await?;
    Ok(RcRustDesk {
        domain: config.rustdesk_domain,
        key: config.rustdesk_key,
        relay: format!("{}:7000", config.relay_host),
    })
}

pub async fn ensure_config(db: &SqlitePool) {
    let _ = load_config(db).await;
}

pub fn spawn_watchdog(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            if !WATCHDOG.load(Ordering::Relaxed) || !frpc_pids().is_empty() {
                continue;
            }
            let state = app.state::<AppState>();
            if let Ok(config) = load_config(&state.db).await {
                let _ = start_frpc(&config);
            }
        }
    });
}

pub async fn start_from_tray(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Ok(config) = load_config(&state.db).await {
        let _ = start_frpc(&config);
    }
}

pub async fn stop_from_tray() {
    let _ = stop_frpc();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliyun_config_credentials_take_precedence() {
        let config = RcConfig {
            aliyun_access_key_id: " cfg-ak ".into(),
            aliyun_access_key_secret: " cfg-sk ".into(),
            ..RcConfig::default()
        };
        let (ak, sk) = credentials(&config).unwrap();
        assert_eq!(ak, "cfg-ak");
        assert_eq!(sk, "cfg-sk");
    }

    #[test]
    fn redaction_hides_the_aliyun_secret() {
        let config = RcConfig {
            aliyun_access_key_id: "ak".into(),
            aliyun_access_key_secret: "sk".into(),
            ..RcConfig::default()
        };
        let view = redacted(config);
        assert!(view.aliyun_key_id_set);
        assert!(view.aliyun_secret_set);
        assert!(view.aliyun_access_key_id.is_empty());
        assert!(view.aliyun_access_key_secret.is_empty());

        let empty = redacted(RcConfig::default());
        assert!(!empty.aliyun_key_id_set);
        assert!(!empty.aliyun_secret_set);
    }

    #[test]
    fn rejects_invalid_configuration() {
        let mut config = RcConfig {
            frpc_path: "not-a-path".into(),
            ..RcConfig::default()
        };
        assert!(validate(&config).is_err());

        config = RcConfig {
            relay_host: "  ".into(),
            ..RcConfig::default()
        };
        assert!(validate(&config).is_err());

        config = RcConfig {
            dashboard_auth: "admin".into(),
            ..RcConfig::default()
        };
        assert!(validate(&config).is_err());

        config = RcConfig {
            price_per_gb: -1.0,
            ..RcConfig::default()
        };
        assert!(validate(&config).is_err());
    }

    #[test]
    fn trims_and_accepts_a_valid_configuration() {
        let config = RcConfig {
            relay_host: " vps.example.com ".into(),
            instance_id: " i-123 ".into(),
            ..RcConfig::default()
        };
        let validated = validate(&config).unwrap();
        assert_eq!(validated.relay_host, "vps.example.com");
        assert_eq!(validated.instance_id, "i-123");
    }

    #[test]
    fn every_key_is_written_once() {
        let entries = RcConfig::default().entries();
        assert_eq!(entries.len(), KEYS.len());
        for key in KEYS {
            assert!(entries.iter().any(|(entry, _)| entry == key), "{key}");
        }
    }

    #[test]
    fn parses_port_maps_in_several_shapes() {
        let ports = parse_port_map(
            "home-rdp = 13389\nhome-ssh:10022\n\n# comment\nnas 22990\n",
            true,
        )
        .unwrap();
        assert_eq!(ports.get("home-rdp"), Some(&13389));
        assert_eq!(ports.get("home-ssh"), Some(&10022));
        assert_eq!(ports.get("nas"), Some(&22990));
        assert!(!ports.contains_key("# comment"));
    }

    #[test]
    fn rejects_invalid_port_map_lines() {
        assert!(parse_port_map("home-rdp = 99999", true).is_err());
        assert!(parse_port_map("home-rdp", true).is_err());
        assert!(parse_port_map("= 13389", true).is_err());
        assert_eq!(parse_port_map("home-rdp = 99999", false).unwrap().len(), 0);
    }

    #[test]
    fn reads_access_ports_by_server_name_from_the_visitor_config() {
        let ports = parse_visitor_config(
            "serverAddr = \"vps.example.com\"\n\n[[visitors]]\nname = \"s4090-ssh\"\ntype = \"stcp\"\nserverName = \"server-4090-ssh\"\nbindPort = 20022\n\n[[visitors]]\nname = \"s4090-rdp\"\ntype = \"stcp\"\nserverName = \"server-4090-rdp\"\nbindAddr = \"127.0.0.1\"\nbindPort = 23389\n",
        );
        assert_eq!(ports.get("server-4090-ssh"), Some(&20022));
        assert_eq!(ports.get("server-4090-rdp"), Some(&23389));
        assert_eq!(ports.get("s4090-ssh"), None);

        assert!(parse_visitor_config("serverAddr = \"x\"").is_empty());
        assert!(visitor_access_ports("does-not-exist.toml").is_empty());
    }

    #[test]
    fn reads_local_ports_from_the_provider_config() {
        let ports = parse_provider_config(
            "serverAddr = \"vps.example.com\"\n\n[[proxies]]\nname = \"home-rdp\"\ntype = \"stcp\"\nlocalIP = \"127.0.0.1\"\nlocalPort = 3389\n\n[[proxies]]\nname = \"home-ssh\"\ntype = \"stcp\"\nlocalPort = 22\n",
        );
        assert_eq!(ports.get("home-rdp"), Some(&("127.0.0.1".to_string(), 3389)));
        assert_eq!(ports.get("home-ssh"), Some(&("127.0.0.1".to_string(), 22)));

        assert!(parse_provider_config("not toml at all {{{").is_empty());
        assert!(parse_provider_config("serverAddr = \"x\"").is_empty());
        assert!(provider_local_ports("does-not-exist.toml").is_empty());
    }

    #[test]
    fn expands_home_prefixed_paths() {
        let expanded = expand_home(r"~\.ssh\id_ed25519");
        assert!(expanded.ends_with(r".ssh\id_ed25519"));
        assert!(!expanded.starts_with('~'));
        assert_eq!(expand_home(r"D:\keys\id"), r"D:\keys\id");
    }

    #[test]
    fn derives_the_dashboard_base_from_the_proxy_endpoint() {
        let mut config = RcConfig::default();
        config.dashboard_url = "http://127.0.0.1:7500/api/proxy/stcp".into();
        assert_eq!(dashboard_base(&config), "http://127.0.0.1:7500");

        config.dashboard_url = "https://example.com:8443/dashboard".into();
        assert_eq!(dashboard_base(&config), "https://example.com:8443/dashboard");
    }

    #[test]
    fn parses_json_chunks_with_surrounding_noise() {        let parsed = parse_json("warning\n{\"proxies\":[]}\n").unwrap();
        assert!(parsed["proxies"].as_array().is_some());
        assert!(parse_json("no json here").is_none());
        assert!(parse_json("").is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn aliyun_signing_reaches_the_account_endpoint() {
        let data = aliyun_call(&RcConfig::default(), BSS_HOST, "QueryAccountBalance", "2017-12-14", &[])
            .await
            .expect("account balance");
        assert!(data["Data"]["AvailableAmount"].is_string());
    }
}
