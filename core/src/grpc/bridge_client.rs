//! Bridge Client for calling Go Bridge gRPC service
//!
//! Provides a client to call the PharmaBridge service running on the Go bridge,
//! enabling the Rust core to send messages via WhatsApp.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tonic::transport::Channel;

use super::pharma::{
    SendMessageRequest, SendMessageResponse, pharma_bridge_client::PharmaBridgeClient,
};

/// Error type for bridge client operations
#[derive(Debug, thiserror::Error)]
pub enum BridgeClientError {
    #[error("Failed to connect to bridge: {0}")]
    ConnectionError(String),

    #[error("gRPC call failed: {0}")]
    GrpcError(#[from] tonic::Status),

    #[error("Bridge returned error: {0}")]
    BridgeError(String),

    #[error("Bridge client not connected")]
    NotConnected,
}

/// Configuration for the bridge client
#[derive(Debug, Clone)]
pub struct BridgeClientConfig {
    /// Address of the Go bridge gRPC server (e.g., "http://localhost:50052")
    pub address: String,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Request timeout
    pub request_timeout: Duration,
}

impl Default for BridgeClientConfig {
    fn default() -> Self {
        Self {
            address: "http://localhost:50052".to_string(),
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(30),
        }
    }
}

/// Client for calling the Go Bridge's PharmaBridge gRPC service
pub struct BridgeClient {
    client: Arc<RwLock<Option<PharmaBridgeClient<Channel>>>>,
    config: BridgeClientConfig,
}

impl BridgeClient {
    /// Create a new bridge client with the given configuration
    pub fn new(config: BridgeClientConfig) -> Self {
        Self {
            client: Arc::new(RwLock::new(None)),
            config,
        }
    }

    /// Connect to the bridge gRPC server
    pub async fn connect(&self) -> Result<(), BridgeClientError> {
        let channel = Channel::from_shared(self.config.address.clone())
            .map_err(|e| BridgeClientError::ConnectionError(e.to_string()))?
            .connect_timeout(self.config.connect_timeout)
            .timeout(self.config.request_timeout)
            .connect()
            .await
            .map_err(|e| BridgeClientError::ConnectionError(e.to_string()))?;

        let client = PharmaBridgeClient::new(channel);
        *self.client.write().await = Some(client);

        tracing::info!(address = %self.config.address, "✅ Connected to Go bridge");
        Ok(())
    }

    /// Check if the client is connected
    pub async fn is_connected(&self) -> bool {
        self.client.read().await.is_some()
    }

    /// Send a message via the Go bridge to WhatsApp
    ///
    /// # Arguments
    /// * `recipient_jid` - WhatsApp JID of the recipient
    /// * `content` - Message content to send
    /// * `reference_id` - Optional reference ID for tracking
    ///
    /// # Returns
    /// * `Ok(message_id)` - The WhatsApp message ID on success
    /// * `Err(BridgeClientError)` - Error details on failure
    pub async fn send_message(
        &self,
        recipient_jid: String,
        content: String,
        reference_id: Option<String>,
    ) -> Result<String, BridgeClientError> {
        // Try to reconnect if not connected
        if !self.is_connected().await {
            tracing::warn!("Bridge client not connected, attempting to reconnect...");
            if let Err(e) = self.connect().await {
                tracing::error!(error = %e, "Failed to reconnect to bridge");
                return Err(BridgeClientError::NotConnected);
            }
        }

        let client_guard = self.client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or(BridgeClientError::NotConnected)?;

        let request = SendMessageRequest {
            recipient_jid: recipient_jid.clone(),
            content: content.clone(),
            reference_id,
        };

        tracing::info!(
            recipient = %recipient_jid,
            content_len = content.len(),
            "📤 Sending message via bridge"
        );

        let response: SendMessageResponse =
            client.clone().send_message(request).await?.into_inner();

        if response.success {
            tracing::info!(
                message_id = %response.message_id,
                "✅ Message sent successfully"
            );
            Ok(response.message_id)
        } else {
            let error = response
                .error
                .unwrap_or_else(|| "Unknown error".to_string());
            tracing::error!(error = %error, "❌ Bridge returned error");
            Err(BridgeClientError::BridgeError(error))
        }
    }
}

impl Clone for BridgeClient {
    fn clone(&self) -> Self {
        Self {
            client: Arc::clone(&self.client),
            config: self.config.clone(),
        }
    }
}
