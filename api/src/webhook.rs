use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use std::sync::Arc;
use tokio::sync::Mutex;
use reqwest::Client;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WebhookError {
    #[error("Failed to send webhook: {0}")]
    SendError(String),
    #[error("Invalid webhook URL")]
    InvalidUrl,
    #[error("Webhook not found")]
    NotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    pub id: String,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookSubscription {
    pub id: String,
    pub url: String,
    pub events: Vec<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub retry_count: u32,
    pub max_retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDelivery {
    pub id: String,
    pub webhook_id: String,
    pub event_id: String,
    pub status: DeliveryStatus,
    pub timestamp: DateTime<Utc>,
    pub response_code: Option<u16>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeliveryStatus {
    Pending,
    Success,
    Failed,
    Retrying,
}

pub struct WebhookManager {
    subscriptions: Arc<Mutex<Vec<WebhookSubscription>>>,
    deliveries: Arc<Mutex<Vec<WebhookDelivery>>>,
    client: Client,
}

impl WebhookManager {
    pub fn new() -> Self {
        Self {
            subscriptions: Arc::new(Mutex::new(Vec::new())),
            deliveries: Arc::new(Mutex::new(Vec::new())),
            client: Client::new(),
        }
    }

    pub async fn subscribe(
        &self,
        url: String,
        events: Vec<String>,
    ) -> Result<WebhookSubscription, WebhookError> {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(WebhookError::InvalidUrl);
        }

        let subscription = WebhookSubscription {
            id: Uuid::new_v4().to_string(),
            url,
            events,
            active: true,
            created_at: Utc::now(),
            retry_count: 0,
            max_retries: 3,
        };

        let mut subs = self.subscriptions.lock().await;
        subs.push(subscription.clone());

        tracing::info!(
            webhook_id = %subscription.id,
            url = %subscription.url,
            events = ?subscription.events,
            "Webhook subscription created"
        );

        Ok(subscription)
    }

    pub async fn unsubscribe(&self, webhook_id: &str) -> Result<(), WebhookError> {
        let mut subs = self.subscriptions.lock().await;
        if let Some(pos) = subs.iter().position(|s| s.id == webhook_id) {
            subs.remove(pos);
            tracing::info!(webhook_id = %webhook_id, "Webhook subscription removed");
            Ok(())
        } else {
            Err(WebhookError::NotFound)
        }
    }

    pub async fn deliver_event(&self, event: WebhookEvent) -> Result<(), WebhookError> {
        let subs = self.subscriptions.lock().await;
        
        for sub in subs.iter().filter(|s| s.active && s.events.contains(&event.event_type)) {
            let delivery = WebhookDelivery {
                id: Uuid::new_v4().to_string(),
                webhook_id: sub.id.clone(),
                event_id: event.id.clone(),
                status: DeliveryStatus::Pending,
                timestamp: Utc::now(),
                response_code: None,
                error: None,
            };

            let mut deliveries = self.deliveries.lock().await;
            deliveries.push(delivery.clone());
            drop(deliveries);

            self.send_webhook(sub.clone(), event.clone()).await;
        }

        Ok(())
    }

    async fn send_webhook(&self, subscription: WebhookSubscription, event: WebhookEvent) {
        let delivery_id = Uuid::new_v4().to_string();
        let mut attempt = 0;

        loop {
            attempt += 1;
            match self.client.post(&subscription.url).json(&event).send().await {
                Ok(response) => {
                    let status_code = response.status().as_u16();
                    let delivery_status = if response.status().is_success() {
                        DeliveryStatus::Success
                    } else {
                        DeliveryStatus::Failed
                    };

                    self.update_delivery(&delivery_id, delivery_status, Some(status_code), None)
                        .await;

                    tracing::info!(
                        delivery_id = %delivery_id,
                        webhook_id = %subscription.id,
                        status = status_code,
                        "Webhook delivered"
                    );
                    break;
                }
                Err(e) => {
                    if attempt < subscription.max_retries {
                        let status = if attempt < subscription.max_retries - 1 {
                            DeliveryStatus::Retrying
                        } else {
                            DeliveryStatus::Failed
                        };
                        self.update_delivery(&delivery_id, status, None, Some(e.to_string()))
                            .await;

                        tracing::warn!(
                            delivery_id = %delivery_id,
                            webhook_id = %subscription.id,
                            attempt = attempt,
                            error = %e,
                            "Webhook delivery failed, retrying"
                        );

                        tokio::time::sleep(tokio::time::Duration::from_secs(2_u64.pow(attempt as u32)))
                            .await;
                    } else {
                        self.update_delivery(&delivery_id, DeliveryStatus::Failed, None, Some(e.to_string()))
                            .await;

                        tracing::error!(
                            delivery_id = %delivery_id,
                            webhook_id = %subscription.id,
                            error = %e,
                            "Webhook delivery failed after retries"
                        );
                        break;
                    }
                }
            }
        }
    }

    async fn update_delivery(
        &self,
        delivery_id: &str,
        status: DeliveryStatus,
        response_code: Option<u16>,
        error: Option<String>,
    ) {
        let mut deliveries = self.deliveries.lock().await;
        if let Some(delivery) = deliveries.iter_mut().find(|d| d.id == delivery_id) {
            delivery.status = status;
            delivery.response_code = response_code;
            delivery.error = error;
        }
    }

    pub async fn get_subscriptions(&self) -> Vec<WebhookSubscription> {
        self.subscriptions.lock().await.clone()
    }

    pub async fn get_deliveries(&self) -> Vec<WebhookDelivery> {
        self.deliveries.lock().await.clone()
    }
}

impl Default for WebhookManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_subscribe_webhook() {
        let manager = WebhookManager::new();
        let result = manager
            .subscribe(
                "https://example.com/webhook".to_string(),
                vec!["loan.created".to_string()],
            )
            .await;

        assert!(result.is_ok());
        let sub = result.unwrap();
        assert_eq!(sub.events, vec!["loan.created"]);
    }

    #[tokio::test]
    async fn test_invalid_webhook_url() {
        let manager = WebhookManager::new();
        let result = manager
            .subscribe(
                "invalid-url".to_string(),
                vec!["loan.created".to_string()],
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_unsubscribe_webhook() {
        let manager = WebhookManager::new();
        let sub = manager
            .subscribe(
                "https://example.com/webhook".to_string(),
                vec!["loan.created".to_string()],
            )
            .await
            .unwrap();

        let result = manager.unsubscribe(&sub.id).await;
        assert!(result.is_ok());

        let subs = manager.get_subscriptions().await;
        assert_eq!(subs.len(), 0);
    }
}
