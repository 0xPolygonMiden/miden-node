use std::sync::Arc;

use miden_air::FieldElement;
use miden_mock::mock::block::mock_block_header;
use miden_node_proto::domain::AccountInputRecord;
use miden_objects::{
    crypto::merkle::MmrPeaks,
    notes::{NoteEnvelope, NoteMetadata},
};
use miden_vm::crypto::SimpleSmt;

use crate::{
    batch_builder::TransactionBatch,
    store::Store,
    test_utils::{DummyProvenTxGenerator, MockStoreSuccess},
};

use super::*;

// BLOCK WITNESS TESTS
// =================================================================================================

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
                Vec::new(),
            ));

            Arc::new(TransactionBatch::new(vec![tx]).unwrap())
        };

        let batch_2 = {
            let tx = Arc::new(tx_gen.dummy_proven_tx_with_params(
                account_id_3,
                Digest::default(),
                Digest::default(),
                Vec::new(),
                Vec::new(),
            ));

            Arc::new(TransactionBatch::new(vec![tx]).unwrap())
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
                Vec::new(),
            ));

            Arc::new(TransactionBatch::new(vec![tx]).unwrap())
        };

        let batch_2 = {
            let tx = Arc::new(tx_gen.dummy_proven_tx_with_params(
                account_id_2,
                Digest::default(),
                Digest::default(),
                Vec::new(),
                Vec::new(),
            ));

            Arc::new(TransactionBatch::new(vec![tx]).unwrap())
        };

        vec![batch_1, batch_2]
    };

    let block_witness_result = BlockWitness::new(block_inputs_from_store, batches);

    assert_eq!(
        block_witness_result,
        Err(BuildBlockError::InconsistentAccountStates(vec![account_id_1]))
    );
}

// ACCOUNT ROOT TESTS
// =================================================================================================

/// Tests that the `BlockProver` computes the proper account root.
///
/// We assume an initial store with 5 accounts, and all will be updated.
#[tokio::test]
async fn test_compute_account_root_success() {
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
                    Vec::new(),
                ))
            })
            .collect();

        let batch_1 = Arc::new(TransactionBatch::new(txs[..2].to_vec()).unwrap());
        let batch_2 = Arc::new(TransactionBatch::new(txs[2..].to_vec()).unwrap());

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

// NOTE ROOT TESTS
// =================================================================================================

/// Tests that the block kernel returns the empty tree (depth 20) if no notes were created, and
/// contains no batches
#[tokio::test]
async fn test_compute_note_root_empty_batches_success() {
    // Set up store
    // ---------------------------------------------------------------------------------------------

    let store = MockStoreSuccess::new(std::iter::empty(), BTreeSet::new());

    // Block prover
    // ---------------------------------------------------------------------------------------------

    // Block inputs is initialized with all the accounts and their initial state
    let block_inputs_from_store: BlockInputs =
        store.get_block_inputs(std::iter::empty(), std::iter::empty()).await.unwrap();

    let batches: Vec<SharedTxBatch> = Vec::new();

    let block_witness = BlockWitness::new(block_inputs_from_store, batches).unwrap();

    let block_prover = BlockProver::new();
    let block_header = block_prover.prove(block_witness).unwrap();

    // Compare roots
    // ---------------------------------------------------------------------------------------------
    let created_notes_empty_root = EmptySubtreeRoots::entry(CREATED_NOTES_TREE_DEPTH, 0);
    assert_eq!(block_header.note_root(), *created_notes_empty_root);
}

/// Tests that the block kernel returns the empty tree (depth 20) if no notes were created, but
/// which contains at least 1 batch.
#[tokio::test]
async fn test_compute_note_root_empty_notes_success() {
    // Set up store
    // ---------------------------------------------------------------------------------------------

    let store = MockStoreSuccess::new(std::iter::empty(), BTreeSet::new());

    // Block prover
    // ---------------------------------------------------------------------------------------------

    // Block inputs is initialized with all the accounts and their initial state
    let block_inputs_from_store: BlockInputs =
        store.get_block_inputs(std::iter::empty(), std::iter::empty()).await.unwrap();

    let batches: Vec<SharedTxBatch> = {
        let batch = Arc::new(TransactionBatch::new(Vec::new()).unwrap());
        vec![batch]
    };

    let block_witness = BlockWitness::new(block_inputs_from_store, batches).unwrap();

    let block_prover = BlockProver::new();
    let block_header = block_prover.prove(block_witness).unwrap();

    // Compare roots
    // ---------------------------------------------------------------------------------------------
    let created_notes_empty_root = EmptySubtreeRoots::entry(CREATED_NOTES_TREE_DEPTH, 0);
    assert_eq!(block_header.note_root(), *created_notes_empty_root);
}

/// Tests that the block kernel returns the expected tree when multiple notes were created across
/// many batches.
#[tokio::test]
async fn test_compute_note_root_success() {
    let tx_gen = DummyProvenTxGenerator::new();

    let account_ids = vec![
        unsafe { AccountId::new_unchecked(Felt::from(0u64)) },
        unsafe { AccountId::new_unchecked(Felt::from(1u64)) },
        unsafe { AccountId::new_unchecked(Felt::from(2u64)) },
    ];

    let notes_created: Vec<NoteEnvelope> = vec![
        Digest::from([Felt::from(1u64), Felt::from(1u64), Felt::from(1u64), Felt::from(1u64)]),
        Digest::from([Felt::from(2u64), Felt::from(2u64), Felt::from(2u64), Felt::from(2u64)]),
        Digest::from([Felt::from(3u64), Felt::from(3u64), Felt::from(3u64), Felt::from(3u64)]),
    ]
    .into_iter()
    .zip(account_ids.iter())
    .map(|(note_digest, &account_id)| {
        NoteEnvelope::new(
            note_digest,
            NoteMetadata::new(account_id, Felt::from(1u64), Felt::from(0u64)),
        )
    })
    .collect();

    // Set up store
    // ---------------------------------------------------------------------------------------------

    let store = MockStoreSuccess::new(std::iter::empty(), BTreeSet::new());

    // Block prover
    // ---------------------------------------------------------------------------------------------

    // Block inputs is initialized with all the accounts and their initial state
    let block_inputs_from_store: BlockInputs =
        store.get_block_inputs(account_ids.iter(), std::iter::empty()).await.unwrap();

    let batches: Vec<SharedTxBatch> = {
        let txs: Vec<_> = notes_created
            .iter()
            .zip(account_ids.iter())
            .map(|(note, &account_id)| {
                Arc::new(tx_gen.dummy_proven_tx_with_params(
                    account_id,
                    Digest::default(),
                    Digest::default(),
                    Vec::new(),
                    vec![note.clone()],
                ))
            })
            .collect();

        let batch_1 = Arc::new(TransactionBatch::new(txs[..2].to_vec()).unwrap());
        let batch_2 = Arc::new(TransactionBatch::new(txs[2..].to_vec()).unwrap());

        vec![batch_1, batch_2]
    };

    let block_witness = BlockWitness::new(block_inputs_from_store, batches).unwrap();

    let block_prover = BlockProver::new();
    let block_header = block_prover.prove(block_witness).unwrap();

    // Create SMT by hand to get new root
    // ---------------------------------------------------------------------------------------------

    // The current logic is hardcoded to a depth of 20
    assert_eq!(CREATED_NOTES_TREE_DEPTH, 20);

    // The first 2 txs were put in the first batch; the 3rd was put in the second. It will lie in
    // the second subtree of depth 12
    let notes_smt = SimpleSmt::with_leaves(
        CREATED_NOTES_TREE_DEPTH,
        vec![
            (0u64, notes_created[0].note_hash().into()),
            (1u64, notes_created[1].note_hash().into()),
            (2u64.pow(12), notes_created[2].note_hash().into()),
        ],
    )
    .unwrap();

    // Compare roots
    // ---------------------------------------------------------------------------------------------
    assert_eq!(block_header.note_root(), notes_smt.root());
}
