use std::sync::Arc;

use miden_air::FieldElement;
use miden_mock::mock::block::mock_block_header;
use miden_node_proto::domain::AccountInputRecord;
use miden_objects::crypto::merkle::MmrPeaks;
use miden_vm::crypto::SimpleSmt;

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

/// Tests that `BlockWitness` constructor fails if the store and transaction batches contain a
/// different at least 1 account who's state hash is different.
///
/// Only account 1 will have a different state hash
#[test]
fn test_block_witness_validation_inconsistent_account_hashes() {
    let tx_gen = DummyProvenTxGenerator::new();
    let account_id_1 = unsafe { AccountId::new_unchecked(Felt::ZERO) };
    let account_id_2 = unsafe { AccountId::new_unchecked(Felt::ONE) };

    let account_1_hash_store =
        Digest::new([Felt::from(1u64), Felt::from(2u64), Felt::from(3u64), Felt::from(4u64)]);
    let account_1_hash_batches =
        Digest::new([Felt::from(4u64), Felt::from(3u64), Felt::from(2u64), Felt::from(1u64)]);

    let block_inputs_from_store: BlockInputs = {
        let block_header = mock_block_header(Felt::ZERO, None, None, &[]);
        let chain_peaks = MmrPeaks::new(0, Vec::new()).unwrap();

        let account_states = vec![
            AccountInputRecord {
                account_id: account_id_1,
                account_hash: account_1_hash_store,
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
                account_id_1,
                account_1_hash_batches,
                Digest::default(),
                Vec::new(),
            ));

            Arc::new(TransactionBatch::new(vec![tx]))
        };

        let batch_2 = {
            let tx = Arc::new(tx_gen.dummy_proven_tx_with_params(
                account_id_2,
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
        Err(BuildBlockError::InconsistentAccountStates(vec![account_id_1]))
    );
}

/// Tests that the `BlockProver` computes the proper account root.
///
/// We assume an initial store with 5 accounts, and all will be updated.
#[test]
fn test_compute_account_root_success() {
    let tx_gen = DummyProvenTxGenerator::new();

    // Set up account states
    // ---------------------------------------------------------------------------------------------
    let account_ids = vec![
        unsafe { AccountId::new_unchecked(Felt::from(0b0000_0000_0000_0000u64)) },
        unsafe { AccountId::new_unchecked(Felt::from(0b1111_0000_0000_0000u64)) },
        unsafe { AccountId::new_unchecked(Felt::from(0b1111_1111_0000_0000u64)) },
        unsafe { AccountId::new_unchecked(Felt::from(0b1111_1111_1111_0000u64)) },
        unsafe { AccountId::new_unchecked(Felt::from(0b1111_1111_1111_1111u64)) },
    ];

    let account_initial_states = vec![
        [Felt::from(1u64), Felt::from(1u64), Felt::from(1u64), Felt::from(1u64)],
        [Felt::from(2u64), Felt::from(2u64), Felt::from(2u64), Felt::from(2u64)],
        [Felt::from(3u64), Felt::from(3u64), Felt::from(3u64), Felt::from(3u64)],
        [Felt::from(4u64), Felt::from(4u64), Felt::from(4u64), Felt::from(4u64)],
        [Felt::from(5u64), Felt::from(5u64), Felt::from(5u64), Felt::from(5u64)],
    ];

    let account_final_states = vec![
        [Felt::from(2u64), Felt::from(2u64), Felt::from(2u64), Felt::from(2u64)],
        [Felt::from(3u64), Felt::from(3u64), Felt::from(3u64), Felt::from(3u64)],
        [Felt::from(4u64), Felt::from(4u64), Felt::from(4u64), Felt::from(4u64)],
        [Felt::from(5u64), Felt::from(5u64), Felt::from(5u64), Felt::from(5u64)],
        [Felt::from(1u64), Felt::from(1u64), Felt::from(1u64), Felt::from(1u64)],
    ];

    // Set up store's account SMT
    // ---------------------------------------------------------------------------------------------

    // store SMT is initialized with all the accounts and their initial state
    let mut store_smt = SimpleSmt::with_leaves(
        64,
        account_ids
            .iter()
            .zip(account_initial_states.iter())
            .map(|(&account_id, &account_hash)| (account_id.into(), account_hash)),
    )
    .unwrap();

    // Block prover
    // ---------------------------------------------------------------------------------------------

    // Block inputs is initialized with all the accounts and their initial state
    let block_inputs_from_store: BlockInputs = {
        let block_header = mock_block_header(Felt::ZERO, None, None, &[]);
        let chain_peaks = MmrPeaks::new(0, Vec::new()).unwrap();

        let account_states = account_ids
            .iter()
            .zip(account_initial_states.iter())
            .map(|(&account_id, account_hash)| AccountInputRecord {
                account_id,
                account_hash: Digest::from(account_hash),
                proof: store_smt.get_leaf_path(account_id.into()).unwrap(),
            })
            .collect();

        BlockInputs {
            block_header,
            chain_peaks,
            account_states,
            nullifiers: Vec::new(),
        }
    };

    let batches: Vec<SharedTxBatch> = {
        let txs: Vec<_> = account_ids
            .iter()
            .enumerate()
            .map(|(idx, &account_id)| {
                Arc::new(tx_gen.dummy_proven_tx_with_params(
                    account_id,
                    account_initial_states[idx].into(),
                    account_final_states[idx].into(),
                    Vec::new(),
                ))
            })
            .collect();

        let batch_1 = Arc::new(TransactionBatch::new(txs[..2].to_vec()));
        let batch_2 = Arc::new(TransactionBatch::new(txs[2..].to_vec()));

        vec![batch_1, batch_2]
    };

    let block_witness = BlockWitness::new(block_inputs_from_store, batches).unwrap();

    let block_prover = BlockProver::new();
    let block_header = block_prover.prove(block_witness).unwrap();

    // Update SMT by hand to get new root
    // ---------------------------------------------------------------------------------------------
    for (idx, &account_id) in account_ids.iter().enumerate() {
        store_smt.update_leaf(account_id.into(), account_final_states[idx]).unwrap();
    }

    // Compare roots
    // ---------------------------------------------------------------------------------------------
    assert_eq!(block_header.account_root(), store_smt.root());
}
