use std::{env, path::PathBuf};

use tonic_build::FileDescriptorSet;

const RPC_PROTO: &str = "rpc.proto";

#[cfg(feature = "internal")]
const STORE_PROTO: &str = "store.proto";

#[cfg(feature = "internal")]
const BLOCK_PRODUCER_PROTO: &str = "block_producer.proto";

pub fn rpc_file_descriptor() -> Result<FileDescriptorSet, protox::Error> {
    let proto_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("proto");
    protox::compile([RPC_PROTO], &[proto_dir])
}

#[cfg(feature = "internal")]
pub fn store_file_descriptor() -> Result<FileDescriptorSet, protox::Error> {
    let proto_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("proto");
    protox::compile([STORE_PROTO], &[proto_dir])
}

#[cfg(feature = "internal")]
pub fn block_producer_file_descriptor() -> Result<FileDescriptorSet, protox::Error> {
    let proto_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("proto");
    protox::compile([BLOCK_PRODUCER_PROTO], &[proto_dir])
}
