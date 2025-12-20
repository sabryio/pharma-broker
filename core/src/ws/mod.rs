//! WebSocket module for real-time updates
//!
//! Provides a broadcast-based pub/sub system to notify connected clients
//! about new offers, requests, and matches.

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
use std::sync::atomic::Ordering;
use tokio::time::{Duration, interval};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::api::routes::AppState;
use crate::domain::{Match, Offer, Request};
use crate::repository::{
    AuditLogRepository, MedicationMappingRepository, ReviewQueueRepository,
};

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

/// WebSocket handler
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
    // 1. Auth check
    let token = params.get("token");
    let expected_token = std::env::var("WS_TOKEN").unwrap_or_else(|_| "secret-token".to_string());

    if token != Some(&expected_token) {
        warn!("🚫 Unauthenticated WebSocket connection attempt");
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    }

    // 2. Connection limit check
    let current_connections = state.active_connections.load(Ordering::Relaxed);
    if current_connections >= 100 {
        warn!("🚫 WebSocket connection limit reached (100)");
        return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response();
    }

    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Handle a single WebSocket connection
async fn handle_socket<RQ, A, MM>(socket: WebSocket, state: AppState<RQ, A, MM>)
where
    RQ: ReviewQueueRepository + 'static,
    A: AuditLogRepository + 'static,
    MM: MedicationMappingRepository + 'static,
{
    // Increment connection count
    state.active_connections.fetch_add(1, Ordering::SeqCst);

    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.ws_tx.subscribe();

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
    let heartbeat_task = tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(30));
        loop {
            ticker.tick().await;
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
                Message::Pong(_) => {
                    // info!("Received pong from client");
                }
                _ => {}
            }
        }
    });

    // Wait for either active task to finish
    tokio::select! {
        _ = (&mut send_task) => {
            recv_task.abort();
            heartbeat_task.abort();
        },
        _ = (&mut recv_task) => {
            send_task.abort();
            heartbeat_task.abort();
        },
    }

    // Decrement connection count
    state.active_connections.fetch_sub(1, Ordering::SeqCst);
    info!("🔌 WebSocket client disconnected");
}
