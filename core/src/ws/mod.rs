//! WebSocket module for real-time updates
//!
//! Provides a broadcast-based pub/sub system to notify connected clients
//! about new offers, requests, and matches.

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use tracing::{error, info};

use crate::api::routes::AppState;
use crate::domain::{Match, Offer, Request};
use crate::repository::{GroupRepository, MatchRepository, OfferRepository, RequestRepository};

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
    /// Ping message (keep-alive)
    Ping,
}

/// WebSocket handler
pub async fn ws_handler<O, R, M, G>(
    ws: WebSocketUpgrade,
    State(state): State<AppState<O, R, M, G>>,
) -> impl IntoResponse
where
    O: OfferRepository + 'static,
    R: RequestRepository + 'static,
    M: MatchRepository + 'static,
    G: GroupRepository + 'static,
{
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Handle a single WebSocket connection
async fn handle_socket<O, R, M, G>(socket: WebSocket, state: AppState<O, R, M, G>)
where
    O: OfferRepository + 'static,
    R: RequestRepository + 'static,
    M: MatchRepository + 'static,
    G: GroupRepository + 'static,
{
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.ws_tx.subscribe();

    info!("🔌 New WebSocket client connected");

    // Task for sending broadcast events to this client
    let mut send_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            let msg = match serde_json::to_string(&event) {
                Ok(json) => Message::Text(json.into()),
                Err(e) => {
                    error!("Fata to serialize WS event: {}", e);
                    continue;
                }
            };

            if let Err(e) = sender.send(msg).await {
                error!("WebSocket send error: {}", e);
                break;
            }
        }
    });

    // Task for receiving messages from this client (e.g. pings)
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Close(_) => break,
                Message::Ping(p) => {
                    // Axum handles pings automatically, but we can log
                    info!("Received ping: {:?}", p);
                }
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    info!("🔌 WebSocket client disconnected");
}
