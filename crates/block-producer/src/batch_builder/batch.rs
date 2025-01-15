use std::{
    borrow::Borrow,
    collections::{btree_map::Entry, BTreeMap, BTreeSet},
    mem,
};

use miden_node_proto::domain::notes::NoteAuthenticationInfo;
use miden_node_utils::formatting::format_blake3_digest;
use miden_objects::{
    accounts::{delta::AccountUpdateDetails, AccountId},
    batches::BatchNoteTree,
    crypto::hash::blake::{Blake3Digest, Blake3_256},
    notes::{NoteHeader, NoteId, Nullifier},
    transaction::{InputNoteCommitment, OutputNote, ProvenTransaction, TransactionId},
    AccountDeltaError, Digest,
};
use tracing::instrument;

use crate::{errors::BuildBatchError, COMPONENT};

// BATCH ID
// ================================================================================================

/// Uniquely identifies a [TransactionBatch].
#[derive(Debug, Copy, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct BatchId(Blake3Digest<32>);

impl BatchId {
    /// Calculates a batch ID from the given set of transactions.
    pub fn compute<T>(txs: impl Iterator<Item = T>) -> Self
    where
        T: Borrow<TransactionId>,
    {
        let mut buf = Vec::with_capacity(32 * txs.size_hint().0);
        for tx in txs {
            buf.extend_from_slice(&tx.borrow().as_bytes());
        }
        Self(Blake3_256::hash(&buf))
    }
}

impl std::fmt::Display for BatchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format_blake3_digest(self.0))
    }
}

// ACCOUNT UPDATE
// ================================================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountUpdate {
    pub init_state: Digest,
    pub final_state: Digest,
    pub transactions: Vec<TransactionId>,
    pub details: AccountUpdateDetails,
}

impl AccountUpdate {
    fn new(tx: &ProvenTransaction) -> Self {
        Self {
            init_state: tx.account_update().init_state_hash(),
            final_state: tx.account_update().final_state_hash(),
            transactions: vec![tx.id()],
            details: tx.account_update().details().clone(),
        }
    }

    /// Merges the transaction's update into this account update.
    fn merge_tx(&mut self, tx: &ProvenTransaction) -> Result<(), AccountDeltaError> {
        assert!(
            self.final_state == tx.account_update().init_state_hash(),
            "Transacion's initial state does not match current account state"
        );

        self.final_state = tx.account_update().final_state_hash();
        self.transactions.push(tx.id());
        self.details = self.details.clone().merge(tx.account_update().details().clone())?;

        Ok(())
    }
}

// TRANSACTION BATCH
// ================================================================================================

/// A batch of transactions that share a common proof.
///
/// Note: Until recursive proofs are available in the Miden VM, we don't include the common proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionBatch {
    id: BatchId,
    updated_accounts: BTreeMap<AccountId, AccountUpdate>,
    input_notes: Vec<InputNoteCommitment>,
    output_notes_smt: BatchNoteTree,
    output_notes: Vec<OutputNote>,
}

impl TransactionBatch {
    // CONSTRUCTORS
    // --------------------------------------------------------------------------------------------

    /// Returns a new [TransactionBatch] built from the provided transactions. If a map of
    /// unauthenticated notes found in the store is provided, it is used for transforming
    /// unauthenticated notes into authenticated notes.
    ///
    /// The tx input takes an `IntoIterator` of a reference, which effectively allows for cheap
    /// cloning of the iterator. Or put differently, we want something similar to `impl
    /// Iterator<Item = ProvenTransaction> + Clone` which this provides.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - There are duplicated output notes or unauthenticated notes found across all transactions
    ///   in the batch.
    /// - Hashes for corresponding input notes and output notes don't match.
    #[instrument(target = COMPONENT, name = "new_batch", skip_all, err)]
    pub fn new<'a, I>(
        txs: impl IntoIterator<Item = &'a ProvenTransaction, IntoIter = I>,
        found_unauthenticated_notes: NoteAuthenticationInfo,
    ) -> Result<Self, BuildBatchError>
    where
        I: Iterator<Item = &'a ProvenTransaction> + Clone,
    {
        let tx_iter = txs.into_iter();
        let id = BatchId::compute(tx_iter.clone().map(ProvenTransaction::id));

        // Populate batch output notes and updated accounts.
        let mut output_notes = OutputNoteTracker::new(tx_iter.clone())?;
        let mut updated_accounts = BTreeMap::<AccountId, AccountUpdate>::new();
        let mut unauthenticated_input_notes = BTreeSet::new();
        for tx in tx_iter.clone() {
            // Merge account updates so that state transitions A->B->C become A->C.
            match updated_accounts.entry(tx.account_id()) {
                Entry::Vacant(vacant) => {
                    vacant.insert(AccountUpdate::new(tx));
                },
                Entry::Occupied(occupied) => {
                    occupied.into_mut().merge_tx(tx).map_err(|source| {
                        BuildBatchError::AccountUpdateError { account_id: tx.account_id(), source }
                    })?
                },
            };

            // Check unauthenticated input notes for duplicates:
            for note in tx.get_unauthenticated_notes() {
                let id = note.id();
                if !unauthenticated_input_notes.insert(id) {
                    return Err(BuildBatchError::DuplicateUnauthenticatedNote(id));
                }
            }
        }

        // Populate batch produced nullifiers and match output notes with corresponding
        // unauthenticated input notes in the same batch, which are removed from the unauthenticated
        // input notes set.
        //
        // One thing to note:
        // This still allows transaction `A` to consume an unauthenticated note `x` and output note
        // `y` and for transaction `B` to consume an unauthenticated note `y` and output
        // note `x` (i.e., have a circular dependency between transactions), but this is not
        // a problem.
        let mut input_notes = vec![];
        for tx in tx_iter {
            for input_note in tx.input_notes().iter() {
                // Header is presented only for unauthenticated input notes.
                let input_note = match input_note.header() {
                    Some(input_note_header) => {
                        if output_notes.remove_note(input_note_header)? {
                            continue;
                        }

                        // If an unauthenticated note was found in the store, transform it to an
                        // authenticated one (i.e. erase additional note details
                        // except the nullifier)
                        if found_unauthenticated_notes.contains_note(&input_note_header.id()) {
                            InputNoteCommitment::from(input_note.nullifier())
                        } else {
                            input_note.clone()
                        }
                    },
                    None => input_note.clone(),
                };
                input_notes.push(input_note)
            }
        }

        let output_notes = output_notes.into_notes();

        // Build the output notes SMT.
        let output_notes_smt = BatchNoteTree::with_contiguous_leaves(
            output_notes.iter().map(|note| (note.id(), note.metadata())),
        )
        .expect("Unreachable: fails only if the output note list contains duplicates");

        Ok(Self {
            id,
            updated_accounts,
            input_notes,
            output_notes_smt,
            output_notes,
        })
    }

    // PUBLIC ACCESSORS
    // --------------------------------------------------------------------------------------------

    /// Returns the batch ID.
    pub fn id(&self) -> BatchId {
        self.id
    }

    /// Returns an iterator over (account_id, init_state_hash) tuples for accounts that were
    /// modified in this transaction batch.
    pub fn account_initial_states(&self) -> impl Iterator<Item = (AccountId, Digest)> + '_ {
        self.updated_accounts
            .iter()
            .map(|(&account_id, update)| (account_id, update.init_state))
    }

    /// Returns an iterator over (account_id, details, new_state_hash) tuples for accounts that were
    /// modified in this transaction batch.
    pub fn updated_accounts(&self) -> impl Iterator<Item = (&AccountId, &AccountUpdate)> + '_ {
        self.updated_accounts.iter()
    }

    /// Returns input notes list consumed by the transactions in this batch. Any unauthenticated
    /// input notes which have matching output notes within this batch are not included in this
    /// list.
    pub fn input_notes(&self) -> &[InputNoteCommitment] {
        &self.input_notes
    }

    /// Returns an iterator over produced nullifiers for all consumed notes.
    pub fn produced_nullifiers(&self) -> impl Iterator<Item = Nullifier> + '_ {
        self.input_notes.iter().map(InputNoteCommitment::nullifier)
    }

    /// Returns the root hash of the output notes SMT.
    pub fn output_notes_root(&self) -> Digest {
        self.output_notes_smt.root()
    }

    /// Returns output notes list.
    pub fn output_notes(&self) -> &Vec<OutputNote> {
        &self.output_notes
    }
}

#[derive(Debug)]
struct OutputNoteTracker {
    output_notes: Vec<Option<OutputNote>>,
    output_note_index: BTreeMap<NoteId, usize>,
}

impl OutputNoteTracker {
    fn new<'a>(txs: impl Iterator<Item = &'a ProvenTransaction>) -> Result<Self, BuildBatchError> {
        let mut output_notes = vec![];
        let mut output_note_index = BTreeMap::new();
        for tx in txs {
            for note in tx.output_notes().iter() {
                if output_note_index.insert(note.id(), output_notes.len()).is_some() {
                    return Err(BuildBatchError::DuplicateOutputNote(note.id()));
                }
                output_notes.push(Some(note.clone()));
            }
        }

        Ok(Self { output_notes, output_note_index })
    }

    pub fn remove_note(&mut self, input_note_header: &NoteHeader) -> Result<bool, BuildBatchError> {
        let id = input_note_header.id();
        if let Some(note_index) = self.output_note_index.remove(&id) {
            if let Some(output_note) = mem::take(&mut self.output_notes[note_index]) {
                let input_hash = input_note_header.hash();
                let output_hash = output_note.hash();
                if output_hash != input_hash {
                    return Err(BuildBatchError::NoteHashesMismatch {
                        id,
                        input_hash,
                        output_hash,
                    });
                }

                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn into_notes(self) -> Vec<OutputNote> {
        self.output_notes.into_iter().flatten().collect()
    }
}

// TESTS
// ================================================================================================

#[cfg(test)]
mod tests {
    use miden_objects::notes::NoteInclusionProof;
    use miden_processor::crypto::MerklePath;

    use super::*;
    use crate::test_utils::{
        mock_proven_tx,
        note::{mock_note, mock_output_note, mock_unauthenticated_note_commitment},
    };

    #[test]
    fn output_note_tracker_duplicate_output_notes() {
        let mut txs = mock_proven_txs();

        let result = OutputNoteTracker::new(txs.iter());
        assert!(
            result.is_ok(),
            "Creation of output note tracker was not expected to fail: {result:?}"
        );

        let duplicate_output_note = txs[1].output_notes().get_note(1).clone();

        txs.push(mock_proven_tx(
            3,
            vec![],
            vec![duplicate_output_note.clone(), mock_output_note(8), mock_output_note(4)],
        ));

        match OutputNoteTracker::new(txs.iter()) {
            Err(BuildBatchError::DuplicateOutputNote(note_id)) => {
                assert_eq!(note_id, duplicate_output_note.id())
            },
            res => panic!("Unexpected result: {res:?}"),
        }
    }

    #[test]
    fn output_note_tracker_remove_in_place_consumed_note() {
        let txs = mock_proven_txs();
        let mut tracker = OutputNoteTracker::new(txs.iter()).unwrap();

        let note_to_remove = mock_note(4);

        assert!(tracker.remove_note(note_to_remove.header()).unwrap());
        assert!(!tracker.remove_note(note_to_remove.header()).unwrap());

        // Check that output notes are in the expected order and consumed note was removed
        assert_eq!(
            tracker.into_notes(),
            vec![
                mock_output_note(2),
                mock_output_note(3),
                mock_output_note(6),
                mock_output_note(7),
                mock_output_note(8),
            ]
        );
    }

    #[test]
    fn duplicate_unauthenticated_notes() {
        let mut txs = mock_proven_txs();
        let duplicate_note = mock_note(5);
        txs.push(mock_proven_tx(4, vec![duplicate_note.clone()], vec![mock_output_note(9)]));
        match TransactionBatch::new(&txs, Default::default()) {
            Err(BuildBatchError::DuplicateUnauthenticatedNote(note_id)) => {
                assert_eq!(note_id, duplicate_note.id())
            },
            res => panic!("Unexpected result: {res:?}"),
        }
    }

    #[test]
    fn consume_notes_in_place() {
        let mut txs = mock_proven_txs();
        let note_to_consume = mock_note(3);
        txs.push(mock_proven_tx(
            3,
            vec![mock_note(11), note_to_consume, mock_note(13)],
            vec![mock_output_note(9), mock_output_note(10)],
        ));

        let batch = TransactionBatch::new(&txs, Default::default()).unwrap();

        // One of the unauthenticated notes must be removed from the batch due to the consumption
        // of the corresponding output note
        let expected_input_notes = vec![
            mock_unauthenticated_note_commitment(1),
            mock_unauthenticated_note_commitment(5),
            mock_unauthenticated_note_commitment(11),
            mock_unauthenticated_note_commitment(13),
        ];
        assert_eq!(batch.input_notes, expected_input_notes);

        // One of the output notes must be removed from the batch due to the consumption
        // by the corresponding unauthenticated note
        let expected_output_notes = vec![
            mock_output_note(2),
            mock_output_note(4),
            mock_output_note(6),
            mock_output_note(7),
            mock_output_note(8),
            mock_output_note(9),
            mock_output_note(10),
        ];
        assert_eq!(batch.output_notes.len(), expected_output_notes.len());
        assert_eq!(batch.output_notes, expected_output_notes);

        // Ensure all nullifiers match the corresponding input notes' nullifiers
        let expected_nullifiers: Vec<_> =
            batch.input_notes().iter().map(InputNoteCommitment::nullifier).collect();
        let actual_nullifiers: Vec<_> = batch.produced_nullifiers().collect();
        assert_eq!(actual_nullifiers, expected_nullifiers);
    }

    #[test]
    fn convert_unauthenticated_note_to_authenticated() {
        let txs = mock_proven_txs();
        let found_unauthenticated_notes = BTreeMap::from_iter([(
            mock_note(5).id(),
            NoteInclusionProof::new(0, 0, MerklePath::default()).unwrap(),
        )]);
        let found_unauthenticated_notes = NoteAuthenticationInfo {
            note_proofs: found_unauthenticated_notes,
            block_proofs: Default::default(),
        };
        let batch = TransactionBatch::new(&txs, found_unauthenticated_notes).unwrap();

        let expected_input_notes =
            vec![mock_unauthenticated_note_commitment(1), mock_note(5).nullifier().into()];
        assert_eq!(batch.input_notes, expected_input_notes);
    }

    // UTILITIES
    // =============================================================================================

    fn mock_proven_txs() -> Vec<ProvenTransaction> {
        vec![
            mock_proven_tx(
                1,
                vec![mock_note(1)],
                vec![mock_output_note(2), mock_output_note(3), mock_output_note(4)],
            ),
            mock_proven_tx(
                2,
                vec![mock_note(5)],
                vec![mock_output_note(6), mock_output_note(7), mock_output_note(8)],
            ),
        ]
    }
}
