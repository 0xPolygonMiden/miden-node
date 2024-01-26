use std::{collections::BTreeSet, sync::Arc};

use async_trait::async_trait;
use miden_node_utils::logging::format_array;
use miden_objects::{accounts::AccountId, notes::Nullifier, transaction::InputNotes, Digest};
use tokio::sync::RwLock;
use tracing::{debug, instrument};

use crate::{
    block::Block,
    store::{ApplyBlock, ApplyBlockError, Store, TxInputs},
    txqueue::{TransactionVerifier, VerifyTxError},
    SharedProvenTx, COMPONENT,
};

#[cfg(test)]
mod tests;

pub struct DefaultStateView<S> {
    store: Arc<S>,

    /// The account ID of accounts being modified by transactions currently in the block production
    /// pipeline. We currently ensure that only 1 tx/block modifies any given account (issue: #186).
    accounts_in_flight: Arc<RwLock<BTreeSet<AccountId>>>,

    /// The nullifiers of notes consumed by transactions currently in the block production pipeline.
    nullifiers_in_flight: Arc<RwLock<BTreeSet<Digest>>>,
}

impl<S> DefaultStateView<S>
where
    S: Store,
{
    pub fn new(store: Arc<S>) -> Self {
        Self {
            store,
            accounts_in_flight: Arc::new(RwLock::new(BTreeSet::new())),
            nullifiers_in_flight: Arc::new(RwLock::new(BTreeSet::new())),
        }
    }
}

#[async_trait]
impl<S> TransactionVerifier for DefaultStateView<S>
where
    S: Store,
{
    // TODO: Verify proof as well
    #[allow(clippy::blocks_in_conditions)] // Workaround of `instrument` issue
    #[instrument(skip_all, err)]
    async fn verify_tx(
        &self,
        candidate_tx: SharedProvenTx,
    ) -> Result<(), VerifyTxError> {
        // 1. soft-check if `tx` violates in-flight requirements.
        //
        // This is a "soft" check, because we'll need to redo it at the end. We do this soft check
        // to quickly reject clearly infracting transactions before hitting the store (slow).
        ensure_in_flight_constraints(
            candidate_tx.clone(),
            &*self.accounts_in_flight.read().await,
            &*self.nullifiers_in_flight.read().await,
        )?;

        // 2. Fetch the transaction inputs from the store, and check tx input constraints
        let tx_inputs = self.store.get_tx_inputs(candidate_tx.clone()).await?;
        ensure_tx_inputs_constraints(candidate_tx.clone(), tx_inputs)?;

        // 3. Re-check in-flight transaction constraints, and if verification passes, register
        //    transaction
        //
        // Note: We need to re-check these constraints because we dropped the locks since we last
        // checked
        {
            let mut locked_accounts_in_flight = self.accounts_in_flight.write().await;
            let mut locked_nullifiers_in_flight = self.nullifiers_in_flight.write().await;

            ensure_in_flight_constraints(
                candidate_tx.clone(),
                &locked_accounts_in_flight,
                &locked_nullifiers_in_flight,
            )?;

            // Success! Register transaction as successfully verified
            locked_accounts_in_flight.insert(candidate_tx.account_id());

            let mut nullifiers_in_tx: BTreeSet<_> =
                candidate_tx.input_notes().iter().map(|nullifier| nullifier.inner()).collect();
            locked_nullifiers_in_flight.append(&mut nullifiers_in_tx);
        }

        Ok(())
    }
}

#[async_trait]
impl<S> ApplyBlock for DefaultStateView<S>
where
    S: Store,
{
    async fn apply_block(
        &self,
        block: Block,
    ) -> Result<(), ApplyBlockError> {
        self.store.apply_block(block.clone()).await?;

        let mut locked_accounts_in_flight = self.accounts_in_flight.write().await;
        let mut locked_nullifiers_in_flight = self.nullifiers_in_flight.write().await;

        // 1. Remove account ids of transactions in block
        let account_ids_in_block = block
            .updated_accounts
            .iter()
            .map(|(account_id, _final_account_hash)| account_id);

        for account_id in account_ids_in_block {
            let was_in_flight = locked_accounts_in_flight.remove(account_id);
            debug_assert!(was_in_flight);
        }

        // 2. Remove new nullifiers of transactions in block
        for nullifier in block.produced_nullifiers.iter() {
            let was_in_flight = locked_nullifiers_in_flight.remove(nullifier);
            debug_assert!(was_in_flight);
        }

        Ok(())
    }
}

// HELPERS
// -------------------------------------------------------------------------------------------------

/// Ensures the constraints related to in-flight transactions:
/// 1. the candidate transaction doesn't modify the same account as an existing in-flight
///    transaction (issue: #186)
/// 2. no consumed note's nullifier in candidate tx's consumed notes is already contained in
///    `already_consumed_nullifiers`
#[instrument(target = "miden-block-producer", skip_all, err)]
fn ensure_in_flight_constraints(
    candidate_tx: SharedProvenTx,
    accounts_in_flight: &BTreeSet<AccountId>,
    already_consumed_nullifiers: &BTreeSet<Digest>,
) -> Result<(), VerifyTxError> {
    debug!(target: COMPONENT, accounts_in_flight = %format_array(accounts_in_flight), already_consumed_nullifiers = %format_array(already_consumed_nullifiers));

    // 1. Check account id hasn't been modified yet
    if accounts_in_flight.contains(&candidate_tx.account_id()) {
        return Err(VerifyTxError::AccountAlreadyModifiedByOtherTx(candidate_tx.account_id()));
    }

    // 2. Check no consumed notes were already consumed
    let infracting_nullifiers: Vec<Nullifier> = {
        candidate_tx
            .input_notes()
            .iter()
            .filter(|&nullifier_in_tx| {
                already_consumed_nullifiers.contains(&nullifier_in_tx.inner())
            })
            .cloned()
            .collect()
    };

    if !infracting_nullifiers.is_empty() {
        return Err(VerifyTxError::InputNotesAlreadyConsumed(InputNotes::new(
            infracting_nullifiers,
        )?));
    }

    Ok(())
}

#[instrument(target = "miden-block-producer", skip_all, err)]
fn ensure_tx_inputs_constraints(
    candidate_tx: SharedProvenTx,
    tx_inputs: TxInputs,
) -> Result<(), VerifyTxError> {
    debug!(target: COMPONENT, %tx_inputs);

    match tx_inputs.account_hash {
        Some(store_account_hash) => {
            if candidate_tx.initial_account_hash() != store_account_hash {
                return Err(VerifyTxError::IncorrectAccountInitialHash {
                    tx_initial_account_hash: candidate_tx.initial_account_hash(),
                    store_account_hash: Some(store_account_hash),
                });
            }
        },
        None => {
            return Err(VerifyTxError::IncorrectAccountInitialHash {
                tx_initial_account_hash: candidate_tx.initial_account_hash(),
                store_account_hash: None,
            })
        },
    }

    let infracting_nullifiers: Vec<Nullifier> = tx_inputs
        .nullifiers
        .into_iter()
        .filter(|&(_, is_already_consumed)| is_already_consumed)
        .map(|(nullifier_in_tx, _)| nullifier_in_tx.into())
        .collect();

    if !infracting_nullifiers.is_empty() {
        return Err(VerifyTxError::InputNotesAlreadyConsumed(InputNotes::new(
            infracting_nullifiers,
        )?));
    }

    Ok(())
}
