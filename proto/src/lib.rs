use protox::prost::Message;
use tonic_build::FileDescriptorSet;

pub fn rpc_api_descriptor() -> FileDescriptorSet {
    let bytes = include_bytes!(concat!(env!("OUT_DIR"), "/", "rpc_file_descriptor.bin"));
    FileDescriptorSet::decode(&bytes[..]).expect("Failed to decode rpc FileDescriptorSet")
}

#[cfg(feature = "internal")]
pub fn store_api_descriptor() -> FileDescriptorSet {
    let bytes = include_bytes!(concat!(env!("OUT_DIR"), "/", "store_file_descriptor.bin"));
    FileDescriptorSet::decode(&bytes[..]).expect("Failed to decode store FileDescriptorSet")
}

#[cfg(feature = "internal")]
pub fn block_producer_api_descriptor() -> FileDescriptorSet {
    let bytes = include_bytes!(concat!(env!("OUT_DIR"), "/", "block_producer_file_descriptor.bin"));
    FileDescriptorSet::decode(&bytes[..])
        .expect("Failed to decode block producer FileDescriptorSet")
}
