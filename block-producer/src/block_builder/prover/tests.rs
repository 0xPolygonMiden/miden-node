use std::sync::Arc;

use miden_air::FieldElement;
use miden_mock::mock::block::mock_block_header;
use miden_node_proto::domain::AccountInputRecord;
use miden_objects::crypto::merkle::MmrPeaks;

use crate::{
    batch_builder::TransactionBatch,
    store::Store,
    test_utils::{DummyProvenTxGenerator, MockStoreSuccess},
};

use super::*;

/// Tests that `BlockWitness` constructor fails if the store and transaction batches contain a
/// different set of account ids.
///
/// The store will contain accounts 1 & 2, while the transaction batches will contain 2 & 3.
#[test]
fn test_block_witness_validation_inconsistent_account_ids() {
    let tx_gen = DummyProvenTxGenerator::new();
    let account_id_1 = AccountId::new_unchecked(Felt::ZERO);
    let account_id_2 = AccountId::new_unchecked(Felt::ONE);
    let account_id_3 = AccountId::new_unchecked(Felt::new(42));

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
    let account_id_1 = AccountId::new_unchecked(Felt::ZERO);
    let account_id_2 = AccountId::new_unchecked(Felt::ONE);

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
#[tokio::test]
async fn test_compute_account_root_success() {
    let tx_gen = DummyProvenTxGenerator::new();

    // Set up account states
    // ---------------------------------------------------------------------------------------------
    let account_ids = vec![
        AccountId::new_unchecked(Felt::from(0b0000_0000_0000_0000u64)),
        AccountId::new_unchecked(Felt::from(0b1111_0000_0000_0000u64)),
        AccountId::new_unchecked(Felt::from(0b1111_1111_0000_0000u64)),
        AccountId::new_unchecked(Felt::from(0b1111_1111_1111_0000u64)),
        AccountId::new_unchecked(Felt::from(0b1111_1111_1111_1111u64)),
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

    let store = MockStoreSuccess::new(
        account_ids
            .iter()
            .zip(account_initial_states.iter())
            .map(|(&account_id, &account_hash)| (account_id.into(), account_hash.into())),
        BTreeSet::new(),
    );
    // Block prover
    // ---------------------------------------------------------------------------------------------

    // Block inputs is initialized with all the accounts and their initial state
    let block_inputs_from_store: BlockInputs =
        store.get_block_inputs(account_ids.iter(), std::iter::empty()).await.unwrap();

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
    store
        .update_accounts(
            account_ids
                .iter()
                .zip(account_final_states.iter())
                .map(|(&account_id, &account_hash)| (account_id.into(), account_hash.into())),
        )
        .await;

    // Compare roots
    // ---------------------------------------------------------------------------------------------
    assert_eq!(block_header.account_root(), store.account_root().await);
}

/// Test that the current account root is returned if the batches are empty
#[tokio::test]
async fn test_compute_account_root_empty_batches() {
    // Set up account states
    // ---------------------------------------------------------------------------------------------
    let account_ids = vec![
        AccountId::new_unchecked(Felt::from(0b0000_0000_0000_0000u64)),
        AccountId::new_unchecked(Felt::from(0b1111_0000_0000_0000u64)),
        AccountId::new_unchecked(Felt::from(0b1111_1111_0000_0000u64)),
        AccountId::new_unchecked(Felt::from(0b1111_1111_1111_0000u64)),
        AccountId::new_unchecked(Felt::from(0b1111_1111_1111_1111u64)),
    ];

    let account_initial_states = vec![
        [Felt::from(1u64), Felt::from(1u64), Felt::from(1u64), Felt::from(1u64)],
        [Felt::from(2u64), Felt::from(2u64), Felt::from(2u64), Felt::from(2u64)],
        [Felt::from(3u64), Felt::from(3u64), Felt::from(3u64), Felt::from(3u64)],
        [Felt::from(4u64), Felt::from(4u64), Felt::from(4u64), Felt::from(4u64)],
        [Felt::from(5u64), Felt::from(5u64), Felt::from(5u64), Felt::from(5u64)],
    ];

    // Set up store's account SMT
    // ---------------------------------------------------------------------------------------------

    let store = MockStoreSuccess::new(
        account_ids
            .iter()
            .zip(account_initial_states.iter())
            .map(|(&account_id, &account_hash)| (account_id.into(), account_hash.into())),
        BTreeSet::new(),
    );
    // Block prover
    // ---------------------------------------------------------------------------------------------

    // Block inputs is initialized with all the accounts and their initial state
    let block_inputs_from_store: BlockInputs =
        store.get_block_inputs(std::iter::empty(), std::iter::empty()).await.unwrap();

    let batches = Vec::new();
    let block_witness = BlockWitness::new(block_inputs_from_store, batches).unwrap();

    let block_prover = BlockProver::new();
    let block_header = block_prover.prove(block_witness).unwrap();

    // Compare roots
    // ---------------------------------------------------------------------------------------------
    assert_eq!(block_header.account_root(), store.account_root().await);
}
