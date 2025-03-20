use std::{env, fs, path::PathBuf};

use anyhow::Context;
use prost::Message;

const RPC_PROTO: &str = "rpc.proto";
const STORE_PROTO: &str = "store.proto";
const BLOCK_PRODUCER_PROTO: &str = "block_producer.proto";

pub struct ProtoBuilder(tonic_build::Builder);

impl ProtoBuilder {
    /// Creates a new `Builder` from the provided `tonic_build::Builder`.
    /// The settings on the `Builder` are used to compile the proto files, including the `out_dir`
    /// directory.
    pub fn new(builder: tonic_build::Builder) -> Self {
        Self(builder)
    }

    /// Compiles the proto bindings. Generated files are written to the `out_dir` set on the
    /// internal Builder.
    ///
    /// By default, the rpc client bindings are not generated. For that, set the "rpc-client" flag.
    pub fn compile(self) -> anyhow::Result<()> {
        let build_rpc_client = cfg!(feature = "rpc-client");
        generate_protos(self.0.clone().build_client(build_rpc_client), &[RPC_PROTO])?;
        generate_protos(self.0, &[STORE_PROTO, BLOCK_PRODUCER_PROTO])
    }
}

/// This wrapper function receives a `tonic_build::Builder` and uses it to generate the protobuf
/// bindings.
fn generate_protos(builder: tonic_build::Builder, proto_files: &[&str]) -> anyhow::Result<()> {
    let out = env::var("OUT_DIR").context("env::OUT_DIR not set")?;
    let file_descriptor_path = PathBuf::from(out).join("file_descriptor_set.bin");

    let crate_root: PathBuf = env!("CARGO_MANIFEST_DIR").into();
    let proto_dir = crate_root.join("proto");

    let includes = &[proto_dir];
    let file_descriptors = protox::compile(proto_files, includes)?;
    fs::write(&file_descriptor_path, file_descriptors.encode_to_vec())
        .context("writing file descriptors")?;

    let mut prost_config = prost_build::Config::new();
    prost_config.skip_debug(["AccountId", "Digest"]);

    builder
        .file_descriptor_set_path(file_descriptor_path)
        .compile_protos_with_config(prost_config, proto_files, includes)
        .context("compiling protobufs")?;
    Ok(())
}
