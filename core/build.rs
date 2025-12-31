//! Build script for protobuf compilation
//!
//! Compiles proto/pharma.proto into Rust types using tonic-prost-build
//! Handles both local dev (proto at ../proto) and Docker (proto at ./proto)

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Check if proto is in current dir (Docker) or parent dir (local dev)
    let (proto_file, proto_dir) = if std::path::Path::new("proto/pharma.proto").exists() {
        ("proto/pharma.proto", "proto")
    } else {
        ("../proto/pharma.proto", "../proto")
    };

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&[proto_file], &[proto_dir])?;

    println!("cargo:rerun-if-changed={}", proto_file);

    Ok(())
}
