use std::{env, fs, path::PathBuf};

use miette::IntoDiagnostic;
use prost::Message;

/// Generates Rust protobuf bindings from .proto files in the root directory.
///
/// This is done only if BUILD_PROTO environment variable is set to `1` to avoid running the script
/// on crates.io where repo-level .proto files are not available.
fn main() -> miette::Result<()> {
    println!("cargo:rerun-if-changed=generated");
    println!("cargo:rerun-if-changed=../../proto");

    // skip this build script in BUILD_PROTO environment variable is not set to `1`
    if env::var("BUILD_PROTO").unwrap_or("0".to_string()) == "0" {
        return Ok(());
    }

    // Compute the directory of the `proto` definitions
    let cwd: PathBuf = env::current_dir().into_diagnostic()?;

    let cwd = cwd
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| miette::miette!("Failed to navigate two directories up"))?;

    let proto_dir: PathBuf = cwd.join("proto");

    // Compute the compiler's target file path.
    let out = env::var("OUT_DIR").into_diagnostic()?;
    let file_descriptor_path = PathBuf::from(out).join("file_descriptor_set.bin");

    // Compile the proto file for all servers APIs
    let protos = &[
        proto_dir.join("block_producer.proto"),
        proto_dir.join("store.proto"),
        proto_dir.join("rpc.proto"),
    ];
    let includes = &[proto_dir];
    let file_descriptors = protox::compile(protos, includes)?;
    fs::write(&file_descriptor_path, file_descriptors.encode_to_vec()).into_diagnostic()?;

    let mut prost_config = prost_build::Config::new();
    prost_config.skip_debug(["AccountId", "Digest"]);

    // Generate the stub of the user facing server from its proto file
    tonic_build::configure()
        .file_descriptor_set_path(&file_descriptor_path)
        .skip_protoc_run()
        .out_dir("src/generated")
        .compile_with_config(prost_config, protos, includes)
        .into_diagnostic()?;

    Ok(())
}
