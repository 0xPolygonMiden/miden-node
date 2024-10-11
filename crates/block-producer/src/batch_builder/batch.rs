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

    limits: BatchLimits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountUpdate {
    pub init_state: Digest,
    pub final_state: Digest,
    pub transactions: Vec<TransactionId>,
    pub details: AccountUpdateDetails,
}

#[derive(Debug, Clone)]
struct BatchLimits {
    input_notes: usize,
    output_notes: usize,
    account_updates: usize,
}

impl Default for BatchLimits {
    fn default() -> Self {
        Self {
            input_notes: MAX_INPUT_NOTES_PER_BATCH,
            output_notes: MAX_OUTPUT_NOTES_PER_BATCH,
            account_updates: MAX_ACCOUNTS_PER_BATCH,
        }
    }
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
    #[cfg(test)]
    fn with_limits(limits: BatchLimits) -> Self {
        Self { limits, ..Default::default() }
    }

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
            limits: _,
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
            self.limits.account_updates,
        )?;
        BuildBatchErrorRework::check_input_note_limit(
            self.input_notes.len(),
            self.limits.input_notes,
        )?;
        BuildBatchErrorRework::check_output_note_limit(
            self.output_notes.len(),
            self.limits.output_notes,
        )?;

        Ok(())
    }
}

// TESTS
// ================================================================================================

#[cfg(test)]
mod tests {
    use miden_node_proto::domain::blocks::BlockInclusionProof;
    use miden_objects::{
        notes::NoteInclusionProof,
        transaction::{InputNote, ProvenTransactionBuilder, ToInputNoteCommitments},
        BlockHeader,
    };
    use miden_processor::crypto::MerklePath;

    use super::*;
    use crate::test_utils::{
        mock_proven_tx,
        note::{mock_note, mock_output_note, mock_unauthenticated_note_commitment},
        MockPrivateAccount, MockProvenTxBuilder,
    };

    #[test]
    fn account_updates_are_merged() {
        // Create a private account with 3 random states.
        let MockPrivateAccount { id, states } = MockPrivateAccount::<3>::from(10);

        let tx0 = MockProvenTxBuilder::with_account(id, states[0], states[1]).build();
        let tx1 = MockProvenTxBuilder::with_account(id, states[1], states[2]).build();

        let mut expected = AccountUpdate::new(tx0.id(), tx0.account_update());
        expected.merge_tx(tx1.id(), tx1.account_update()).unwrap();

        let tx0 = VerifiedTransaction::new_unchecked(tx0);
        let tx1 = VerifiedTransaction::new_unchecked(tx1);

        let mut uut = TransactionBatch::builder();
        uut.push_transaction(tx0).unwrap();
        uut.push_transaction(tx1).unwrap();
        let batch = uut.build().unwrap();

        let expected = [(id, expected)].into();

        assert_eq!(batch.updated_accounts, expected);
    }

    #[test]
    fn notes_are_propagated() {
        let input_notes = vec![mock_note(1), mock_note(2), mock_note(3)];
        let output_notes = vec![mock_output_note(4), mock_output_note(5), mock_output_note(6)];

        let tx0 = mock_proven_tx(0x12, input_notes[..2].to_vec(), output_notes[..1].to_vec());
        let tx1 = mock_proven_tx(0xAB, input_notes[2..].to_vec(), output_notes[1..].to_vec());

        let tx0 = VerifiedTransaction::new_unchecked(tx0);
        let tx1 = VerifiedTransaction::new_unchecked(tx1);

        let mut uut = TransactionBatch::builder();
        uut.push_transaction(tx0).unwrap();
        uut.push_transaction(tx1).unwrap();

        let batch = uut.build().unwrap();

        let expected: BTreeMap<_, _> = input_notes
            .into_iter()
            .map(InputNote::unauthenticated)
            .map(|note| (note.nullifier(), note.into()))
            .collect();
        let batch_input_notes: BTreeMap<_, _> =
            batch.input_notes.into_iter().map(|note| (note.nullifier(), note)).collect();
        assert_eq!(batch_input_notes, expected);

        let expected: BTreeMap<_, _> =
            output_notes.into_iter().map(|note| (note.id(), note)).collect();
        let batch_output_notes: BTreeMap<_, _> =
            batch.output_notes.into_iter().map(|note| (note.id(), note)).collect();
        assert_eq!(batch_output_notes, expected);
    }

    #[test]
    fn duplicate_output_notes_are_rejected() {
        let output_note = mock_output_note(123);

        let tx0 = mock_proven_tx(0x12, vec![], vec![output_note.clone()]);
        let tx1 = mock_proven_tx(0xAB, vec![], vec![output_note.clone()]);

        let tx0 = VerifiedTransaction::new_unchecked(tx0);
        let tx1 = VerifiedTransaction::new_unchecked(tx1);

        let mut uut = TransactionBatch::builder();
        uut.push_transaction(tx0).unwrap();

        let err = uut.push_transaction(tx1).unwrap_err();
        let expected = BuildBatchErrorRework::DuplicateOutputNote(output_note.id());

        assert_eq!(err, expected);
    }

    #[test]
    fn duplicate_nullifiers_are_rejected() {
        let input_note = mock_note(222);

        let tx0 = mock_proven_tx(0x12, vec![input_note.clone()], vec![]);
        let tx1 = mock_proven_tx(0xAB, vec![input_note.clone()], vec![]);

        let tx0 = VerifiedTransaction::new_unchecked(tx0);
        let tx1 = VerifiedTransaction::new_unchecked(tx1);

        let mut uut = TransactionBatch::builder();
        uut.push_transaction(tx0).unwrap();

        let err = uut.push_transaction(tx1).unwrap_err();
        let expected = BuildBatchErrorRework::DuplicateNullifiers([input_note.nullifier()].into());

        assert_eq!(err, expected);
    }

    #[test]
    fn ephemeral_notes_are_removed() {
        let output_note = mock_output_note(123);
        let input_note = mock_note(123);
        let input_note_nullifier = input_note.nullifier();
        assert_eq!(input_note.id(), output_note.id(), "Same seed should give the same note.");

        let tx0 = mock_proven_tx(0x12, vec![input_note], vec![]);
        let tx1 = mock_proven_tx(0xAB, vec![], vec![output_note]);

        let tx0 = VerifiedTransaction::new_unchecked(tx0);
        let tx1 = VerifiedTransaction::new_unchecked(tx1);

        let mut uut = TransactionBatchBuilder::default();
        uut.push_transaction(tx0).unwrap();
        uut.push_transaction(tx1).unwrap();

        let batch = uut.build().unwrap();

        assert!(batch.input_notes().is_empty());
        assert!(batch.output_notes().is_empty());

        let empty_smt = BatchNoteTree::with_contiguous_leaves([]).unwrap();
        assert_eq!(batch.output_notes_smt, empty_smt);
    }

    #[test]
    fn witnessed_notes_are_upgraded_to_authenticated() {
        let unauthenticated_input_note = mock_note(123);
        let input_note_nullifier = unauthenticated_input_note.nullifier();

        // Create a witness for the input note. Most of the data is random
        // but it suffices since the batch producer assumes its correct.
        // This may change in the future once we have a batch proof kernel.
        const BLOCK_NUM: u32 = 0xABC;
        let note_proofs = [(
            unauthenticated_input_note.id(),
            NoteInclusionProof::new(BLOCK_NUM, 0, MerklePath::default()).unwrap(),
        )]
        .into();
        let block_proofs = [(
            BLOCK_NUM,
            BlockInclusionProof {
                block_header: BlockHeader::new(
                    0,
                    [1u32, 0, 0, 0].into(),
                    BLOCK_NUM,
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
                chain_length: BLOCK_NUM + 10,
            },
        )]
        .into();

        let tx0 = mock_proven_tx(0x12, vec![unauthenticated_input_note], vec![]);

        let note_witness = NoteAuthenticationInfo { note_proofs, block_proofs };
        let batch = TransactionBatch::new(vec![tx0], note_witness).unwrap();

        // Authenticated notes only have a nullifier.
        let expected = vec![InputNoteCommitment::from(input_note_nullifier)];

        assert_eq!(batch.input_notes, expected);
    }

    #[test]
    fn input_note_limit_is_respected() {
        let limit = 1;
        let limits = BatchLimits { input_notes: limit, ..Default::default() };

        let input_notes = vec![mock_note(123), mock_note(45)];
        let actual = input_notes.len();

        let tx0 = mock_proven_tx(0x12, input_notes, vec![]);
        let tx0 = VerifiedTransaction::new_unchecked(tx0);

        let mut uut = TransactionBatchBuilder::with_limits(limits);
        uut.push_transaction(tx0).unwrap();

        let err = uut.build().unwrap_err();
        let expected = BuildBatchErrorRework::InputeNoteLimitExceeded { actual, limit };

        assert_eq!(err, expected);
    }

    #[test]
    fn output_note_limit_is_respected() {
        let limit = 1;
        let limits = BatchLimits {
            output_notes: limit,
            ..Default::default()
        };

        let output_notes = vec![mock_output_note(123), mock_output_note(45)];
        let actual = output_notes.len();

        let tx0 = mock_proven_tx(0x12, vec![], output_notes);
        let tx0 = VerifiedTransaction::new_unchecked(tx0);

        let mut uut = TransactionBatchBuilder::with_limits(limits);
        uut.push_transaction(tx0).unwrap();

        let err = uut.build().unwrap_err();
        let expected = BuildBatchErrorRework::OutputNoteLimitExceeded { actual, limit };

        assert_eq!(err, expected);
    }

    #[test]
    fn account_update_limit_is_respected() {
        let limit = 1;
        let limits = BatchLimits {
            account_updates: limit,
            ..Default::default()
        };

        let tx0 = mock_proven_tx(0x12, vec![], vec![]);
        let tx0 = VerifiedTransaction::new_unchecked(tx0);

        let tx1 = mock_proven_tx(0xAB, vec![], vec![]);
        let tx1 = VerifiedTransaction::new_unchecked(tx1);

        let mut uut = TransactionBatchBuilder::with_limits(limits.clone());
        uut.push_transaction(tx0).unwrap();
        uut.push_transaction(tx1).unwrap();

        let err = uut.build().unwrap_err();
        let expected = BuildBatchErrorRework::AccountLimitExceeded { actual: 2, limit };

        assert_eq!(err, expected);
    }
}
