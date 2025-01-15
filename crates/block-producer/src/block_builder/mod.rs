use std::{collections::BTreeSet, ops::Range};

use miden_node_utils::formatting::format_array;
use miden_objects::{
    accounts::AccountId,
    block::Block,
    notes::{NoteHeader, Nullifier},
    transaction::InputNoteCommitment,
};
use rand::Rng;
use tokio::time::Duration;
use tracing::{debug, info, instrument};

use crate::{
    batch_builder::batch::TransactionBatch, errors::BuildBlockError, mempool::SharedMempool,
    store::StoreClient, COMPONENT, SERVER_BLOCK_FREQUENCY,
};

pub(crate) mod prover;

use self::prover::{block_witness::BlockWitness, BlockProver};

// BLOCK BUILDER
// =================================================================================================

pub struct BlockBuilder {
    pub block_interval: Duration,
    /// Used to simulate block proving by sleeping for a random duration selected from this range.
    pub simulated_proof_time: Range<Duration>,

    /// Simulated block failure rate as a percentage.
    ///
    /// Note: this _must_ be sign positive and less than 1.0.
    pub failure_rate: f32,

    pub store: StoreClient,
    pub block_kernel: BlockProver,
}

impl BlockBuilder {
    pub fn new(store: StoreClient) -> Self {
        Self {
            block_interval: SERVER_BLOCK_FREQUENCY,
            // Note: The range cannot be empty.
            simulated_proof_time: Duration::ZERO..Duration::from_millis(1),
            failure_rate: 0.0,
            block_kernel: BlockProver::new(),
            store,
        }
    }
    /// Starts the [BlockBuilder], infinitely producing blocks at the configured interval.
    ///
    /// Block production is sequential and consists of
    ///
    ///   1. Pulling the next set of batches from the mempool
    ///   2. Compiling these batches into the next block
    ///   3. Proving the block (this is simulated using random sleeps)
    ///   4. Committing the block to the store
    pub async fn run(self, mempool: SharedMempool) {
        assert!(
            self.failure_rate < 1.0 && self.failure_rate.is_sign_positive(),
            "Failure rate must be a percentage"
        );

        let mut interval = tokio::time::interval(self.block_interval);
        // We set the inverval's missed tick behaviour to burst. This means we'll catch up missed
        // blocks as fast as possible. In other words, we try our best to keep the desired block
        // interval on average. The other options would result in at least one skipped block.
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Burst);

        loop {
            interval.tick().await;

            let (block_number, batches) = mempool.lock().await.select_block();

            let mut result = self.build_block(&batches).await;
            let proving_duration = rand::thread_rng().gen_range(self.simulated_proof_time.clone());

            tokio::time::sleep(proving_duration).await;

            // Randomly inject failures at the given rate.
            //
            // Note: Rng::gen rolls between [0, 1.0) for f32, so this works as expected.
            if rand::thread_rng().gen::<f32>() < self.failure_rate {
                result = Err(BuildBlockError::InjectedFailure);
            }

            let mut mempool = mempool.lock().await;
            match result {
                Ok(_) => mempool.block_committed(block_number),
                Err(_) => mempool.block_failed(block_number),
            }
        }
    }

    #[instrument(target = COMPONENT, skip_all, err)]
    pub async fn build_block(&self, batches: &[TransactionBatch]) -> Result<(), BuildBlockError> {
        info!(
            target: COMPONENT,
            num_batches = batches.len(),
            batches = %format_array(batches.iter().map(|batch| batch.id())),
        );

        let updated_account_set: BTreeSet<AccountId> = batches
            .iter()
            .flat_map(TransactionBatch::updated_accounts)
            .map(|(account_id, _)| *account_id)
            .collect();

        let output_notes: Vec<_> =
            batches.iter().map(TransactionBatch::output_notes).cloned().collect();

        let produced_nullifiers: Vec<Nullifier> =
            batches.iter().flat_map(TransactionBatch::produced_nullifiers).collect();

        // Populate set of output notes from all batches
        let output_notes_set: BTreeSet<_> = output_notes
            .iter()
            .flat_map(|batch| batch.iter().map(|note| note.id()))
            .collect();

        // Build a set of unauthenticated input notes for this block which do not have a matching
        // output note produced in this block
        let dangling_notes: BTreeSet<_> = batches
            .iter()
            .flat_map(TransactionBatch::input_notes)
            .filter_map(InputNoteCommitment::header)
            .map(NoteHeader::id)
            .filter(|note_id| !output_notes_set.contains(note_id))
            .collect();

        // Request information needed for block building from the store
        let block_inputs = self
            .store
            .get_block_inputs(
                updated_account_set.into_iter(),
                produced_nullifiers.iter(),
                dangling_notes.iter(),
            )
            .await
            .map_err(BuildBlockError::GetBlockInputsFailed)?;

        let missing_notes: Vec<_> = dangling_notes
            .difference(&block_inputs.found_unauthenticated_notes.note_ids())
            .copied()
            .collect();
        if !missing_notes.is_empty() {
            return Err(BuildBlockError::UnauthenticatedNotesNotFound(missing_notes));
        }

        let (block_header_witness, updated_accounts) = BlockWitness::new(block_inputs, batches)?;

        let new_block_header = self.block_kernel.prove(block_header_witness)?;
        let block_num = new_block_header.block_num();

        // TODO: return an error?
        let block =
            Block::new(new_block_header, updated_accounts, output_notes, produced_nullifiers)
                .expect("invalid block components");

        let block_hash = block.hash();

        info!(target: COMPONENT, block_num, %block_hash, "block built");
        debug!(target: COMPONENT, ?block);

        self.store
            .apply_block(&block)
            .await
            .map_err(BuildBlockError::StoreApplyBlockFailed)?;

        info!(target: COMPONENT, block_num, %block_hash, "block committed");

        Ok(())
    }
}
