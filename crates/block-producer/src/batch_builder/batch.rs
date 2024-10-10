use std::{
    collections::{btree_map::Entry, BTreeMap, BTreeSet},
    mem,
    sync::Arc,
};

use miden_node_proto::domain::notes::NoteAuthenticationInfo;
use miden_objects::{
    accounts::{delta::AccountUpdateDetails, AccountId},
    batches::BatchNoteTree,
    crypto::hash::blake::{Blake3Digest, Blake3_256},
    notes::{NoteHeader, NoteId, Nullifier},
    transaction::{InputNoteCommitment, OutputNote, TransactionId, TxAccountUpdate},
    AccountDeltaError, Digest, MAX_ACCOUNTS_PER_BATCH, MAX_INPUT_NOTES_PER_BATCH,
    MAX_OUTPUT_NOTES_PER_BATCH,
};
use tracing::instrument;

use crate::{
    errors::{BuildBatchError, BuildBatchErrorRework},
    transaction::{InputNotes, VerifiedTransaction},
    ProvenTransaction,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct BatchId(Blake3Digest<32>);

impl BatchId {
    pub fn compute(tx_ids: impl Iterator<Item = TransactionId>) -> Self {
        let upper_bound = tx_ids.size_hint().1.unwrap_or_default();
        let mut buf = Vec::with_capacity(32 * upper_bound);
        for id in tx_ids {
            buf.extend_from_slice(&id.as_bytes());
        }
        Self(Blake3_256::hash(&buf))
    }

    pub fn inner(&self) -> &Blake3Digest<32> {
        &self.0
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

#[derive(Debug, Clone, Default)]
pub struct TransactionBatchBuilder {
    updated_accounts: BTreeMap<AccountId, AccountUpdate>,
    input_notes: InputNotes,
    output_notes: BTreeMap<NoteId, OutputNote>,

    /// Transactions that form part of this batch.
    transactions: Vec<TransactionId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountUpdate {
    pub init_state: Digest,
    pub final_state: Digest,
    pub transactions: Vec<TransactionId>,
    pub details: AccountUpdateDetails,
}

impl AccountUpdate {
    fn new(tx_id: TransactionId, update: &TxAccountUpdate) -> Self {
        Self {
            init_state: update.init_state_hash(),
            final_state: update.final_state_hash(),
            transactions: vec![tx_id],
            details: update.details().clone(),
        }
    }

    /// Merges the transaction's update into this account update.
    fn merge_tx(
        &mut self,
        tx_id: TransactionId,
        update: &TxAccountUpdate,
    ) -> Result<(), AccountDeltaError> {
        assert!(
            self.final_state == update.init_state_hash(),
            "Transacion's initial state does not match current account state"
        );

        self.final_state = update.final_state_hash();
        self.transactions.push(tx_id);
        self.details = self.details.clone().merge(update.details().clone())?;

        Ok(())
    }
}

impl TransactionBatch {
    // CONSTRUCTORS
    // --------------------------------------------------------------------------------------------

    /// Returns a new [TransactionBatch] instantiated from the provided vector of proven
    /// transactions. If a map of unauthenticated notes found in the store is provided, it is used
    /// for transforming unauthenticated notes into authenticated notes.
    ///
    /// # Errors
    /// Returns an error if:
    /// - The number of output notes across all transactions exceeds 4096.
    /// - There are duplicated output notes or unauthenticated notes found across all transactions
    ///   in the batch.
    /// - Hashes for corresponding input notes and output notes don't match.
    ///
    /// TODO: enforce limit on the number of created nullifiers.
    #[instrument(target = "miden-block-producer", name = "new_batch", skip_all, err)]
    pub fn new(
        txs: Vec<ProvenTransaction>,
        found_unauthenticated_notes: NoteAuthenticationInfo,
    ) -> Result<Self, BuildBatchError> {
        let mut batch = TransactionBatchBuilder::default();

        for (idx, tx) in txs.iter().cloned().enumerate() {
            let tx = VerifiedTransaction::new_unchecked(tx);

            if let Err(err) = batch.push_transaction(tx) {
                return Err(err.into_old(txs));
            }
        }

        batch.witness_notes(found_unauthenticated_notes);

        batch.build().map_err(|err| err.into_old(txs))
    }

    // PUBLIC ACCESSORS
    // --------------------------------------------------------------------------------------------

    /// Returns the batch ID.
    pub fn id(&self) -> BatchId {
        self.id
    }

    /// Returns an iterator over (account_id, init_state_hash) tuples for accounts that were
    /// modified in this transaction batch.
    #[cfg(test)]
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

    pub fn builder() -> TransactionBatchBuilder {
        TransactionBatchBuilder::default()
    }
}

impl TransactionBatchBuilder {
    pub fn push_transaction(
        &mut self,
        tx: VerifiedTransaction,
    ) -> Result<(), BuildBatchErrorRework> {
        self.update_account(&tx)?;
        self.merge_input_notes(&tx)?;
        self.merge_output_notes(&tx)?;

        Ok(())
    }

    pub fn witness_notes(&mut self, witnesses: NoteAuthenticationInfo) {
        for (note_id, (block_witness, note_witness)) in witnesses.note_proofs() {
            if !self.input_notes.witness_note(note_id, block_witness, note_witness) {
                tracing::warn!(note=%note_id, "Received a witness for a note that was not unauthenticated.");
            }
        }
    }

    pub fn build(mut self) -> Result<TransactionBatch, BuildBatchErrorRework> {
        // Remove ephemeral notes prior to asserting batch constraints.
        let ephemeral = self.remove_ephemeral_notes();
        self.check_limits()?;

        let Self {
            updated_accounts,
            input_notes,
            output_notes,
            transactions,
        } = self;

        let id = BatchId::compute(transactions.into_iter());

        // Build the output notes SMT.
        let output_notes = output_notes.into_values().collect::<Vec<_>>();
        let output_notes_smt = BatchNoteTree::with_contiguous_leaves(
            output_notes.iter().map(|note| (note.id(), note.metadata())),
        )
        .expect("Duplicate output notes aren't possible by construction");

        let input_notes = input_notes.into_input_note_commitments().collect();

        Ok(TransactionBatch {
            id,
            updated_accounts,
            input_notes,
            output_notes,
            output_notes_smt,
        })
    }

    fn update_account(&mut self, tx: &VerifiedTransaction) -> Result<(), BuildBatchErrorRework> {
        let tx_id = tx.id();
        let account_update = tx.account_update();

        match self.updated_accounts.entry(account_update.account_id()) {
            Entry::Vacant(vacant) => {
                vacant.insert(AccountUpdate::new(tx_id, account_update));
                Ok(())
            },
            Entry::Occupied(occupied) => occupied
                .into_mut()
                .merge_tx(tx_id, account_update)
                .map_err(|error| BuildBatchErrorRework::AccountUpdateError {
                    account_id: account_update.account_id(),
                    error,
                }),
        }
    }

    fn merge_input_notes(&mut self, tx: &VerifiedTransaction) -> Result<(), BuildBatchErrorRework> {
        self.input_notes
            .merge(tx.input_notes().clone())
            .map_err(BuildBatchErrorRework::DuplicateNullifiers)
    }

    fn merge_output_notes(
        &mut self,
        tx: &VerifiedTransaction,
    ) -> Result<(), BuildBatchErrorRework> {
        for (id, note) in tx.output_notes().clone() {
            if self.output_notes.insert(id, note).is_some() {
                return Err(BuildBatchErrorRework::DuplicateOutputNote(id));
            }
        }

        Ok(())
    }

    /// Removes all ephemeral notes within the batch.
    ///
    /// These are notes which are both produced and consumed within this batch.
    ///
    /// Their nullifiers are retained.
    fn remove_ephemeral_notes(&mut self) -> BTreeSet<NoteId> {
        let mut ephemeral = BTreeSet::new();
        for note_id in self.output_notes.keys() {
            // We can ignore proven and witnessed input notes. These are known to be outputs of
            // committed blocks and therefore cannot be outputs of this batch.
            if self.input_notes.remove_unauthenticated(note_id).is_some() {
                ephemeral.insert(*note_id);
            }
        }
        for note in &ephemeral {
            self.output_notes.remove(note);
        }
        ephemeral
    }

    /// Returns an error if any of the batch length limits are violated.
    ///
    /// More specifically, it checks that the number of account updates, input and
    /// output notes fall within the batch limits.
    fn check_limits(&self) -> Result<(), BuildBatchErrorRework> {
        BuildBatchErrorRework::check_account_limit(
            self.updated_accounts.len(),
            MAX_ACCOUNTS_PER_BATCH,
        )?;
        BuildBatchErrorRework::check_input_note_limit(
            self.input_notes.len(),
            MAX_INPUT_NOTES_PER_BATCH,
        )?;
        BuildBatchErrorRework::check_output_note_limit(
            self.output_notes.len(),
            MAX_OUTPUT_NOTES_PER_BATCH,
        )?;

        Ok(())
    }
}

#[derive(Debug)]
struct OutputNoteTracker {
    output_notes: Vec<Option<OutputNote>>,
    output_note_index: BTreeMap<NoteId, usize>,
}

impl OutputNoteTracker {
    fn new(txs: &[ProvenTransaction]) -> Result<Self, BuildBatchError> {
        let mut output_notes = vec![];
        let mut output_note_index = BTreeMap::new();
        for tx in txs {
            for note in tx.output_notes().iter() {
                if output_note_index.insert(note.id(), output_notes.len()).is_some() {
                    return Err(BuildBatchError::DuplicateOutputNote(note.id(), txs.to_vec()));
                }
                output_notes.push(Some(note.clone()));
            }
        }

        Ok(Self { output_notes, output_note_index })
    }

    pub fn remove_note(
        &mut self,
        input_note_header: &NoteHeader,
        txs: &[ProvenTransaction],
    ) -> Result<bool, BuildBatchError> {
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
                        txs: txs.to_vec(),
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
    use miden_node_proto::domain::blocks::BlockInclusionProof;
    use miden_objects::{notes::NoteInclusionProof, BlockHeader};
    use miden_processor::crypto::MerklePath;

    use super::*;
    use crate::test_utils::{
        mock_proven_tx,
        note::{mock_note, mock_output_note, mock_unauthenticated_note_commitment},
    };

    #[test]
    fn test_output_note_tracker_duplicate_output_notes() {
        let mut txs = mock_proven_txs();

        let result = OutputNoteTracker::new(&txs);
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

        match OutputNoteTracker::new(&txs) {
            Err(BuildBatchError::DuplicateOutputNote(note_id, _)) => {
                assert_eq!(note_id, duplicate_output_note.id())
            },
            res => panic!("Unexpected result: {res:?}"),
        }
    }

    #[test]
    fn test_output_note_tracker_remove_in_place_consumed_note() {
        let txs = mock_proven_txs();
        let mut tracker = OutputNoteTracker::new(&txs).unwrap();

        let note_to_remove = mock_note(4);

        assert!(tracker.remove_note(note_to_remove.header(), &txs).unwrap());
        assert!(!tracker.remove_note(note_to_remove.header(), &txs).unwrap());

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
    fn test_duplicate_unauthenticated_notes() {
        let mut txs = mock_proven_txs();
        let duplicate_note = mock_note(5);
        txs.push(mock_proven_tx(4, vec![duplicate_note.clone()], vec![mock_output_note(9)]));
        match TransactionBatch::new(txs, Default::default()) {
            Err(BuildBatchError::DuplicateNullifiers(nullifiers, _)) => {
                assert_eq!(nullifiers, [duplicate_note.nullifier()].into())
            },
            res => panic!("Unexpected result: {res:?}"),
        }
    }

    #[test]
    fn test_consume_notes_in_place() {
        let mut txs = mock_proven_txs();
        let note_to_consume = mock_note(3);
        txs.push(mock_proven_tx(
            3,
            vec![mock_note(11), note_to_consume, mock_note(13)],
            vec![mock_output_note(9), mock_output_note(10)],
        ));

        let mut batch = TransactionBatch::new(txs, Default::default()).unwrap();

        // One of the unauthenticated notes must be removed from the batch due to the consumption
        // of the corresponding output note
        let mut expected_input_notes = vec![
            mock_unauthenticated_note_commitment(1),
            mock_unauthenticated_note_commitment(5),
            mock_unauthenticated_note_commitment(11),
            mock_unauthenticated_note_commitment(13),
        ];
        expected_input_notes.sort_unstable_by_key(|note| note.nullifier());
        batch.input_notes.sort_unstable_by_key(|note| note.nullifier());

        assert_eq!(batch.input_notes, expected_input_notes);

        // One of the output notes must be removed from the batch due to the consumption
        // by the corresponding unauthenticated note
        let mut expected_output_notes = vec![
            mock_output_note(2),
            mock_output_note(4),
            mock_output_note(6),
            mock_output_note(7),
            mock_output_note(8),
            mock_output_note(9),
            mock_output_note(10),
        ];
        expected_output_notes.sort_unstable_by_key(|note| note.id());
        batch.output_notes.sort_unstable_by_key(|note| note.id());

        assert_eq!(batch.output_notes, expected_output_notes);

        // Ensure all nullifiers match the corresponding input notes' nullifiers
        let expected_nullifiers: Vec<_> =
            batch.input_notes().iter().map(InputNoteCommitment::nullifier).collect();
        let actual_nullifiers: Vec<_> = batch.produced_nullifiers().collect();
        assert_eq!(actual_nullifiers, expected_nullifiers);
    }

    #[test]
    fn test_convert_unauthenticated_note_to_authenticated() {
        let txs = mock_proven_txs();
        let note_proofs =
            [(mock_note(5).id(), NoteInclusionProof::new(0, 0, MerklePath::default()).unwrap())]
                .into();
        let block_proofs = [(
            0,
            BlockInclusionProof {
                block_header: BlockHeader::new(
                    0,
                    [1u32, 0, 0, 0].into(),
                    0,
                    [2u32, 0, 0, 0].into(),
                    [3u32, 0, 0, 0].into(),
                    [4u32, 0, 0, 0].into(),
                    [5u32, 0, 0, 0].into(),
                    [6u32, 0, 0, 0].into(),
                    [7u32, 0, 0, 0].into(),
                    [8u32, 0, 0, 0].into(),
                    123,
                ),
                mmr_path: Default::default(),
                chain_length: 10,
            },
        )]
        .into();
        let found_unauthenticated_notes = NoteAuthenticationInfo { note_proofs, block_proofs };
        let batch = TransactionBatch::new(txs, found_unauthenticated_notes).unwrap();

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
