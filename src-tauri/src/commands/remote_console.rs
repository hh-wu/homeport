use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use hmac::{Hmac, KeyInit, Mac};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};

const FRP_DIR: &str = r"D:\Programs\frp";
const FRPC_EXE: &str = r"D:\Programs\frp\frpc.exe";
const FRPC_CFG: &str = r"D:\Programs\frp\frpc.toml";
const FRPC_LOG: &str = r"D:\Programs\frp\frpc.log";

const VPS_DOMAIN: &str = "vps.nick0x01.com";
const RUSTDESK_DOMAIN: &str = "rustdesk.nick0x01.com";
const RUSTDESK_KEY: &str = "redacted=";

const ECS_HOST: &str = "ecs.cn-shanghai.aliyuncs.com";
const BSS_HOST: &str = "business.aliyuncs.com";
const METRICS_HOST: &str = "metrics.cn-shanghai.aliyuncs.com";
const REGION_ID: &str = "cn-shanghai";
const INSTANCE_ID: &str = "redacted";
const DASH_AUTH: &str = "admin:redacted";
const PRICE_PER_GB: f64 = 0.8;

static WATCHDOG: AtomicBool = AtomicBool::new(false);

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
    vps: String,
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

fn log_lines() -> Vec<String> {
    fs::read_to_string(FRPC_LOG)
        .map(|text| text.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

fn uptime_text() -> Option<String> {
    for line in log_lines().iter().rev() {
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

pub fn start_frpc() -> AppResult<()> {
    if !frpc_pids().is_empty() {
        return Ok(());
    }
    if !std::path::Path::new(FRPC_EXE).exists() {
        return Err(AppError::NotFound(FRPC_EXE.into()));
    }
    Command::new(FRPC_EXE)
        .args(["-c", FRPC_CFG])
        .current_dir(FRP_DIR)
        .spawn()?;
    Ok(())
}

pub fn stop_frpc() -> AppResult<()> {
    if frpc_pids().is_empty() {
        return Ok(());
    }
    let status = Command::new("taskkill")
        .args(["/IM", "frpc.exe", "/F"])
        .output()?;
    if status.status.success() {
        Ok(())
    } else {
        Err(AppError::Other("frpc did not stop".into()))
    }
}

fn status_snapshot() -> RcStatus {
    let pids = frpc_pids();
    let running = !pids.is_empty();
    let lines = log_lines();
    RcStatus {
        running,
        pids,
        uptime: if running { uptime_text() } else { None },
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

fn home_dir() -> String {
    std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into())
}

fn credentials() -> AppResult<(String, String)> {
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
    host: &str,
    action: &str,
    version: &str,
    params: &[(String, String)],
) -> AppResult<serde_json::Value> {
    let (ak, sk) = credentials()?;
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

async fn latest_metric(metric: &str) -> Option<f64> {
    let data = aliyun_call(
        METRICS_HOST,
        "DescribeMetricLast",
        "2019-01-01",
        &[
            ("Namespace".into(), "acs_ecs_dashboard".into()),
            ("MetricName".into(), metric.into()),
            (
                "Dimensions".into(),
                format!(r#"[{{"instanceId":"{INSTANCE_ID}"}}]"#),
            ),
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

async fn month_traffic(metric: &str) -> Option<f64> {
    use chrono::Datelike;
    let now = chrono::Local::now();
    let start = chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), 1)?.and_hms_opt(0, 0, 0)?;
    let format = "%Y-%m-%d %H:%M:%S";
    let data = aliyun_call(
        METRICS_HOST,
        "DescribeMetricList",
        "2019-01-01",
        &[
            ("Namespace".into(), "acs_ecs_dashboard".into()),
            ("MetricName".into(), metric.into()),
            (
                "Dimensions".into(),
                format!(r#"[{{"instanceId":"{INSTANCE_ID}"}}]"#),
            ),
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

async fn cloud_snapshot() -> RcCloud {
    let mut snapshot = RcCloud {
        instance_status: "unknown".into(),
        cycle: chrono::Local::now().format("%Y-%m").to_string(),
        ..Default::default()
    };

    match aliyun_call(
        ECS_HOST,
        "DescribeInstances",
        "2014-05-26",
        &[
            ("RegionId".into(), REGION_ID.into()),
            ("InstanceIds".into(), format!(r#"["{INSTANCE_ID}"]"#)),
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

    match aliyun_call(BSS_HOST, "QueryAccountBalance", "2017-12-14", &[]).await {
        Ok(data) => snapshot.balance = data["Data"]["AvailableAmount"].as_str().map(str::to_string),
        Err(error) => snapshot.errors.push(format!("balance: {error}")),
    }

    match aliyun_call(
        BSS_HOST,
        "DescribeInstanceBill",
        "2017-12-14",
        &[
            ("BillingCycle".into(), snapshot.cycle.clone()),
            ("InstanceID".into(), INSTANCE_ID.into()),
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

    snapshot.out_rate = latest_metric("VPC_PublicIP_InternetOutRate").await;
    snapshot.in_rate = latest_metric("VPC_PublicIP_InternetInRate").await;
    snapshot.out_gb = month_traffic("VPC_PublicIP_InternetOutRate").await;
    snapshot.in_gb = month_traffic("VPC_PublicIP_InternetInRate").await;
    snapshot.est_fee = snapshot.out_gb.map(|gb| gb * PRICE_PER_GB);

    snapshot
}

fn check_proxies_blocking() -> AppResult<String> {
    let key = format!("{}\\.ssh\\id_ed25519", home_dir());
    let remote = format!("curl -s -u {DASH_AUTH} http://127.0.0.1:7500/api/proxy/stcp");
    let output = Command::new("ssh")
        .args([
            "-i",
            &key,
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=8",
            "-o",
            "StrictHostKeyChecking=accept-new",
            &format!("root@{VPS_DOMAIN}"),
            &remote,
        ])
        .output()?;
    if !output.status.success() {
        return Err(AppError::Network("ssh failed".into()));
    }
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let start = text
        .find('{')
        .ok_or_else(|| AppError::Other("dashboard response unreadable".into()))?;
    let end = text
        .rfind('}')
        .ok_or_else(|| AppError::Other("dashboard response unreadable".into()))?;
    let data: serde_json::Value = serde_json::from_str(&text[start..=end])?;
    let proxies = data["proxies"].as_array().cloned().unwrap_or_default();
    if proxies.is_empty() {
        return Err(AppError::Other("no proxies registered".into()));
    }
    let labels: Vec<String> = proxies
        .iter()
        .map(|proxy| {
            let name = proxy["name"].as_str().unwrap_or("?");
            let status = proxy["status"].as_str().unwrap_or("?");
            format!("{name}:{status}")
        })
        .collect();
    if proxies.iter().all(|proxy| proxy["status"] == "online") {
        Ok(labels.join(" / "))
    } else {
        Err(AppError::Other(labels.join(" / ")))
    }
}

#[tauri::command]
pub async fn rc_status() -> AppResult<RcStatus> {
    Ok(status_snapshot())
}

#[tauri::command]
pub async fn rc_start() -> AppResult<RcStatus> {
    start_frpc()?;
    tokio::time::sleep(std::time::Duration::from_millis(900)).await;
    Ok(status_snapshot())
}

#[tauri::command]
pub async fn rc_stop() -> AppResult<RcStatus> {
    stop_frpc()?;
    Ok(status_snapshot())
}

#[tauri::command]
pub async fn rc_set_watchdog(enabled: bool) -> AppResult<bool> {
    WATCHDOG.store(enabled, Ordering::Relaxed);
    Ok(enabled)
}

#[tauri::command]
pub async fn rc_check_proxies() -> AppResult<String> {
    tauri::async_runtime::spawn_blocking(check_proxies_blocking)
        .await
        .map_err(|error| AppError::Other(error.to_string()))?
}

#[tauri::command]
pub async fn rc_cloud() -> AppResult<RcCloud> {
    Ok(cloud_snapshot().await)
}

#[tauri::command]
pub async fn rc_log_tail(lines: usize) -> AppResult<String> {
    let all = log_lines();
    let start = all.len().saturating_sub(lines.clamp(1, 2000));
    Ok(all[start..].join("\n"))
}

#[tauri::command]
pub async fn rc_open_log() -> AppResult<()> {
    if !std::path::Path::new(FRPC_LOG).exists() {
        return Err(AppError::NotFound(FRPC_LOG.into()));
    }
    Command::new("cmd")
        .args(["/C", "start", "", FRPC_LOG])
        .spawn()?;
    Ok(())
}

#[tauri::command]
pub async fn rc_rustdesk_info() -> AppResult<RcRustDesk> {
    Ok(RcRustDesk {
        domain: RUSTDESK_DOMAIN.into(),
        key: RUSTDESK_KEY.into(),
        vps: format!("{VPS_DOMAIN}:7000"),
    })
}

pub fn spawn_watchdog() {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            if WATCHDOG.load(Ordering::Relaxed) && frpc_pids().is_empty() {
                let _ = start_frpc();
            }
        }
    });
}
