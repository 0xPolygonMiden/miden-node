use std::{env, fs, path::PathBuf};

use anyhow::Context;
use prost::Message;

pub struct ProtoBuilder(tonic_build::Builder);

const RPC_PROTO: &str = "rpc.proto";

#[cfg(feature = "internal")]
const STORE_PROTO: &str = "store.proto";

#[cfg(feature = "internal")]
const BLOCK_PRODUCER_PROTO: &str = "block_producer.proto";

impl ProtoBuilder {
    /// Creates a new `Builder` from the provided `tonic_build::Builder`.
    /// The settings on the `Builder` are used to compile the proto files, including the `out_dir`
    /// directory.
    /// By default, the client is not included in the bindings (see `Builder::with_client`).
    pub fn new(builder: tonic_build::Builder) -> Self {
        Self(builder.build_client(false))
    }

    /// Compiles the proto bindings.
    /// Generated files are written to the `out_dir` set on the internal Builder.
    pub fn compile(self) -> anyhow::Result<()> {
        #[cfg(feature = "internal")]
        return generate_protos(
            self.0.build_client(true),
            &[RPC_PROTO, STORE_PROTO, BLOCK_PRODUCER_PROTO],
        );

        #[cfg(not(feature = "internal"))]
        generate_protos(self.0.build_client(false), &[RPC_PROTO])
    }
}

/// This wrapper function receives a `tonic_build::Builder` and uses it to generate the protobuf
/// bindings.
fn generate_protos(builder: tonic_build::Builder, proto_files: &[&str]) -> anyhow::Result<()> {
    let cwd: PathBuf = env::current_dir().context("current directory")?;
    let cwd = cwd
        .parent()
        .and_then(|p| p.parent())
        .context("navigating to grandparent directory")?;
    let proto_dir: PathBuf = cwd.join("proto");

    let out = env::var("OUT_DIR").context("env::OUT_DIR not set")?;
    let file_descriptor_path = PathBuf::from(out).join("file_descriptor_set.bin");

    let protos = proto_files
        .iter()
        .map(|proto_file| proto_dir.join(proto_file))
        .collect::<Vec<_>>();

    let includes = &[proto_dir];
    let file_descriptors = protox::compile(&protos, includes)?;
    fs::write(&file_descriptor_path, file_descriptors.encode_to_vec())
        .context("writing file descriptors")?;

    let mut prost_config = prost_build::Config::new();
    prost_config.skip_debug(["AccountId", "Digest"]);

    builder
        .file_descriptor_set_path(file_descriptor_path)
        .compile_protos_with_config(prost_config, &protos, includes)
        .context("compiling protobufs")?;
    Ok(())
}
