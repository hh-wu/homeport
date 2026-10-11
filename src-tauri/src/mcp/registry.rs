use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::Serialize;
use tokio::sync::Notify;

const MAX_RECORDS: usize = 100;
const RECORD_TTL: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostResult {
    pub host_id: String,
    pub host_label: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl HostResult {
    pub fn running(host_id: String, host_label: String) -> Self {
        Self {
            host_id,
            host_label,
            status: "running".into(),
            stdout: None,
            stderr: None,
            exit_code: None,
            message: None,
        }
    }

    pub fn done(
        host_id: String,
        host_label: String,
        stdout: String,
        stderr: String,
        code: i32,
    ) -> Self {
        Self {
            host_id,
            host_label,
            status: "done".into(),
            stdout: Some(stdout),
            stderr: Some(stderr),
            exit_code: Some(code),
            message: None,
        }
    }

    pub fn error(host_id: String, host_label: String, message: String) -> Self {
        Self {
            host_id,
            host_label,
            status: "error".into(),
            stdout: None,
            stderr: None,
            exit_code: None,
            message: Some(message),
        }
    }
}

pub struct CommandEntry {
    pub command: String,
    results: Mutex<Vec<HostResult>>,
    notify: Notify,
}

impl CommandEntry {
    pub fn new(command: String, pending: Vec<HostResult>) -> Self {
        Self {
            command,
            results: Mutex::new(pending),
            notify: Notify::new(),
        }
    }

    pub fn complete(&self, host_id: &str, result: HostResult) {
        {
            let mut results = self.results.lock();
            if let Some(slot) = results.iter_mut().find(|item| item.host_id == host_id) {
                *slot = result;
            }
        }
        self.notify.notify_waiters();
    }

    pub fn snapshot(&self) -> Vec<HostResult> {
        self.results.lock().clone()
    }

    pub fn is_finished(&self) -> bool {
        self.results
            .lock()
            .iter()
            .all(|item| item.status != "running")
    }

    pub async fn wait_finished(&self) {
        loop {
            if self.is_finished() {
                return;
            }
            self.notify.notified().await;
        }
    }
}

#[derive(Default)]
pub struct CommandRegistry {
    entries: Mutex<HashMap<String, Arc<CommandEntry>>>,
    order: Mutex<VecDeque<(String, Instant)>>,
}

impl CommandRegistry {
    pub fn insert(&self, id: String, entry: Arc<CommandEntry>) {
        self.entries.lock().insert(id.clone(), entry);
        self.order.lock().push_back((id, Instant::now()));
        self.evict();
    }

    pub fn get(&self, id: &str) -> Option<Arc<CommandEntry>> {
        self.evict();
        self.entries.lock().get(id).cloned()
    }

    fn evict(&self) {
        let mut order = self.order.lock();
        let mut entries = self.entries.lock();
        while let Some((_, created)) = order.front() {
            let expired = created.elapsed() > RECORD_TTL;
            if !expired && order.len() <= MAX_RECORDS {
                break;
            }
            if let Some((evicted, _)) = order.pop_front() {
                entries.remove(&evicted);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_completion_per_host() {
        let entry = CommandEntry::new(
            "uptime".into(),
            vec![
                HostResult::running("a".into(), "A".into()),
                HostResult::running("b".into(), "B".into()),
            ],
        );
        assert!(!entry.is_finished());
        entry.complete(
            "a",
            HostResult::done("a".into(), "A".into(), "up".into(), String::new(), 0),
        );
        assert!(!entry.is_finished());
        entry.complete(
            "b",
            HostResult::error("b".into(), "B".into(), "boom".into()),
        );
        assert!(entry.is_finished());
        let snapshot = entry.snapshot();
        assert_eq!(snapshot[0].stdout.as_deref(), Some("up"));
        assert_eq!(snapshot[1].message.as_deref(), Some("boom"));
    }

    #[tokio::test]
    async fn wait_finished_returns_after_last_host() {
        let entry = Arc::new(CommandEntry::new(
            "uptime".into(),
            vec![HostResult::running("a".into(), "A".into())],
        ));
        let waiting = {
            let entry = entry.clone();
            tokio::spawn(async move { entry.wait_finished().await })
        };
        tokio::task::yield_now().await;
        entry.complete(
            "a",
            HostResult::done("a".into(), "A".into(), String::new(), String::new(), 0),
        );
        tokio::time::timeout(Duration::from_secs(2), waiting)
            .await
            .unwrap()
            .unwrap();
    }
}
