//! gRPC module for Go bridge communication
//!
//! Implements the PharmaCore gRPC service defined in proto/pharma.proto

mod params;
mod server;

// Include generated proto types
pub mod pharma {
    tonic::include_proto!("pharma");
}

pub use params::{GrpcDependencies, GrpcRepositories};
pub use server::{PharmaCoreService, start_grpc_server};
