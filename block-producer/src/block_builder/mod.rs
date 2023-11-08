use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use miden_node_proto::domain::{AccountInputRecord, BlockInputs};
use miden_objects::{accounts::AccountId, BlockHeader, Digest, Felt};
use miden_vm::{crypto::MerkleStore, AdviceInputs, StackInputs};
use thiserror::Error;

use crate::{block::Block, store::Store, SharedTxBatch};

mod kernel;
use self::kernel::{BlockKernel, AccountRootUpdateError};

#[cfg(test)]
mod tests;

// BLOCK BUILDER
// =================================================================================================

#[derive(Debug, Error)]
pub enum BuildBlockError {
    #[error("failed to update account root")]
    AccountRootUpdateFailed(AccountRootUpdateError),
    #[error("dummy")]
    Dummy,
}

impl From<AccountRootUpdateError> for BuildBlockError {
    fn from(err: AccountRootUpdateError) -> Self {
        Self::AccountRootUpdateFailed(err)
    }
}

#[async_trait]
pub trait BlockBuilder: Send + Sync + 'static {
    /// Receive batches to be included in a block. An empty vector indicates that no batches were
    /// ready, and that an empty block should be created.
    ///
    /// The `BlockBuilder` relies on `build_block()` to be called as a precondition to creating a
    /// block. In other words, if `build_block()` is never called, then no blocks are produced.
    async fn build_block(
        &self,
        batches: Vec<SharedTxBatch>,
    ) -> Result<(), BuildBlockError>;
}

#[derive(Debug)]
pub struct DefaultBlockBuilder<S> {
    store: Arc<S>,
    account_root_program: BlockKernel,
}

impl<S> DefaultBlockBuilder<S>
where
    S: Store,
{
    pub fn new(store: Arc<S>) -> Self {
        Self {
            store,
            account_root_program: BlockKernel::new(),
        }
    }

    fn compute_block_header(
        &self,
        prev_block_header: &BlockHeader,
        account_states: Vec<AccountInputRecord>,
        account_updates: impl Iterator<Item = (AccountId, Digest)>,
    ) -> Result<BlockHeader, BuildBlockError> {
        let prev_hash = prev_block_header.prev_hash();
        let chain_root = Digest::default();
        let account_root = self.account_root_program.compute_new_account_root(
            account_states
                .into_iter()
                .map(|record| (record.account_id, record.account_hash, record.proof)),
            account_updates,
            prev_block_header.account_root(),
        )?;
        let nullifier_root = Digest::default();
        let note_root = Digest::default();
        let batch_root = Digest::default();
        let proof_hash = Digest::default();
        let timestamp: Felt = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("today is expected to be before 1970")
            .as_millis()
            .into();

        Ok(BlockHeader::new(
            prev_hash,
            prev_block_header.block_num(),
            chain_root,
            account_root,
            nullifier_root,
            note_root,
            batch_root,
            proof_hash,
            prev_block_header.version(),
            timestamp,
        ))
    }
}

#[async_trait]
impl<S> BlockBuilder for DefaultBlockBuilder<S>
where
    S: Store,
{
    async fn build_block(
        &self,
        batches: Vec<SharedTxBatch>,
    ) -> Result<(), BuildBlockError> {
        let account_updates: Vec<(AccountId, Digest)> =
            batches.iter().flat_map(|batch| batch.updated_accounts()).collect();
        let created_notes: Vec<Digest> =
            batches.iter().flat_map(|batch| batch.created_notes()).collect();
        let produced_nullifiers: Vec<Digest> =
            batches.iter().flat_map(|batch| batch.produced_nullifiers()).collect();

        let BlockInputs {
            block_header: prev_block_header,
            chain_peaks: _,
            account_states,
            nullifiers: _,
        } = self
            .store
            .get_block_inputs(
                account_updates.iter().map(|(account_id, _)| account_id),
                produced_nullifiers.iter(),
            )
            .await
            .unwrap();

        let new_block_header = self.compute_block_header(
            &prev_block_header,
            account_states,
            account_updates.iter().cloned(),
        )?;

        let block = Arc::new(Block {
            header: new_block_header,
            updated_accounts: account_updates,
            created_notes,
            produced_nullifiers,
        });

        // TODO: properly handle
        self.store.apply_block(block.clone()).await.expect("apply block failed");

        Ok(())
    }
}

// HELPERS
// =================================================================================================

/// Provides inputs to the `BlockKernel` so that it can generate the new header
struct BlockHeaderWitness {
    account_states: Vec<AccountInputRecord>,
    account_updates: Vec<(AccountId, Digest)>,

    prev_header: BlockHeader,
}

impl BlockHeaderWitness {
    fn new(
        block_inputs: BlockInputs,
        batches: Vec<SharedTxBatch>,
        prev_header: BlockHeader,
    ) -> Result<Self, BuildBlockError> {
        // TODO: VALIDATE
        // - initial account states in `BlockInputs` are the same as in batches
        // - Block height returned for each nullifier is 0.


        let account_updates: Vec<(AccountId, Digest)> =
            batches.iter().flat_map(|batch| batch.updated_accounts()).collect();

        Ok(Self {
            account_states: block_inputs.account_states,
            account_updates,
            prev_header,
        })
    }

    fn into_parts(self) -> Result<(AdviceInputs, StackInputs), BuildBlockError> {
        let advice_inputs = {
            let mut merkle_store = MerkleStore::default();
            merkle_store
                .add_merkle_paths(self.account_states.into_iter().map(
                    |AccountInputRecord {
                         account_id,
                         account_hash,
                         proof,
                     }| (u64::from(account_id), account_hash, proof),
                ))
                .map_err(AccountRootUpdateError::InvalidMerklePaths)?;

            AdviceInputs::default().with_merkle_store(merkle_store)
        };

        let stack_inputs = {
            // Note: `StackInputs::new()` reverses the input vector, so we need to construct the stack
            // from the bottom to the top
            let mut stack_inputs = Vec::new();

            // append all insert key/values
            let mut num_accounts_updated: u64 = 0;
            for (idx, (account_id, new_account_hash)) in
                self.account_updates.into_iter().enumerate()
            {
                stack_inputs.push(account_id.into());
                stack_inputs.extend(new_account_hash);

                let idx = u64::try_from(idx).expect("can't be more than 2^64 - 1 accounts");
                num_accounts_updated = idx + 1;
            }

            // append initial account root
            stack_inputs.extend(self.prev_header.account_root());

            // append number of accounts updated
            stack_inputs.push(num_accounts_updated.into());

            StackInputs::new(stack_inputs)
        };

        Ok((advice_inputs, stack_inputs))
    }

    /// Note: this method will no longer be necessary when we will have a full block kernel.
    fn get_previous_header(&self) -> &BlockHeader {
        &self.prev_header
    }
}
