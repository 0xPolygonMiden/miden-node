use std::{collections::BTreeSet, sync::Arc};

use async_trait::async_trait;
use miden_objects::{accounts::AccountId, Digest};
use tokio::sync::RwLock;

use crate::{
    block::Block,
    store::GetTxInputs,
    txqueue::{TransactionVerifier, VerifyTxError},
    SharedProvenTx,
};

#[derive(Debug)]
pub enum ApplyBlockError {}

#[async_trait]
pub trait ApplyBlock {
    async fn apply_block(
        &self,
        block: Block,
    ) -> Result<(), ApplyBlockError>;
}

pub struct DefaulStateView<TI>
where
    TI: GetTxInputs,
{
    get_tx_inputs: Arc<TI>,

    /// The account ID of accounts being modified by transactions currently in the block production
    /// pipeline. We currently ensure that only 1 tx/block modifies any given account.
    accounts_in_flight: Arc<RwLock<BTreeSet<AccountId>>>,

    /// The nullifiers of notes consumed by transactions currently in the block production pipeline.
    nullifiers_in_flight: Arc<RwLock<BTreeSet<Digest>>>,
}

impl<TI> DefaulStateView<TI>
where
    TI: GetTxInputs,
{
    pub fn new(get_tx_inputs: Arc<TI>) -> Self {
        Self {
            get_tx_inputs,
            accounts_in_flight: Arc::new(RwLock::new(BTreeSet::new())),
            nullifiers_in_flight: Arc::new(RwLock::new(BTreeSet::new())),
        }
    }
}

#[async_trait]
impl<TI> TransactionVerifier for DefaulStateView<TI>
where
    TI: GetTxInputs,
{
    // TODO: Verify proof as well
    async fn verify_tx(
        &self,
        tx: SharedProvenTx,
    ) -> Result<(), VerifyTxError> {
        // 1. soft-check if `tx` violates in-flight requirements.
        //
        // This is a "soft" check, because we'll need to redo it at the end. We do this soft check
        // to quickly reject clearly infracting transactions before hitting the store (slow).
        ensure_in_flight_constraints(
            tx.clone(),
            &*self.accounts_in_flight.read().await,
            &*self.nullifiers_in_flight.read().await,
        )?;

        // 2. Fetch the transaction inputs from the store
        let tx_inputs = self.get_tx_inputs.get_tx_inputs(tx.clone()).await?;

        // 3. Checks against transaction inputs
        match tx_inputs.account_hash {
            Some(store_account_hash) => {
                if tx.initial_account_hash() != store_account_hash {
                    return Err(VerifyTxError::IncorrectAccountInitialHash {
                        tx_initial_account_hash: tx.initial_account_hash(),
                        store_account_hash: Some(store_account_hash),
                    });
                }
            },
            None => {
                return Err(VerifyTxError::IncorrectAccountInitialHash {
                    tx_initial_account_hash: tx.initial_account_hash(),
                    store_account_hash: None,
                })
            },
        }

        let infracting_nullifiers: Vec<_> = tx_inputs
            .nullifiers
            .into_iter()
            .filter_map(|(nullifier_in_tx, is_already_consumed)| {
                // If already consumed, add to list of infracting nullifiers
                is_already_consumed.then(|| nullifier_in_tx)
            })
            .collect();

        if !infracting_nullifiers.is_empty() {
            return Err(VerifyTxError::ConsumedNotesAlreadyConsumed(infracting_nullifiers));
        }

        // 4. Re-check against in-flight transactions, and if verification passes, register
        //    transaction
        {
            let mut locked_accounts_in_flight = self.accounts_in_flight.write().await;
            let mut locked_nullifiers_in_flight = self.nullifiers_in_flight.write().await;

            ensure_in_flight_constraints(
                tx.clone(),
                &locked_accounts_in_flight,
                &locked_nullifiers_in_flight,
            )?;

            // Success! Register transaction as successfully verified
            locked_accounts_in_flight.insert(tx.account_id());

            let mut nullifiers_in_tx: BTreeSet<_> =
                tx.consumed_notes().iter().map(|note| note.nullifier()).collect();
            locked_nullifiers_in_flight.append(&mut nullifiers_in_tx);
        }

        Ok(())
    }
}

#[async_trait]
impl<TI> ApplyBlock for DefaulStateView<TI>
where
    TI: GetTxInputs,
{
    async fn apply_block(
        &self,
        block: Block,
    ) -> Result<(), ApplyBlockError> {
        todo!()
    }
}

// HELPERS
// -------------------------------------------------------------------------------------------------

/// Ensures the constraints related to in-flight transactions:
/// 1. the candidate transaction doesn't modify the same account as an existing in-flight transaction
/// 2. no consumed note's nullifier in candidate tx's consumed notes is already contained
/// in `already_consumed_nullifiers`
fn ensure_in_flight_constraints(
    candidate_tx: SharedProvenTx,
    accounts_in_flight: &BTreeSet<AccountId>,
    already_consumed_nullifiers: &BTreeSet<Digest>,
) -> Result<(), VerifyTxError> {
    // 1. Check account id hasn't been modified yet
    if accounts_in_flight.contains(&candidate_tx.account_id()) {
        return Err(VerifyTxError::AccountAlreadyModifiedByOtherTx);
    }

    // 2. Check no consumed notes were already consumed
    let infracting_nullifiers: Vec<Digest> = {
        let nullifiers_in_tx = candidate_tx.consumed_notes().iter().map(|note| note.nullifier());

        nullifiers_in_tx
            .filter(|nullifier_in_tx| already_consumed_nullifiers.contains(nullifier_in_tx))
            .collect()
    };

    if !infracting_nullifiers.is_empty() {
        return Err(VerifyTxError::ConsumedNotesAlreadyConsumed(infracting_nullifiers));
    }

    Ok(())
}
