//! WebSocket module for real-time updates
//!
//! Provides a broadcast-based pub/sub system to notify connected clients
//! about new offers, requests, and matches.

mod auth;

pub use auth::{
    TokenClaims, TokenError, WsAuthConfig, generate_token, validate_hmac_token,
    validate_simple_token,
};

use axum::{
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::api::routes::AppState;
use crate::domain::{Match, Offer, Request};
use crate::repository::{AuditLogRepository, MedicationMappingRepository, ReviewQueueRepository};

/// Payload for match status change events
#[derive(Debug, Clone, Serialize)]
pub struct MatchStatusEvent {
    pub match_id: String,
    pub user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Real-time events sent over WebSocket
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "payload")]
pub enum WsEvent {
    /// New offer created
    NewOffer(Offer),
    /// New request created
    NewRequest(Request),
    /// New match found
    NewMatch(Match),
    /// Match was confirmed by operator
    MatchConfirmed(MatchStatusEvent),
    /// Match was rejected by operator
    MatchRejected(MatchStatusEvent),
    /// Item queued for review
    ReviewQueued(Uuid),
    /// Ping message (keep-alive)
    Ping,
}

/// WebSocket handler with configurable authentication
pub async fn ws_handler<RQ, A, MM>(
    ws: WebSocketUpgrade,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<AppState<RQ, A, MM>>,
) -> impl IntoResponse
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    let auth_config = WsAuthConfig::from_env();

    // 1. Auth check
    if auth_config.enabled {
        let token = params.get(&auth_config.token_param);

        match token {
            None => {
                warn!("🚫 Missing WebSocket token");
                return axum::http::StatusCode::UNAUTHORIZED.into_response();
            }
            Some(token_str) => {
                // Try HMAC validation first, fallback to simple token
                let validation = if token_str.contains(':') {
                    validate_hmac_token(token_str, &auth_config.secret)
                } else {
                    validate_simple_token(token_str)
                };

                match validation {
                    Ok(claims) => {
                        info!(
                            user_id = %claims.user_id,
                            scopes = ?claims.scopes,
                            "✅ WebSocket client authenticated"
                        );
                    }
                    Err(e) => {
                        warn!(error = %e, "🚫 WebSocket authentication failed");
                        return axum::http::StatusCode::UNAUTHORIZED.into_response();
                    }
                }
            }
        }
    }

    // 2. Connection limit check
    let current_connections = state.active_connections.load(Ordering::Relaxed);
    if current_connections >= auth_config.max_connections {
        warn!(
            current = current_connections,
            max = auth_config.max_connections,
            "🚫 WebSocket connection limit reached"
        );
        return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response();
    }

    ws.on_upgrade(move |socket| handle_socket(socket, state, auth_config))
}

/// Handle a single WebSocket connection with inactivity timeout
async fn handle_socket<RQ, A, MM>(
    socket: WebSocket,
    state: AppState<RQ, A, MM>,
    config: WsAuthConfig,
) where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    // Increment connection count
    state.active_connections.fetch_add(1, Ordering::SeqCst);

    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.ws_tx.subscribe();

    // Track last activity for inactivity timeout
    let last_activity = Arc::new(RwLock::new(Instant::now()));
    let last_activity_clone = last_activity.clone();

    info!("🔌 New WebSocket client connected");

    // Task for sending broadcast events to this client
    let mut send_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            let msg = match serde_json::to_string(&event) {
                Ok(json) => Message::Text(json.into()),
                Err(e) => {
                    error!("Failed to serialize WS event: {}", e);
                    continue;
                }
            };

            if let Err(e) = sender.send(msg).await {
                error!("WebSocket send error: {}", e);
                break;
            }
        }
    });

    // Task for sending periodic heartbeats (pings)
    let ws_tx_heartbeat = state.ws_tx.clone();
    let heartbeat_interval = config.heartbeat_interval();
    let inactivity_timeout = config.inactivity_timeout();
    let last_activity_heartbeat = last_activity.clone();

    let mut heartbeat_task = tokio::spawn(async move {
        let mut ticker = interval(heartbeat_interval);
        loop {
            ticker.tick().await;

            // Check for inactivity
            let elapsed = {
                let last = last_activity_heartbeat.read().await;
                last.elapsed()
            };

            if elapsed > inactivity_timeout {
                warn!(
                    elapsed_secs = elapsed.as_secs(),
                    timeout_secs = inactivity_timeout.as_secs(),
                    "🔌 WebSocket client inactive, disconnecting"
                );
                break;
            }

            if let Err(e) = ws_tx_heartbeat.send(WsEvent::Ping) {
                error!("Failed to broadcast WS ping: {}", e);
                break;
            }
        }
    });

    // Task for receiving messages from this client (e.g. pongs)
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Close(_) => break,
                Message::Pong(_) | Message::Ping(_) => {
                    // Update last activity on any client response
                    let mut last = last_activity_clone.write().await;
                    *last = Instant::now();
                }
                Message::Text(_) => {
                    // Update activity on any message
                    let mut last = last_activity_clone.write().await;
                    *last = Instant::now();
                }
                _ => {}
            }
        }
    });

    // Wait for any task to finish
    tokio::select! {
        _ = (&mut send_task) => {
            recv_task.abort();
            heartbeat_task.abort();
        },
        _ = (&mut recv_task) => {
            send_task.abort();
            heartbeat_task.abort();
        },
        _ = (&mut heartbeat_task) => {
            send_task.abort();
            recv_task.abort();
        },
    }

    // Decrement connection count
    state.active_connections.fetch_sub(1, Ordering::SeqCst);
    info!("🔌 WebSocket client disconnected");
}
