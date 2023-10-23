use std::{cmp::min, fmt::Debug, sync::Arc, time::Duration};

use async_trait::async_trait;
use miden_objects::{accounts::AccountId, Digest};
use tokio::{sync::RwLock, time};

use crate::{block_builder::BlockBuilder, SharedProvenTx, SharedRwVec};

// TRANSACTION BATCH
// ================================================================================================

/// A batch of transactions that share a common proof. For any given account, at most 1 transaction
/// in the batch must be addressing that account.
///
/// Note: Until recursive proofs are available in the Miden VM, we don't include the common proof.
pub struct TransactionBatch {
    txs: Vec<SharedProvenTx>,
}

impl TransactionBatch {
    pub fn new(txs: Vec<SharedProvenTx>) -> Self {
        Self { txs }
    }

    /// Returns an iterator over account ids that were modified in the transaction batch, and their
    /// corresponding new hash
    pub fn updated_accounts(&self) -> impl Iterator<Item = (AccountId, Digest)> + '_ {
        self.txs.iter().map(|tx| (tx.account_id(), tx.final_account_hash()))
    }
}

// BATCH BUILDER
// ================================================================================================

#[async_trait]
pub trait BatchBuilder: Send + Sync + 'static {
    // TODO: Make concrete `AddBatches` Error?
    type AddBatchesError: Debug;

    async fn add_tx_groups(
        &self,
        tx_groups: Vec<Vec<SharedProvenTx>>,
    ) -> Result<(), Self::AddBatchesError>;
}

pub struct DefaultBatchBuilderOptions {
    /// The frequency at which blocks are created
    pub block_frequency: Duration,

    /// Maximum number of batches in any given block
    pub max_batches_per_block: usize,
}

pub struct DefaultBatchBuilder<BB>
where
    BB: BlockBuilder,
{
    /// Batches ready to be included in a block
    ready_batches: SharedRwVec<Arc<TransactionBatch>>,

    block_builder: Arc<BB>,

    options: DefaultBatchBuilderOptions,
}

impl<BB> DefaultBatchBuilder<BB>
where
    BB: BlockBuilder,
{
    pub fn new(
        block_builder: Arc<BB>,
        options: DefaultBatchBuilderOptions,
    ) -> Self {
        Self {
            ready_batches: Arc::new(RwLock::new(Vec::new())),
            block_builder,
            options,
        }
    }

    pub async fn run(self) {
        let mut interval = time::interval(self.options.block_frequency);

        loop {
            interval.tick().await;
            self.try_send_batches().await;
        }
    }

    /// Note that we call `add_batches()` regardless of whether the `ready_batches` queue is empty.
    /// A call to an empty `add_batches()` indicates that an empty block should be created.
    async fn try_send_batches(&self) {
        let mut locked_ready_batches = self.ready_batches.write().await;

        let num_batches_to_send =
            min(self.options.max_batches_per_block, locked_ready_batches.len());
        let batches_to_send = locked_ready_batches[..num_batches_to_send].to_vec();

        match self.block_builder.add_batches(batches_to_send) {
            Ok(_) => {
                // transaction groups were successfully sent; remove the batches that we sent
                *locked_ready_batches = locked_ready_batches[num_batches_to_send..].to_vec();
            },
            Err(_) => {
                // Batches were not sent, and remain in the queue. Do nothing.
            },
        }
    }
}

#[derive(Debug)]
pub enum DefaultAddBatchesError {
    AccountAccessedByMultipleTxs(AccountId),
}

#[async_trait]
impl<BB> BatchBuilder for DefaultBatchBuilder<BB>
where
    BB: BlockBuilder,
{
    type AddBatchesError = DefaultAddBatchesError;

    async fn add_tx_groups(
        &self,
        tx_groups: Vec<Vec<SharedProvenTx>>,
    ) -> Result<(), Self::AddBatchesError> {
        confirm_at_most_one_tx_per_account(&tx_groups)?;

        let ready_batches = self.ready_batches.clone();

        tokio::spawn(async move {
            let mut batches = groups_to_batches(tx_groups).await;

            ready_batches.write().await.append(&mut batches);
        });

        Ok(())
    }
}

/// Confirms that for any given account, at most one transaction in the the transaction group
/// addresses that account.
fn confirm_at_most_one_tx_per_account(
    tx_groups: &[Vec<SharedProvenTx>]
) -> Result<(), DefaultAddBatchesError> {
    let account_ids: Vec<AccountId> =
        tx_groups.iter().flatten().map(|tx| tx.account_id()).collect();

    // We do a dumb O(n^2) search because `AccountId` doesn't derive `Hash` at the moment, which is
    // necessary for faster algorithms (notably, using `Itertools::all_unique()`)
    for (runner_1_index, runner_1_acc_id) in account_ids.iter().enumerate() {
        for (runner_2_index, runner_2_acc_id) in account_ids.iter().enumerate() {
            if runner_1_index == runner_2_index {
                // We're looking at the same item - skip
                continue;
            }

            if runner_1_acc_id == runner_2_acc_id {
                return Err(DefaultAddBatchesError::AccountAccessedByMultipleTxs(
                    runner_1_acc_id.clone(),
                ));
            }
        }
    }

    Ok(())
}

/// Transforms the transaction groups to transaction batches
async fn groups_to_batches(tx_groups: Vec<Vec<SharedProvenTx>>) -> Vec<Arc<TransactionBatch>> {
    // Note: in the future, this will send jobs to a cluster to transform groups into batches
    tx_groups.into_iter().map(|txs| Arc::new(TransactionBatch::new(txs))).collect()
}
