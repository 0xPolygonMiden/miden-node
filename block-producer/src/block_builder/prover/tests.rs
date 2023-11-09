// prover tests
// 1. account validation works
// 2. `BlockProver::compute_account_root()` works
//   + make the updates outside the VM, and compare root

use std::sync::Arc;

use miden_air::FieldElement;
use miden_mock::mock::block::mock_block_header;
use miden_node_proto::domain::AccountInputRecord;
use miden_objects::crypto::merkle::MmrPeaks;

use crate::{batch_builder::TransactionBatch, test_utils::DummyProvenTxGenerator};

use super::*;

/// Tests that `BlockWitness` constructor fails if the store and transaction batches contain a
/// different set of account ids.
///
/// The store will contain accounts 1 & 2, while the transaction batches will contain 2 & 3.
#[test]
fn test_block_witness_validation_inconsistent_account_ids() {
    let tx_gen = DummyProvenTxGenerator::new();
    let account_id_1 = unsafe { AccountId::new_unchecked(Felt::ZERO) };
    let account_id_2 = unsafe { AccountId::new_unchecked(Felt::ONE) };
    let account_id_3 = unsafe { AccountId::new_unchecked(Felt::new(42)) };

    let block_inputs_from_store: BlockInputs = {
        let block_header = mock_block_header(Felt::ZERO, None, None, &[]);
        let chain_peaks = MmrPeaks::new(0, Vec::new()).unwrap();

        let account_states = vec![
            AccountInputRecord {
                account_id: account_id_1,
                account_hash: Digest::default(),
                proof: MerklePath::default(),
            },
            AccountInputRecord {
                account_id: account_id_2,
                account_hash: Digest::default(),
                proof: MerklePath::default(),
            },
        ];

        BlockInputs {
            block_header,
            chain_peaks,
            account_states,
            nullifiers: Vec::new(),
        }
    };

    let batches: Vec<SharedTxBatch> = {
        let batch_1 = {
            let tx = Arc::new(tx_gen.dummy_proven_tx_with_params(
                account_id_2,
                Digest::default(),
                Digest::default(),
                Vec::new(),
            ));

            Arc::new(TransactionBatch::new(vec![tx]))
        };

        let batch_2 = {
            let tx = Arc::new(tx_gen.dummy_proven_tx_with_params(
                account_id_3,
                Digest::default(),
                Digest::default(),
                Vec::new(),
            ));

            Arc::new(TransactionBatch::new(vec![tx]))
        };

        vec![batch_1, batch_2]
    };

    let block_witness_result = BlockWitness::new(block_inputs_from_store, batches);

    assert_eq!(
        block_witness_result,
        Err(BuildBlockError::InconsistentAccountIds(vec![account_id_1, account_id_3]))
    );
}
