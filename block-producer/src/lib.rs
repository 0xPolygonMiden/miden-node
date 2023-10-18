use std::sync::Arc;

use miden_objects::transaction::ProvenTransaction;

#[cfg(test)]
pub mod test_utils;

// TODO: Better name than rpc?
pub mod rpc;
pub mod tasks;

type SharedProvenTx = Arc<ProvenTransaction>;
