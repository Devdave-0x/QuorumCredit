mod auth;
mod logging;
mod webhook;

use axum::{
    extract::{State, Json},
    http::{StatusCode, Request},
    middleware::{self, Next},
    response::Response,
    routing::{get, post, delete},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tower_http::trace::TraceLayer;
use tracing_subscriber;

use auth::JwtAuth;
use logging::RequestLogger;
use webhook::{WebhookManager, WebhookEvent};

#[derive(Clone)]
pub struct AppState {
    jwt_auth: Arc<JwtAuth>,
    logger: Arc<RequestLogger>,
    webhook_manager: Arc<WebhookManager>,
}

#[derive(Serialize, Deserialize)]
pub struct AuthRequest {
    pub api_key: String,
}

#[derive(Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
}

#[derive(Serialize, Deserialize)]
pub struct WebhookSubscribeRequest {
    pub url: String,
    pub events: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct WebhookEventRequest {
    pub event_type: String,
    pub data: serde_json::Value,
}

async fn logging_middleware<B>(
    State(state): State<AppState>,
    req: Request<B>,
    next: Next,
) -> Response {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let start = Instant::now();

    let response = next.run(req).await;
    let duration = start.elapsed().as_millis() as u64;
    let status = response.status().as_u16();

    let api_key = None;
    let ip_address = None;

    state
        .logger
        .log_request(method, path, status, duration, api_key, ip_address, None)
        .await;

    response
}

async fn authenticate(
    State(state): State<AppState>,
    Json(payload): Json<AuthRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    match state.jwt_auth.generate_token(&payload.api_key, 24) {
        Ok(token) => Ok(Json(AuthResponse { token })),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

async fn verify_token(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let token = payload
        .get("token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing token".to_string()))?;

    match state.jwt_auth.verify_token(token) {
        Ok(claims) => Ok(Json(serde_json::json!({
            "valid": true,
            "api_key": claims.api_key,
            "exp": claims.exp
        }))),
        Err(e) => Err((StatusCode::UNAUTHORIZED, e.to_string())),
    }
}

async fn subscribe_webhook(
    State(state): State<AppState>,
    Json(payload): Json<WebhookSubscribeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    match state
        .webhook_manager
        .subscribe(payload.url, payload.events)
        .await
    {
        Ok(sub) => Ok(Json(serde_json::to_value(sub).unwrap())),
        Err(e) => Err((StatusCode::BAD_REQUEST, e.to_string())),
    }
}

async fn unsubscribe_webhook(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<StatusCode, (StatusCode, String)> {
    let webhook_id = payload
        .get("webhook_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing webhook_id".to_string()))?;

    match state.webhook_manager.unsubscribe(webhook_id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err((StatusCode::NOT_FOUND, e.to_string())),
    }
}

async fn deliver_webhook_event(
    State(state): State<AppState>,
    Json(payload): Json<WebhookEventRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let event = WebhookEvent {
        id: uuid::Uuid::new_v4().to_string(),
        event_type: payload.event_type,
        timestamp: chrono::Utc::now(),
        data: payload.data,
    };

    match state.webhook_manager.deliver_event(event).await {
        Ok(_) => Ok(StatusCode::ACCEPTED),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

async fn get_logs(
    State(state): State<AppState>,
) -> Json<Vec<logging::RequestLog>> {
    let logs = state.logger.get_logs().await;
    Json(logs)
}

async fn health_check() -> &'static str {
    "OK"
}

pub async fn run_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "default_secret".to_string());
    
    let state = AppState {
        jwt_auth: Arc::new(JwtAuth::new(jwt_secret)),
        logger: Arc::new(RequestLogger::new()),
        webhook_manager: Arc::new(WebhookManager::new()),
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/auth/token", post(authenticate))
        .route("/auth/verify", post(verify_token))
        .route("/webhooks/subscribe", post(subscribe_webhook))
        .route("/webhooks/unsubscribe", delete(unsubscribe_webhook))
        .route("/webhooks/events", post(deliver_webhook_event))
        .route("/logs", get(get_logs))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            logging_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("Server listening on port {}", port);

    axum::serve(listener, app).await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()?;

    run_server(port).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_creation() {
        let state = AppState {
            jwt_auth: Arc::new(JwtAuth::new("test_secret".to_string())),
            logger: Arc::new(RequestLogger::new()),
            webhook_manager: Arc::new(WebhookManager::new()),
        };

        assert!(state.jwt_auth.is_some());
    }
}
