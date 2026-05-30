use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestLog {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub method: String,
    pub path: String,
    pub status_code: u16,
    pub duration_ms: u64,
    pub api_key: Option<String>,
    pub ip_address: Option<String>,
    pub error: Option<String>,
}

pub struct RequestLogger {
    logs: Arc<Mutex<Vec<RequestLog>>>,
}

impl RequestLogger {
    pub fn new() -> Self {
        Self {
            logs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn log_request(
        &self,
        method: String,
        path: String,
        status_code: u16,
        duration_ms: u64,
        api_key: Option<String>,
        ip_address: Option<String>,
        error: Option<String>,
    ) {
        let log = RequestLog {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            method,
            path,
            status_code,
            duration_ms,
            api_key,
            ip_address,
            error,
        };

        tracing::info!(
            request_id = %log.id,
            method = %log.method,
            path = %log.path,
            status = log.status_code,
            duration_ms = log.duration_ms,
            api_key = ?log.api_key,
            ip = ?log.ip_address,
            error = ?log.error,
            "API Request"
        );

        let mut logs = self.logs.lock().await;
        logs.push(log);
    }

    pub async fn get_logs(&self) -> Vec<RequestLog> {
        self.logs.lock().await.clone()
    }

    pub async fn get_logs_by_api_key(&self, api_key: &str) -> Vec<RequestLog> {
        self.logs
            .lock()
            .await
            .iter()
            .filter(|log| log.api_key.as_deref() == Some(api_key))
            .cloned()
            .collect()
    }

    pub async fn clear_logs(&self) {
        self.logs.lock().await.clear();
    }
}

impl Default for RequestLogger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_log_request() {
        let logger = RequestLogger::new();
        logger
            .log_request(
                "GET".to_string(),
                "/api/test".to_string(),
                200,
                100,
                Some("test_key".to_string()),
                Some("127.0.0.1".to_string()),
                None,
            )
            .await;

        let logs = logger.get_logs().await;
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].method, "GET");
        assert_eq!(logs[0].status_code, 200);
    }

    #[tokio::test]
    async fn test_get_logs_by_api_key() {
        let logger = RequestLogger::new();
        logger
            .log_request(
                "GET".to_string(),
                "/api/test".to_string(),
                200,
                100,
                Some("key1".to_string()),
                None,
                None,
            )
            .await;

        logger
            .log_request(
                "POST".to_string(),
                "/api/test".to_string(),
                201,
                150,
                Some("key2".to_string()),
                None,
                None,
            )
            .await;

        let logs = logger.get_logs_by_api_key("key1").await;
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].method, "GET");
    }
}
