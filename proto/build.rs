use std::{env, fs, path::PathBuf};

use anyhow::Context;
use prost::Message;

const RPC_PROTO: &str = "rpc.proto";
const STORE_PROTO: &str = "store.proto";
const BLOCK_PRODUCER_PROTO: &str = "block_producer.proto";

/// Generates Rust protobuf bindings from .proto files in the root directory.
///
/// This is done only if `BUILD_PROTO` environment variable is set to `1` to avoid running the
/// script on crates.io where repo-level .proto files are not available.
fn main() -> anyhow::Result<()> {
    println!("cargo::rerun-if-changed=../proto");
    println!("cargo::rerun-if-env-changed=BUILD_PROTO");

    let out = env::var("OUT_DIR").context("env::OUT_DIR not set")?;
    let file_descriptor_path = PathBuf::from(out).join("file_descriptor_set.bin");

    let crate_root: PathBuf = env!("CARGO_MANIFEST_DIR").into();
    let proto_dir = crate_root.join("proto");

    let includes = &[proto_dir];
    let file_descriptors =
        protox::compile([RPC_PROTO, STORE_PROTO, BLOCK_PRODUCER_PROTO], includes)?;
    fs::write(&file_descriptor_path, file_descriptors.encode_to_vec())
        .context("writing file descriptors")?;

    Ok(())
}
