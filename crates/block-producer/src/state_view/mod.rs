use std::{collections::BTreeSet, sync::Arc};

use async_trait::async_trait;
use miden_node_utils::formatting::format_array;
use miden_objects::{
    accounts::AccountId,
    block::Block,
    notes::{NoteId, Nullifier},
    transaction::OutputNote,
    Digest, MIN_PROOF_SECURITY_LEVEL,
};
use miden_tx::TransactionVerifier;
use tokio::sync::RwLock;
use tracing::{debug, instrument};

use crate::{
    errors::VerifyTxError,
    store::{ApplyBlock, ApplyBlockError, Store, TransactionInputs},
    txqueue::TransactionValidator,
    ProvenTransaction, COMPONENT,
};

#[cfg(test)]
mod tests;

pub struct DefaultStateView<S> {
    store: Arc<S>,

    /// Enables or disables the verification of transaction proofs in `verify_tx`
    verify_tx_proofs: bool,

    /// The account ID of accounts being modified by transactions currently in the block production
    /// pipeline. We currently ensure that only 1 tx/block modifies any given account (issue: #186).
    accounts_in_flight: Arc<RwLock<BTreeSet<AccountId>>>,

    /// The nullifiers of notes consumed by transactions currently in the block production pipeline.
    nullifiers_in_flight: Arc<RwLock<BTreeSet<Nullifier>>>,

    /// The output notes of transactions currently in the block production pipeline.
    notes_in_flight: Arc<RwLock<BTreeSet<NoteId>>>,
}

impl<S> DefaultStateView<S>
where
    S: Store,
{
    pub fn new(store: Arc<S>, verify_tx_proofs: bool) -> Self {
        Self {
            store,
            verify_tx_proofs,
            accounts_in_flight: Default::default(),
            nullifiers_in_flight: Default::default(),
            notes_in_flight: Default::default(),
        }
    }
}

// FIXME: remove the allow when the upstream clippy issue is fixed:
// https://github.com/rust-lang/rust-clippy/issues/12281
#[allow(clippy::blocks_in_conditions)]
#[async_trait]
impl<S> TransactionValidator for DefaultStateView<S>
where
    S: Store,
{
    #[instrument(skip_all, err)]
    async fn verify_tx(&self, candidate_tx: &ProvenTransaction) -> Result<(), VerifyTxError> {
        if self.verify_tx_proofs {
            // Make sure that the transaction proof is valid and meets the required security level
            let tx_verifier = TransactionVerifier::new(MIN_PROOF_SECURITY_LEVEL);
            tx_verifier
                .verify(candidate_tx.clone())
                .map_err(|_| VerifyTxError::InvalidTransactionProof(candidate_tx.id()))?;
        }

        // Soft-check if `tx` violates in-flight requirements.
        //
        // This is a "soft" check, because we'll need to redo it at the end. We do this soft check
        // to quickly reject clearly infracting transactions before hitting the store (slow).
        //
        // At this stage we don't provide missing notes, they will be available on the second check
        // after getting the transaction inputs.
        ensure_in_flight_constraints(
            candidate_tx,
            &*self.accounts_in_flight.read().await,
            &*self.nullifiers_in_flight.read().await,
            &*self.notes_in_flight.read().await,
            &[],
        )?;

        // Fetch the transaction inputs from the store, and check tx input constraints
        let tx_inputs = self.store.get_tx_inputs(candidate_tx).await?;
        let missing_notes = ensure_tx_inputs_constraints(candidate_tx, tx_inputs)?;

        // Re-check in-flight transaction constraints, and if verification passes, register
        // transaction
        //
        // Note: We need to re-check these constraints because we dropped the locks since we last
        // checked
        {
            let mut locked_accounts_in_flight = self.accounts_in_flight.write().await;
            let mut locked_nullifiers_in_flight = self.nullifiers_in_flight.write().await;
            let mut locked_notes_in_flight = self.notes_in_flight.write().await;

            ensure_in_flight_constraints(
                candidate_tx,
                &locked_accounts_in_flight,
                &locked_nullifiers_in_flight,
                &locked_notes_in_flight,
                &missing_notes,
            )?;

            // Success! Register transaction as successfully verified
            locked_accounts_in_flight.insert(candidate_tx.account_id());

            let mut nullifiers_in_tx: BTreeSet<_> =
                candidate_tx.input_notes().iter().map(|note| note.nullifier()).collect();
            locked_nullifiers_in_flight.append(&mut nullifiers_in_tx);

            let mut notes_in_tx: BTreeSet<_> =
                candidate_tx.output_notes().iter().map(OutputNote::id).collect();
            locked_notes_in_flight.append(&mut notes_in_tx);
        }

        Ok(())
    }
}

// FIXME: remove the allow when the upstream clippy issue is fixed:
// https://github.com/rust-lang/rust-clippy/issues/12281
#[allow(clippy::blocks_in_conditions)]
#[async_trait]
impl<S> ApplyBlock for DefaultStateView<S>
where
    S: Store,
{
    #[instrument(target = "miden-block-producer", skip_all, err)]
    async fn apply_block(&self, block: &Block) -> Result<(), ApplyBlockError> {
        self.store.apply_block(block).await?;

        let mut locked_accounts_in_flight = self.accounts_in_flight.write().await;
        let mut locked_nullifiers_in_flight = self.nullifiers_in_flight.write().await;
        let mut locked_notes_in_flight = self.notes_in_flight.write().await;

        // Remove account ids of transactions in block
        for update in block.updated_accounts() {
            let was_in_flight = locked_accounts_in_flight.remove(&update.account_id());
            debug_assert!(was_in_flight);
        }

        // Remove new nullifiers of transactions in block
        for nullifier in block.created_nullifiers() {
            let was_in_flight = locked_nullifiers_in_flight.remove(nullifier);
            debug_assert!(was_in_flight);
        }

        // Remove new notes of transactions in block
        for batch in block.created_notes() {
            for note in batch.iter() {
                let was_in_flight = locked_notes_in_flight.remove(&note.id());
                debug_assert!(was_in_flight);
            }
        }

        Ok(())
    }
}

// HELPERS
// -------------------------------------------------------------------------------------------------

/// Ensures the constraints related to in-flight transactions:
/// - the candidate transaction doesn't modify the same account as an existing in-flight
///   transaction (issue: #186)
/// - no consumed note's nullifier in candidate tx's consumed notes is already contained in
///   `already_consumed_nullifiers`
/// - all notes which not found in Store are in in-flight notes
#[instrument(target = "miden-block-producer", skip_all, err)]
fn ensure_in_flight_constraints(
    candidate_tx: &ProvenTransaction,
    accounts_in_flight: &BTreeSet<AccountId>,
    already_consumed_nullifiers: &BTreeSet<Nullifier>,
    notes_in_flight: &BTreeSet<NoteId>,
    tx_notes_not_in_store: &[NoteId],
) -> Result<(), VerifyTxError> {
    debug!(target: COMPONENT, accounts_in_flight = %format_array(accounts_in_flight), already_consumed_nullifiers = %format_array(already_consumed_nullifiers));

    // Check account id hasn't been modified yet
    if accounts_in_flight.contains(&candidate_tx.account_id()) {
        return Err(VerifyTxError::AccountAlreadyModifiedByOtherTx(candidate_tx.account_id()));
    }

    // Check no consumed notes were already consumed
    let infracting_nullifiers: Vec<Nullifier> = {
        candidate_tx
            .input_notes()
            .iter()
            .map(|commitment| commitment.nullifier())
            .filter(|nullifier| already_consumed_nullifiers.contains(nullifier))
            .collect()
    };

    if !infracting_nullifiers.is_empty() {
        return Err(VerifyTxError::InputNotesAlreadyConsumed(infracting_nullifiers));
    }

    // Check all notes not found in Store are in in-flight notes, return list of missing notes
    let missing_notes: Vec<NoteId> = tx_notes_not_in_store
        .iter()
        .filter(|note_id| !notes_in_flight.contains(note_id))
        .copied()
        .collect();
    if !missing_notes.is_empty() {
        return Err(VerifyTxError::UnauthenticatedNotesNotFound(missing_notes));
    }

    Ok(())
}

/// Ensures the constraints related to transaction inputs:
/// - the candidate transaction's initial account state hash must be the same as the one
///   in the Store or empty for new accounts
/// - input notes must not be already consumed
///
/// Returns a list of input notes that were not found in the Store
#[instrument(target = "miden-block-producer", skip_all, err)]
fn ensure_tx_inputs_constraints(
    candidate_tx: &ProvenTransaction,
    tx_inputs: TransactionInputs,
) -> Result<Vec<NoteId>, VerifyTxError> {
    debug!(target: COMPONENT, %tx_inputs);

    match tx_inputs.account_hash {
        // if the account is present in the Store, make sure that the account state hash
        // from the received transaction is the same as the one from the Store
        Some(store_account_hash) => {
            if candidate_tx.account_update().init_state_hash() != store_account_hash {
                return Err(VerifyTxError::IncorrectAccountInitialHash {
                    tx_initial_account_hash: candidate_tx.account_update().init_state_hash(),
                    store_account_hash: Some(store_account_hash),
                });
            }
        },
        // if the account is not present in the Store, it must be a new account
        None => {
            // if the initial account hash is not equal to `Digest::default()` it
            // signifies that the account is not new but is also not recorded in the Store
            if candidate_tx.account_update().init_state_hash() != Digest::default() {
                return Err(VerifyTxError::IncorrectAccountInitialHash {
                    tx_initial_account_hash: candidate_tx.account_update().init_state_hash(),
                    store_account_hash: None,
                });
            }
        },
    }

    let infracting_nullifiers: Vec<Nullifier> = tx_inputs
        .nullifiers
        .into_iter()
        .filter_map(|(nullifier_in_tx, block_num)| (block_num != 0).then_some(nullifier_in_tx))
        .collect();

    if !infracting_nullifiers.is_empty() {
        return Err(VerifyTxError::InputNotesAlreadyConsumed(infracting_nullifiers));
    }

    Ok(tx_inputs.missing_notes)
}
