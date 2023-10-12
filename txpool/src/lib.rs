use miden_objects::transaction::ProvenTransaction;

pub mod tasks;

#[cfg(test)]
pub mod test_utils;

pub type TxBatch = Vec<ProvenTransaction>;
