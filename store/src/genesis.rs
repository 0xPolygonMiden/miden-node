use miden_crypto::merkle::{EmptySubtreeRoots, MmrPeaks, TieredSmt};
use miden_node_proto::block_header;
use miden_objects::{notes::NOTE_LEAF_DEPTH, Digest};

use crate::state::ACCOUNT_DB_DEPTH;

/// Generates the header of the genesis block. The timestamp is currently hardcoded to be the UNIX epoch.
pub fn genesis_header() -> block_header::BlockHeader {
    block_header::BlockHeader {
        prev_hash: Some(Digest::default().into()),
        block_num: 0,
        chain_root: Some(MmrPeaks::new(0, Vec::new()).unwrap().hash_peaks().into()),
        account_root: Some(EmptySubtreeRoots::entry(ACCOUNT_DB_DEPTH, 0).into()),
        nullifier_root: Some(TieredSmt::default().root().into()),
        note_root: Some(EmptySubtreeRoots::entry(NOTE_LEAF_DEPTH, 0).into()),
        batch_root: Some(Digest::default().into()),
        proof_hash: Some(Digest::default().into()),
        version: 0,
        timestamp: 0,
    }
}
