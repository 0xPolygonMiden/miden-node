use std::sync::Arc;

use miden_objects::transaction::ProvenTransaction;
use tokio::sync::Mutex;

#[cfg(test)]
pub mod test_utils;

// TODO: Better name than rpc?
pub mod msg;
pub mod tasks;

type SharedProvenTx = Arc<ProvenTransaction>;
type SharedMutVec<T> = Arc<Mutex<Vec<T>>>;
