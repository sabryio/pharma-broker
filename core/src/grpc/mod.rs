//! gRPC module for Go bridge communication
//!
//! Implements the PharmaCore gRPC service defined in proto/pharma.proto

mod server;

pub use server::start_grpc_server;
