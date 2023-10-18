use std::sync::Arc;

use miden_objects::transaction::ProvenTransaction;

#[cfg(test)]
pub mod test_utils;

pub mod tasks;

type SharedProvenTx = Arc<ProvenTransaction>;
