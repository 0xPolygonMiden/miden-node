use std::{collections::BTreeMap, iter};

use assert_matches::assert_matches;
use miden_objects::{
    accounts::{
        account_id::testing::{
            ACCOUNT_ID_OFF_CHAIN_SENDER, ACCOUNT_ID_REGULAR_ACCOUNT_UPDATABLE_CODE_OFF_CHAIN,
        },
        delta::AccountUpdateDetails,
        AccountId,
    },
    block::{BlockAccountUpdate, BlockNoteIndex, BlockNoteTree},
    crypto::merkle::{
        EmptySubtreeRoots, LeafIndex, MerklePath, Mmr, MmrPeaks, Smt, SmtLeaf, SmtProof, SMT_DEPTH,
    },
    notes::{NoteExecutionHint, NoteHeader, NoteMetadata, NoteTag, NoteType, Nullifier},
    transaction::{OutputNote, ProvenTransaction},
    Felt, BATCH_NOTE_TREE_DEPTH, BLOCK_NOTE_TREE_DEPTH, ONE, ZERO,
};

use self::block_witness::AccountUpdateWitness;
use super::*;
use crate::{
    batch_builder::TransactionBatch,
    block::{AccountWitness, BlockInputs},
    test_utils::{
        block::{build_actual_block_header, build_expected_block_header, MockBlockBuilder},
        MockProvenTxBuilder, MockStoreSuccessBuilder,
    },
};

// BLOCK WITNESS TESTS
// =================================================================================================

/// Tests that `BlockWitness` constructor fails if the store and transaction batches contain a
/// different set of account ids.
///
/// The store will contain accounts 1 & 2, while the transaction batches will contain 2 & 3.
#[test]
fn block_witness_validation_inconsistent_account_ids() {
    let account_id_1 = AccountId::new_unchecked(Felt::new(ACCOUNT_ID_OFF_CHAIN_SENDER));
    let account_id_2 = AccountId::new_unchecked(Felt::new(ACCOUNT_ID_OFF_CHAIN_SENDER + 1));
    let account_id_3 = AccountId::new_unchecked(Felt::new(ACCOUNT_ID_OFF_CHAIN_SENDER + 2));

    let block_inputs_from_store: BlockInputs = {
        let block_header = BlockHeader::mock(0, None, None, &[], Default::default());
        let chain_peaks = MmrPeaks::new(0, Vec::new()).unwrap();

        let accounts = BTreeMap::from_iter(vec![
            (account_id_1, AccountWitness::default()),
            (account_id_2, AccountWitness::default()),
        ]);

        BlockInputs {
            block_header,
            chain_peaks,
            accounts,
            nullifiers: Default::default(),
            found_unauthenticated_notes: Default::default(),
        }
    };

    let batches: Vec<TransactionBatch> = {
        let batch_1 = {
            let tx = MockProvenTxBuilder::with_account(
                account_id_2,
                Digest::default(),
                Digest::default(),
            )
            .build();

            TransactionBatch::new([&tx], Default::default()).unwrap()
        };

        let batch_2 = {
            let tx = MockProvenTxBuilder::with_account(
                account_id_3,
                Digest::default(),
                Digest::default(),
            )
            .build();

            TransactionBatch::new([&tx], Default::default()).unwrap()
        };

        vec![batch_1, batch_2]
    };

    let block_witness_result = BlockWitness::new(block_inputs_from_store, &batches);

    assert!(block_witness_result.is_err());
}

/// Tests that `BlockWitness` constructor fails if the store and transaction batches contain a
/// different at least 1 account who's state hash is different.
///
/// Only account 1 will have a different state hash
#[test]
fn block_witness_validation_inconsistent_account_hashes() {
    let account_id_1 =
        AccountId::new_unchecked(Felt::new(ACCOUNT_ID_REGULAR_ACCOUNT_UPDATABLE_CODE_OFF_CHAIN));
    let account_id_2 = AccountId::new_unchecked(Felt::new(ACCOUNT_ID_OFF_CHAIN_SENDER));

    let account_1_hash_store =
        Digest::new([Felt::new(1u64), Felt::new(2u64), Felt::new(3u64), Felt::new(4u64)]);
    let account_1_hash_batches =
        Digest::new([Felt::new(4u64), Felt::new(3u64), Felt::new(2u64), Felt::new(1u64)]);

    let block_inputs_from_store: BlockInputs = {
        let block_header = BlockHeader::mock(0, None, None, &[], Default::default());
        let chain_peaks = MmrPeaks::new(0, Vec::new()).unwrap();

        let accounts = BTreeMap::from_iter(vec![
            (
                account_id_1,
                AccountWitness {
                    hash: account_1_hash_store,
                    proof: Default::default(),
                },
            ),
            (account_id_2, Default::default()),
        ]);

        BlockInputs {
            block_header,
            chain_peaks,
            accounts,
            nullifiers: Default::default(),
            found_unauthenticated_notes: Default::default(),
        }
    };

    let batches = {
        let batch_1 = TransactionBatch::new(
            [&MockProvenTxBuilder::with_account(
                account_id_1,
                account_1_hash_batches,
                Digest::default(),
            )
            .build()],
            Default::default(),
        )
        .unwrap();
        let batch_2 = TransactionBatch::new(
            [&MockProvenTxBuilder::with_account(
                account_id_2,
                Digest::default(),
                Digest::default(),
            )
            .build()],
            Default::default(),
        )
        .unwrap();

        vec![batch_1, batch_2]
    };

    let block_witness_result = BlockWitness::new(block_inputs_from_store, &batches);

    assert_matches!(
        block_witness_result,
        Err(BuildBlockError::InconsistentAccountStateTransition(
            account_id,
            account_hash_store,
            account_hash_batches
        )) => {
            assert_eq!(account_id, account_id_1);
            assert_eq!(account_hash_store, account_1_hash_store);
            assert_eq!(account_hash_batches, vec![account_1_hash_batches]);
        }
    );
}

/// Creates two batches which each update the same pair of accounts.
///
/// The transactions are ordered such that the batches cannot be chronologically ordered
/// themselves: `[tx_x0, tx_y1], [tx_y0, tx_x1]`. This test ensures that the witness is
/// produced correctly as if for a single batch: `[tx_x0, tx_x1, tx_y0, tx_y1]`.
#[test]
fn block_witness_multiple_batches_per_account() {
    let x_account_id =
        AccountId::new_unchecked(Felt::new(ACCOUNT_ID_REGULAR_ACCOUNT_UPDATABLE_CODE_OFF_CHAIN));
    let y_account_id = AccountId::new_unchecked(Felt::new(ACCOUNT_ID_OFF_CHAIN_SENDER));

    let x_hashes = [
        Digest::new((0..4).map(Felt::new).collect::<Vec<_>>().try_into().unwrap()),
        Digest::new((4..8).map(Felt::new).collect::<Vec<_>>().try_into().unwrap()),
        Digest::new((8..12).map(Felt::new).collect::<Vec<_>>().try_into().unwrap()),
    ];
    let y_hashes = [
        Digest::new((12..16).map(Felt::new).collect::<Vec<_>>().try_into().unwrap()),
        Digest::new((16..20).map(Felt::new).collect::<Vec<_>>().try_into().unwrap()),
        Digest::new((20..24).map(Felt::new).collect::<Vec<_>>().try_into().unwrap()),
    ];

    let x_txs = [
        MockProvenTxBuilder::with_account(x_account_id, x_hashes[0], x_hashes[1]).build(),
        MockProvenTxBuilder::with_account(x_account_id, x_hashes[1], x_hashes[2]).build(),
    ];
    let y_txs = [
        MockProvenTxBuilder::with_account(y_account_id, y_hashes[0], y_hashes[1]).build(),
        MockProvenTxBuilder::with_account(y_account_id, y_hashes[1], y_hashes[2]).build(),
    ];

    let x_proof = MerklePath::new(vec![Digest::new(
        (24..28).map(Felt::new).collect::<Vec<_>>().try_into().unwrap(),
    )]);
    let y_proof = MerklePath::new(vec![Digest::new(
        (28..32).map(Felt::new).collect::<Vec<_>>().try_into().unwrap(),
    )]);

    let block_inputs_from_store: BlockInputs = {
        let block_header = BlockHeader::mock(0, None, None, &[], Default::default());
        let chain_peaks = MmrPeaks::new(0, Vec::new()).unwrap();

        let x_witness = AccountWitness {
            hash: x_hashes[0],
            proof: x_proof.clone(),
        };
        let y_witness = AccountWitness {
            hash: y_hashes[0],
            proof: y_proof.clone(),
        };
        let accounts = BTreeMap::from_iter([(x_account_id, x_witness), (y_account_id, y_witness)]);

        BlockInputs {
            block_header,
            chain_peaks,
            accounts,
            nullifiers: Default::default(),
            found_unauthenticated_notes: Default::default(),
        }
    };

    let batches = {
        let batch_1 = TransactionBatch::new([&x_txs[0], &y_txs[1]], Default::default()).unwrap();
        let batch_2 = TransactionBatch::new([&y_txs[0], &x_txs[1]], Default::default()).unwrap();

        vec![batch_1, batch_2]
    };

    let (block_witness, _) = BlockWitness::new(block_inputs_from_store, &batches).unwrap();
    let account_witnesses = block_witness.updated_accounts.into_iter().collect::<BTreeMap<_, _>>();

    let x_expected = AccountUpdateWitness {
        initial_state_hash: x_hashes[0],
        final_state_hash: *x_hashes.last().unwrap(),
        proof: x_proof,
        transactions: x_txs.iter().map(ProvenTransaction::id).collect(),
    };

    let y_expected = AccountUpdateWitness {
        initial_state_hash: y_hashes[0],
        final_state_hash: *y_hashes.last().unwrap(),
        proof: y_proof,
        transactions: y_txs.iter().map(ProvenTransaction::id).collect(),
    };

    let expected = [(x_account_id, x_expected), (y_account_id, y_expected)].into();

    assert_eq!(account_witnesses, expected);
}

// ACCOUNT ROOT TESTS
// =================================================================================================

/// Tests that the `BlockProver` computes the proper account root.
///
/// We assume an initial store with 5 accounts, and all will be updated.
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn compute_account_root_success() {
    // Set up account states
    // ---------------------------------------------------------------------------------------------
    let account_ids = [
        AccountId::new_unchecked(Felt::new(ACCOUNT_ID_OFF_CHAIN_SENDER)),
        AccountId::new_unchecked(Felt::new(ACCOUNT_ID_OFF_CHAIN_SENDER + 1)),
        AccountId::new_unchecked(Felt::new(ACCOUNT_ID_OFF_CHAIN_SENDER + 2)),
        AccountId::new_unchecked(Felt::new(ACCOUNT_ID_OFF_CHAIN_SENDER + 3)),
        AccountId::new_unchecked(Felt::new(ACCOUNT_ID_OFF_CHAIN_SENDER + 4)),
    ];

    let account_initial_states = [
        [Felt::new(1u64), Felt::new(1u64), Felt::new(1u64), Felt::new(1u64)],
        [Felt::new(2u64), Felt::new(2u64), Felt::new(2u64), Felt::new(2u64)],
        [Felt::new(3u64), Felt::new(3u64), Felt::new(3u64), Felt::new(3u64)],
        [Felt::new(4u64), Felt::new(4u64), Felt::new(4u64), Felt::new(4u64)],
        [Felt::new(5u64), Felt::new(5u64), Felt::new(5u64), Felt::new(5u64)],
    ];

    let account_final_states = [
        [Felt::new(2u64), Felt::new(2u64), Felt::new(2u64), Felt::new(2u64)],
        [Felt::new(3u64), Felt::new(3u64), Felt::new(3u64), Felt::new(3u64)],
        [Felt::new(4u64), Felt::new(4u64), Felt::new(4u64), Felt::new(4u64)],
        [Felt::new(5u64), Felt::new(5u64), Felt::new(5u64), Felt::new(5u64)],
        [Felt::new(1u64), Felt::new(1u64), Felt::new(1u64), Felt::new(1u64)],
    ];

    // Set up store's account SMT
    // ---------------------------------------------------------------------------------------------

    let store = MockStoreSuccessBuilder::from_accounts(
        account_ids
            .iter()
            .zip(account_initial_states.iter())
            .map(|(&account_id, &account_hash)| (account_id, account_hash.into())),
    )
    .build();

    // Block prover
    // ---------------------------------------------------------------------------------------------

    // Block inputs is initialized with all the accounts and their initial state
    let block_inputs_from_store: BlockInputs = store
        .get_block_inputs(account_ids.into_iter(), std::iter::empty(), std::iter::empty())
        .await
        .unwrap();

    let batches: Vec<TransactionBatch> = {
        let txs: Vec<_> = account_ids
            .iter()
            .enumerate()
            .map(|(idx, &account_id)| {
                MockProvenTxBuilder::with_account(
                    account_id,
                    account_initial_states[idx].into(),
                    account_final_states[idx].into(),
                )
                .build()
            })
            .collect();

        let batch_1 = TransactionBatch::new(&txs[..2], Default::default()).unwrap();
        let batch_2 = TransactionBatch::new(&txs[2..], Default::default()).unwrap();

        vec![batch_1, batch_2]
    };

    let (block_witness, _) = BlockWitness::new(block_inputs_from_store, &batches).unwrap();

    let block_prover = BlockProver::new();
    let block_header = block_prover.prove(block_witness).unwrap();

    // Update SMT by hand to get new root
    // ---------------------------------------------------------------------------------------------
    let block = MockBlockBuilder::new(&store)
        .await
        .account_updates(
            account_ids
                .iter()
                .zip(account_final_states.iter())
                .map(|(&account_id, &account_hash)| {
                    BlockAccountUpdate::new(
                        account_id,
                        account_hash.into(),
                        AccountUpdateDetails::Private,
                        vec![],
                    )
                })
                .collect(),
        )
        .build();

    // Compare roots
    // ---------------------------------------------------------------------------------------------
    assert_eq!(block_header.account_root(), block.header().account_root());
}

/// Test that the current account root is returned if the batches are empty
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn compute_account_root_empty_batches() {
    // Set up account states
    // ---------------------------------------------------------------------------------------------
    let account_ids = [
        AccountId::new_unchecked(Felt::new(0b0000_0000_0000_0000u64)),
        AccountId::new_unchecked(Felt::new(0b1111_0000_0000_0000u64)),
        AccountId::new_unchecked(Felt::new(0b1111_1111_0000_0000u64)),
        AccountId::new_unchecked(Felt::new(0b1111_1111_1111_0000u64)),
        AccountId::new_unchecked(Felt::new(0b1111_1111_1111_1111u64)),
    ];

    let account_initial_states = [
        [Felt::new(1u64), Felt::new(1u64), Felt::new(1u64), Felt::new(1u64)],
        [Felt::new(2u64), Felt::new(2u64), Felt::new(2u64), Felt::new(2u64)],
        [Felt::new(3u64), Felt::new(3u64), Felt::new(3u64), Felt::new(3u64)],
        [Felt::new(4u64), Felt::new(4u64), Felt::new(4u64), Felt::new(4u64)],
        [Felt::new(5u64), Felt::new(5u64), Felt::new(5u64), Felt::new(5u64)],
    ];

    // Set up store's account SMT
    // ---------------------------------------------------------------------------------------------

    let store = MockStoreSuccessBuilder::from_accounts(
        account_ids
            .iter()
            .zip(account_initial_states.iter())
            .map(|(&account_id, &account_hash)| (account_id, account_hash.into())),
    )
    .build();

    // Block prover
    // ---------------------------------------------------------------------------------------------

    // Block inputs is initialized with all the accounts and their initial state
    let block_inputs_from_store: BlockInputs = store
        .get_block_inputs(std::iter::empty(), std::iter::empty(), std::iter::empty())
        .await
        .unwrap();

    let batches = Vec::new();
    let (block_witness, _) = BlockWitness::new(block_inputs_from_store, &batches).unwrap();

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
#[miden_node_test_macro::enable_logging]
async fn compute_note_root_empty_batches_success() {
    // Set up store
    // ---------------------------------------------------------------------------------------------

    let store = MockStoreSuccessBuilder::from_batches(iter::empty()).build();

    // Block prover
    // ---------------------------------------------------------------------------------------------

    // Block inputs is initialized with all the accounts and their initial state
    let block_inputs_from_store: BlockInputs = store
        .get_block_inputs(std::iter::empty(), std::iter::empty(), std::iter::empty())
        .await
        .unwrap();

    let batches: Vec<TransactionBatch> = Vec::new();

    let (block_witness, _) = BlockWitness::new(block_inputs_from_store, &batches).unwrap();

    let block_prover = BlockProver::new();
    let block_header = block_prover.prove(block_witness).unwrap();

    // Compare roots
    // ---------------------------------------------------------------------------------------------
    let created_notes_empty_root = EmptySubtreeRoots::entry(BLOCK_NOTE_TREE_DEPTH, 0);
    assert_eq!(block_header.note_root(), *created_notes_empty_root);
}

/// Tests that the block kernel returns the empty tree (depth 20) if no notes were created, but
/// which contains at least 1 batch.
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn compute_note_root_empty_notes_success() {
    // Set up store
    // ---------------------------------------------------------------------------------------------

    let store = MockStoreSuccessBuilder::from_batches(iter::empty()).build();

    // Block prover
    // ---------------------------------------------------------------------------------------------

    // Block inputs is initialized with all the accounts and their initial state
    let block_inputs_from_store: BlockInputs = store
        .get_block_inputs(std::iter::empty(), std::iter::empty(), std::iter::empty())
        .await
        .unwrap();

    let batches: Vec<TransactionBatch> = {
        let batch = TransactionBatch::new(vec![], Default::default()).unwrap();
        vec![batch]
    };

    let (block_witness, _) = BlockWitness::new(block_inputs_from_store, &batches).unwrap();

    let block_prover = BlockProver::new();
    let block_header = block_prover.prove(block_witness).unwrap();

    // Compare roots
    // ---------------------------------------------------------------------------------------------
    let created_notes_empty_root = EmptySubtreeRoots::entry(BLOCK_NOTE_TREE_DEPTH, 0);
    assert_eq!(block_header.note_root(), *created_notes_empty_root);
}

/// Tests that the block kernel returns the expected tree when multiple notes were created across
/// many batches.
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn compute_note_root_success() {
    let account_ids = [
        AccountId::new_unchecked(Felt::new(ACCOUNT_ID_OFF_CHAIN_SENDER)),
        AccountId::new_unchecked(Felt::new(ACCOUNT_ID_OFF_CHAIN_SENDER + 1)),
        AccountId::new_unchecked(Felt::new(ACCOUNT_ID_OFF_CHAIN_SENDER + 2)),
    ];

    let notes_created: Vec<NoteHeader> = [
        Digest::from([Felt::new(1u64), Felt::new(1u64), Felt::new(1u64), Felt::new(1u64)]),
        Digest::from([Felt::new(2u64), Felt::new(2u64), Felt::new(2u64), Felt::new(2u64)]),
        Digest::from([Felt::new(3u64), Felt::new(3u64), Felt::new(3u64), Felt::new(3u64)]),
    ]
    .into_iter()
    .zip(account_ids.iter())
    .map(|(note_digest, &account_id)| {
        NoteHeader::new(
            note_digest.into(),
            NoteMetadata::new(
                account_id,
                NoteType::Private,
                NoteTag::for_local_use_case(0u16, 0u16).unwrap(),
                NoteExecutionHint::none(),
                ONE,
            )
            .unwrap(),
        )
    })
    .collect();

    // Set up store
    // ---------------------------------------------------------------------------------------------

    let store = MockStoreSuccessBuilder::from_batches(iter::empty()).build();

    // Block prover
    // ---------------------------------------------------------------------------------------------

    // Block inputs is initialized with all the accounts and their initial state
    let block_inputs_from_store: BlockInputs = store
        .get_block_inputs(account_ids.into_iter(), std::iter::empty(), std::iter::empty())
        .await
        .unwrap();

    let batches: Vec<TransactionBatch> = {
        let txs: Vec<_> = notes_created
            .iter()
            .zip(account_ids.iter())
            .map(|(note, &account_id)| {
                let note = OutputNote::Header(*note);
                MockProvenTxBuilder::with_account(account_id, Digest::default(), Digest::default())
                    .output_notes(vec![note])
                    .build()
            })
            .collect();

        let batch_1 = TransactionBatch::new(&txs[..2], Default::default()).unwrap();
        let batch_2 = TransactionBatch::new(&txs[2..], Default::default()).unwrap();

        vec![batch_1, batch_2]
    };

    let (block_witness, _) = BlockWitness::new(block_inputs_from_store, &batches).unwrap();

    let block_prover = BlockProver::new();
    let block_header = block_prover.prove(block_witness).unwrap();

    // Create block note tree to get new root
    // ---------------------------------------------------------------------------------------------

    // The current logic is hardcoded to a depth of 6
    // Specifically, we assume the block has up to 2^6 batches, and each batch up to 2^10 created
    // notes, where each note is stored at depth 10 in the batch tree.
    const _: () = assert!(BLOCK_NOTE_TREE_DEPTH - BATCH_NOTE_TREE_DEPTH == 6);

    // The first 2 txs were put in the first batch; the 3rd was put in the second
    let note_tree = BlockNoteTree::with_entries([
        (
            BlockNoteIndex::new(0, 0).unwrap(),
            notes_created[0].id(),
            *notes_created[0].metadata(),
        ),
        (
            BlockNoteIndex::new(0, 1).unwrap(),
            notes_created[1].id(),
            *notes_created[1].metadata(),
        ),
        (
            BlockNoteIndex::new(1, 0).unwrap(),
            notes_created[2].id(),
            *notes_created[2].metadata(),
        ),
    ])
    .unwrap();

    // Compare roots
    // ---------------------------------------------------------------------------------------------
    assert_eq!(block_header.note_root(), note_tree.root());
}

// NULLIFIER ROOT TESTS
// =================================================================================================

/// Tests that `BlockWitness` constructor fails if the store and transaction batches contain a
/// different set of nullifiers.
///
/// The transaction batches will contain nullifiers 1 & 2, while the store will contain 2 & 3.
#[test]
fn block_witness_validation_inconsistent_nullifiers() {
    let batches: Vec<TransactionBatch> = {
        let batch_1 = {
            let tx = MockProvenTxBuilder::with_account_index(0).nullifiers_range(0..1).build();

            TransactionBatch::new([&tx], Default::default()).unwrap()
        };

        let batch_2 = {
            let tx = MockProvenTxBuilder::with_account_index(1).nullifiers_range(1..2).build();

            TransactionBatch::new([&tx], Default::default()).unwrap()
        };

        vec![batch_1, batch_2]
    };

    let nullifier_1 = batches[0].produced_nullifiers().next().unwrap();
    let nullifier_2 = batches[1].produced_nullifiers().next().unwrap();
    let nullifier_3 =
        Nullifier::from([101_u32.into(), 102_u32.into(), 103_u32.into(), 104_u32.into()]);

    let block_inputs_from_store: BlockInputs = {
        let block_header = BlockHeader::mock(0, None, None, &[], Default::default());
        let chain_peaks = MmrPeaks::new(0, Vec::new()).unwrap();

        let accounts = batches
            .iter()
            .flat_map(|batch| batch.account_initial_states())
            .map(|(account_id, hash)| {
                (account_id, AccountWitness { hash, proof: MerklePath::default() })
            })
            .collect();

        let nullifiers = BTreeMap::from_iter(vec![
            (
                nullifier_2,
                SmtProof::new(
                    MerklePath::new(vec![Digest::default(); SMT_DEPTH as usize]),
                    SmtLeaf::new_empty(LeafIndex::new_max_depth(
                        nullifier_2.most_significant_felt().into(),
                    )),
                )
                .unwrap(),
            ),
            (
                nullifier_3,
                SmtProof::new(
                    MerklePath::new(vec![Digest::default(); SMT_DEPTH as usize]),
                    SmtLeaf::new_empty(LeafIndex::new_max_depth(
                        nullifier_3.most_significant_felt().into(),
                    )),
                )
                .unwrap(),
            ),
        ]);

        BlockInputs {
            block_header,
            chain_peaks,
            accounts,
            nullifiers,
            found_unauthenticated_notes: Default::default(),
        }
    };

    let block_witness_result = BlockWitness::new(block_inputs_from_store, &batches);

    assert_matches!(
        block_witness_result,
        Err(BuildBlockError::InconsistentNullifiers(nullifiers)) => {
            assert_eq!(nullifiers, vec![nullifier_1, nullifier_3]);
        }
    );
}

/// Tests that the block kernel returns the expected nullifier tree when no nullifiers are present
/// in the transaction
#[tokio::test]
async fn compute_nullifier_root_empty_success() {
    let batches: Vec<TransactionBatch> = {
        let batch_1 = {
            let tx = MockProvenTxBuilder::with_account_index(0).build();

            TransactionBatch::new([&tx], Default::default()).unwrap()
        };

        let batch_2 = {
            let tx = MockProvenTxBuilder::with_account_index(1).build();

            TransactionBatch::new([&tx], Default::default()).unwrap()
        };

        vec![batch_1, batch_2]
    };

    let account_ids: Vec<AccountId> = batches
        .iter()
        .flat_map(|batch| batch.account_initial_states())
        .map(|(account_id, _)| account_id)
        .collect();

    // Set up store
    // ---------------------------------------------------------------------------------------------

    let store = MockStoreSuccessBuilder::from_batches(batches.iter()).build();

    // Block prover
    // ---------------------------------------------------------------------------------------------

    // Block inputs is initialized with all the accounts and their initial state
    let block_inputs_from_store: BlockInputs = store
        .get_block_inputs(account_ids.into_iter(), std::iter::empty(), std::iter::empty())
        .await
        .unwrap();

    let (block_witness, _) = BlockWitness::new(block_inputs_from_store, &batches).unwrap();

    let block_prover = BlockProver::new();
    let block_header = block_prover.prove(block_witness).unwrap();

    // Create SMT by hand to get new root
    // ---------------------------------------------------------------------------------------------
    let nullifier_smt = Smt::new();

    // Compare roots
    // ---------------------------------------------------------------------------------------------
    assert_eq!(block_header.nullifier_root(), nullifier_smt.root());
}

/// Tests that the block kernel returns the expected nullifier tree when multiple nullifiers are
/// present in the transaction
#[tokio::test]
async fn compute_nullifier_root_success() {
    let batches: Vec<TransactionBatch> = {
        let batch_1 = {
            let tx = MockProvenTxBuilder::with_account_index(0).nullifiers_range(0..1).build();

            TransactionBatch::new([&tx], Default::default()).unwrap()
        };

        let batch_2 = {
            let tx = MockProvenTxBuilder::with_account_index(1).nullifiers_range(1..2).build();

            TransactionBatch::new([&tx], Default::default()).unwrap()
        };

        vec![batch_1, batch_2]
    };

    let account_ids: Vec<AccountId> = batches
        .iter()
        .flat_map(|batch| batch.account_initial_states())
        .map(|(account_id, _)| account_id)
        .collect();

    let nullifiers = [
        batches[0].produced_nullifiers().next().unwrap(),
        batches[1].produced_nullifiers().next().unwrap(),
    ];

    // Set up store
    // ---------------------------------------------------------------------------------------------
    let initial_block_num = 42;

    let store = MockStoreSuccessBuilder::from_batches(batches.iter())
        .initial_block_num(initial_block_num)
        .build();

    // Block prover
    // ---------------------------------------------------------------------------------------------

    // Block inputs is initialized with all the accounts and their initial state
    let block_inputs_from_store: BlockInputs = store
        .get_block_inputs(account_ids.into_iter(), nullifiers.iter(), std::iter::empty())
        .await
        .unwrap();

    let (block_witness, _) = BlockWitness::new(block_inputs_from_store, &batches).unwrap();

    let block_prover = BlockProver::new();
    let block_header = block_prover.prove(block_witness).unwrap();

    // Create SMT by hand to get new root
    // ---------------------------------------------------------------------------------------------

    // Note that the block number in store is 42; the nullifiers get added to the next block (i.e.
    // block number 43)
    let nullifier_smt =
        Smt::with_entries(nullifiers.into_iter().map(|nullifier| {
            (nullifier.inner(), [(initial_block_num + 1).into(), ZERO, ZERO, ZERO])
        }))
        .unwrap();

    // Compare roots
    // ---------------------------------------------------------------------------------------------
    assert_eq!(block_header.nullifier_root(), nullifier_smt.root());
}

// CHAIN MMR ROOT TESTS
// =================================================================================================

/// Test that the chain mmr root is as expected if the batches are empty
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn compute_chain_mmr_root_empty_mmr() {
    let store = MockStoreSuccessBuilder::from_batches(iter::empty()).build();

    let expected_block_header = build_expected_block_header(&store, &[]).await;
    let actual_block_header = build_actual_block_header(&store, Vec::new()).await;

    assert_eq!(actual_block_header.chain_root(), expected_block_header.chain_root());
}

/// add header to non-empty MMR (1 peak), and check that we get the expected commitment
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn compute_chain_mmr_root_mmr_1_peak() {
    let initial_chain_mmr = {
        let mut mmr = Mmr::new();
        mmr.add(Digest::default());

        mmr
    };

    let store = MockStoreSuccessBuilder::from_batches(iter::empty())
        .initial_chain_mmr(initial_chain_mmr)
        .build();

    let expected_block_header = build_expected_block_header(&store, &[]).await;
    let actual_block_header = build_actual_block_header(&store, Vec::new()).await;

    assert_eq!(actual_block_header.chain_root(), expected_block_header.chain_root());
}

/// add header to an MMR with 17 peaks, and check that we get the expected commitment
#[tokio::test]
#[miden_node_test_macro::enable_logging]
async fn compute_chain_mmr_root_mmr_17_peaks() {
    let initial_chain_mmr = {
        let mut mmr = Mmr::new();
        for _ in 0..(2_u32.pow(17) - 1) {
            mmr.add(Digest::default());
        }

        assert_eq!(mmr.peaks().peaks().len(), 17);

        mmr
    };

    let store = MockStoreSuccessBuilder::from_batches(iter::empty())
        .initial_chain_mmr(initial_chain_mmr)
        .build();

    let expected_block_header = build_expected_block_header(&store, &[]).await;
    let actual_block_header = build_actual_block_header(&store, Vec::new()).await;

    assert_eq!(actual_block_header.chain_root(), expected_block_header.chain_root());
}
