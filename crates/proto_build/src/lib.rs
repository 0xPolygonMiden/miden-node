use std::{env, fs, path::PathBuf};

use anyhow::Context;
use prost::Message;
use tonic_build::Builder;

/// A builder for the `rpc` proto bindings.
pub struct RpcBuilder(Builder);
impl RpcBuilder {
    /// Creates a new `RpcBuilder` from the provided `Builder`.
    /// The settings on the `Builder` are used to compile the proto files, including the `out_dir`
    /// directory.
    /// By default, the client is not included in the bindings (see `RpcBuilder::with_client`).
    pub fn new(builder: Builder) -> Self {
        Self(builder.build_client(false))
    }

    /// Sets whether the bindings should include the client.
    #[must_use]
    pub fn with_client(self, client: bool) -> Self {
        Self(self.0.build_client(client))
    }

    /// Compiles the proto bindings for the `rpc` service.
    /// Generated files are written to the `out_dir` set on the internal Builder.
    pub fn compile(self) -> anyhow::Result<()> {
        generate_protos(self.0, "rpc.proto")
    }
}

/// A builder for the `store` proto bindings.
pub struct StoreBuilder(Builder);
impl StoreBuilder {
    /// Creates a new `StoreBuilder` from the provided `Builder`.
    /// The settings on the `Builder` are used to compile the proto files, including the `out_dir`
    /// directory.
    pub fn new(builder: Builder) -> Self {
        Self(builder)
    }

    /// Compiles the proto bindings for the `store` service.
    /// Generated files are written to the `out_dir` set on the internal Builder.
    pub fn compile(self) -> anyhow::Result<()> {
        generate_protos(self.0, "store.proto")
    }
}

/// A builder for the `block_producer` proto bindings.
pub struct BlockProducerBuilder(Builder);
impl BlockProducerBuilder {
    /// Creates a new `BlockProducerBuilder` from the provided `Builder`.
    /// The settings on the `Builder` are used to compile the proto files, including the `out_dir`
    /// directory.
    pub fn new(builder: Builder) -> Self {
        Self(builder)
    }

    /// Compiles the proto bindings for the `block_producer` service.
    /// Generated files are written to the `out_dir` set on the internal Builder.
    pub fn compile(self) -> anyhow::Result<()> {
        generate_protos(self.0, "block_producer.proto")
    }
}

/// This wrapper function receives a `Builder` and uses it to generate the protobuf bindings.
/// The reason of this wrapper is to avoid the need of copying the proto definitions.
fn generate_protos(builder: Builder, proto_file: &str) -> anyhow::Result<()> {
    let cwd: PathBuf = env::current_dir().context("current directory")?;
    let cwd = cwd
        .parent()
        .and_then(|p| p.parent())
        .context("navigating to grandparent directory")?;
    let proto_dir: PathBuf = cwd.join("proto");

    let out = env::var("OUT_DIR").context("env::OUT_DIR not set")?;
    let file_descriptor_path = PathBuf::from(out).join("file_descriptor_set.bin");

    let protos = &[proto_dir.join(proto_file)];

    let includes = &[proto_dir];
    let file_descriptors = protox::compile(protos, includes)?;
    fs::write(&file_descriptor_path, file_descriptors.encode_to_vec())
        .context("writing file descriptors")?;

    let mut prost_config = prost_build::Config::new();
    prost_config.skip_debug(["AccountId", "Digest"]);

    builder
        .file_descriptor_set_path(file_descriptor_path)
        .compile_protos_with_config(prost_config, protos, includes)
        .context("compiling protobufs")?;
    Ok(())
}
