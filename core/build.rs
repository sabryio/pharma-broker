//! Build script for protobuf compilation
//!
//! Compiles proto/pharma.proto into Rust types using tonic-prost-build

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(false)
        .compile_protos(&["../proto/pharma.proto"], &["../proto"])?;

    println!("cargo:rerun-if-changed=../proto/pharma.proto");

    Ok(())
}
