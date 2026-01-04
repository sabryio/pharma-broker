//! gRPC module for Go bridge communication
//!
//! Implements the PharmaCore gRPC service defined in proto/pharma.proto

mod bridge_client;
mod params;
mod server;

// Include generated proto types
pub mod pharma {
    tonic::include_proto!("pharma");
}

pub use bridge_client::{BridgeClient, BridgeClientConfig, BridgeClientError};
pub use params::{GrpcDependencies, GrpcRepositories};
pub use server::{PharmaCoreService, start_grpc_server};
