use std::collections::BTreeSet;

use miden_objects::{
    accounts::AccountId,
    notes::{NoteId, Nullifier},
    transaction::{ProvenTransaction, TransactionId, TxAccountUpdate},
    Digest,
};

use crate::{errors::VerifyTxError, store::TransactionInputs};

/// A transaction whose proof has been verified.
#[derive(Clone, PartialEq)]
pub struct VerifiedTransaction(ProvenTransaction);

/// A transaction who's proof has been verified, and which has been authenticated against the store.
#[derive(Clone, PartialEq)]
pub struct AuthenticatedTransaction {
    inner: VerifiedTransaction,
    store_account_state: Option<Digest>,
    authenticated_notes: BTreeSet<NoteId>,
    authentication_height: u32,
}

impl AuthenticatedTransaction {
    pub fn id(&self) -> TransactionId {
        self.inner.0.id()
    }

    pub fn account_id(&self) -> AccountId {
        self.inner.0.account_id()
    }

    pub fn account_update(&self) -> &TxAccountUpdate {
        self.inner.0.account_update()
    }

    pub fn store_account_state(&self) -> Option<Digest> {
        self.store_account_state
    }

    pub fn authentication_height(&self) -> u32 {
        self.authentication_height
    }

    pub fn nullifiers(&self) -> impl Iterator<Item = Nullifier> + '_ {
        self.inner.0.get_nullifiers()
    }

    pub fn output_notes(&self) -> impl Iterator<Item = NoteId> + '_ {
        self.inner.0.output_notes().iter().map(|note| note.id())
    }

    pub fn unauthenticated_notes(&self) -> impl Iterator<Item = NoteId> + '_ {
        self.inner
            .0
            .get_unauthenticated_notes()
            .cloned()
            .map(|header| header.id())
            .filter(|note_id| !self.authenticated_notes.contains(note_id))
    }

    pub fn into_raw(self) -> ProvenTransaction {
        self.inner.0
    }
}

#[cfg(test)]
impl AuthenticatedTransaction {
    pub fn from_inner(inner: ProvenTransaction) -> Self {
        let store_account_state = match inner.account_update().init_state_hash() {
            zero if zero == Digest::default() => None,
            non_zero => Some(non_zero),
        };
        Self {
            inner: VerifiedTransaction::new_unchecked(inner),
            store_account_state,
            authenticated_notes: Default::default(),
            authentication_height: Default::default(),
        }
    }

    pub fn with_store_state(mut self, state: Digest) -> Self {
        self.store_account_state = Some(state);
        self
    }

    pub fn with_empty_store_state(mut self) -> Self {
        self.store_account_state = None;
        self
    }
}

impl VerifiedTransaction {
    /// Creates a new verified transaction without actually verifying the proof.
    ///
    /// The caller assumes reponsibility for ensuring the proof was actually valid.
    pub fn new_unchecked(tx: ProvenTransaction) -> Self {
        Self(tx)
    }

    /// Validates the transaction against the inputs from the store.
    pub fn validate_inputs(
        self,
        inputs: TransactionInputs,
    ) -> Result<AuthenticatedTransaction, VerifyTxError> {
        let nullifiers_already_spent = self
            .0
            .get_nullifiers()
            .filter(|nullifier| inputs.nullifiers.get(nullifier).cloned().flatten().is_some())
            .collect::<Vec<_>>();
        if !nullifiers_already_spent.is_empty() {
            return Err(VerifyTxError::InputNotesAlreadyConsumed(nullifiers_already_spent));
        }

        // Invert the missing notes; i.e. we now know the rest were actually found.
        let authenticated_notes = self
            .0
            .get_unauthenticated_notes()
            .map(|header| header.id())
            .filter(|note_id| !inputs.missing_unauthenticated_notes.contains(note_id))
            .collect();

        Ok(AuthenticatedTransaction {
            inner: self,
            authenticated_notes,
            authentication_height: inputs.current_block_height,
            store_account_state: inputs.account_hash,
        })
    }
}
