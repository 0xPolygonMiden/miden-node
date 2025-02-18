use std::{
    collections::BTreeSet,
    ops::{Add, Range},
};

use futures::FutureExt;
use miden_node_utils::tracing::OpenTelemetrySpanExt;
use miden_objects::{
    account::AccountId,
    batch::ProvenBatch,
    block::{Block, BlockNumber},
    note::{NoteHeader, NoteId, Nullifier},
    transaction::{InputNoteCommitment, OutputNote},
};
use rand::Rng;
use tokio::time::Duration;
use tracing::{instrument, Span};

use crate::{
    block::BlockInputs, errors::BuildBlockError, mempool::SharedMempool, store::StoreClient,
    COMPONENT, SERVER_BLOCK_FREQUENCY,
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
    pub failure_rate: f64,

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
    /// Starts the [`BlockBuilder`], infinitely producing blocks at the configured interval.
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

            self.build_block(&mempool).await;
        }
    }

    /// Run the block building stages and add open-telemetry trace information where applicable.
    ///
    /// A failure in any stage will result in that block being rolled back.
    ///
    /// ## Telemetry
    ///
    /// - Creates a new root span which means each block gets its own complete trace.
    /// - Important telemetry fields are added to the root span with the `block.xxx` prefix.
    /// - Each stage has its own child span and are free to add further field data.
    /// - A failed stage will emit an error event, and both its own span and the root span will be
    ///   marked as errors.
    #[instrument(parent = None, target = COMPONENT, name = "block_builder.build_block", skip_all)]
    async fn build_block(&self, mempool: &SharedMempool) {
        use futures::TryFutureExt;

        Self::select_block(mempool)
            .inspect(SelectedBlock::inject_telemetry)
            .then(|selected| self.get_block_inputs(selected))
            .inspect_ok(BlockSummaryAndInputs::inject_telemetry)
            .and_then(|inputs| self.prove_block(inputs))
            .inspect_ok(ProvenBlock::inject_telemetry)
            // Failure must be injected before the final pipeline stage i.e. before commit is called. The system cannot
            // handle errors after it considers the process complete (which makes sense).
            .and_then(|proven_block| async { self.inject_failure(proven_block) })
            .and_then(|proven_block| self.commit_block(mempool, proven_block))
            // Handle errors by propagating the error to the root span and rolling back the block.
            .inspect_err(|err| Span::current().set_error(err))
            .or_else(|_err| self.rollback_block(mempool).never_error())
            // Error has been handled, this is just type manipulation to remove the result wrapper.
            .unwrap_or_else(|_| ())
            .await;
    }

    #[instrument(target = COMPONENT, name = "block_builder.select_block", skip_all)]
    async fn select_block(mempool: &SharedMempool) -> SelectedBlock {
        let (block_number, batches) = mempool.lock().await.select_block();
        SelectedBlock { block_number, batches }
    }

    #[instrument(target = COMPONENT, name = "block_builder.get_block_inputs", skip_all, err)]
    async fn get_block_inputs(
        &self,
        selected_block: SelectedBlock,
    ) -> Result<BlockSummaryAndInputs, BuildBlockError> {
        let SelectedBlock { block_number: _, batches } = selected_block;
        let summary = BlockSummary::summarize_batches(&batches);

        let inputs = self
            .store
            .get_block_inputs(
                summary.updated_accounts.iter().copied(),
                summary.nullifiers.iter(),
                summary.dangling_notes.iter(),
            )
            .await
            .map_err(BuildBlockError::GetBlockInputsFailed)?;

        let missing_notes: Vec<_> = summary
            .dangling_notes
            .difference(&inputs.found_unauthenticated_notes.note_ids())
            .copied()
            .collect();
        if !missing_notes.is_empty() {
            return Err(BuildBlockError::UnauthenticatedNotesNotFound(missing_notes));
        }

        Ok(BlockSummaryAndInputs { batches, summary, inputs })
    }

    #[instrument(target = COMPONENT, name = "block_builder.prove_block", skip_all, err)]
    async fn prove_block(
        &self,
        preimage: BlockSummaryAndInputs,
    ) -> Result<ProvenBlock, BuildBlockError> {
        let BlockSummaryAndInputs { batches, summary, inputs } = preimage;

        let (block_header_witness, updated_accounts) = BlockWitness::new(inputs, &batches)?;

        let new_block_header = self.block_kernel.prove(block_header_witness)?;

        let block = Block::new(
            new_block_header,
            updated_accounts,
            summary.output_notes,
            summary.nullifiers,
        )?;

        self.simulate_proving().await;

        Ok(ProvenBlock { block })
    }

    #[instrument(target = COMPONENT, name = "block_builder.commit_block", skip_all, err)]
    async fn commit_block(
        &self,
        mempool: &SharedMempool,
        proven_block: ProvenBlock,
    ) -> Result<(), BuildBlockError> {
        self.store
            .apply_block(&proven_block.block)
            .await
            .map_err(BuildBlockError::StoreApplyBlockFailed)?;

        mempool.lock().await.commit_block();

        Ok(())
    }

    #[instrument(target = COMPONENT, name = "block_builder.rollback_block", skip_all)]
    async fn rollback_block(&self, mempool: &SharedMempool) {
        mempool.lock().await.rollback_block();
    }

    #[instrument(target = COMPONENT, name = "block_builder.simulate_proving", skip_all)]
    async fn simulate_proving(&self) {
        let proving_duration = rand::thread_rng().gen_range(self.simulated_proof_time.clone());

        Span::current().set_attribute("range.min_s", self.simulated_proof_time.start);
        Span::current().set_attribute("range.max_s", self.simulated_proof_time.end);
        Span::current().set_attribute("dice_roll_s", proving_duration);

        tokio::time::sleep(proving_duration).await;
    }

    #[instrument(target = COMPONENT, name = "block_builder.inject_failure", skip_all, err)]
    fn inject_failure<T>(&self, value: T) -> Result<T, BuildBlockError> {
        let roll = rand::thread_rng().gen::<f64>();

        Span::current().set_attribute("failure_rate", self.failure_rate);
        Span::current().set_attribute("dice_roll", roll);

        if roll < self.failure_rate {
            Err(BuildBlockError::InjectedFailure)
        } else {
            Ok(value)
        }
    }
}

struct BlockSummary {
    updated_accounts: BTreeSet<AccountId>,
    nullifiers: Vec<Nullifier>,
    output_notes: Vec<Vec<OutputNote>>,
    dangling_notes: BTreeSet<NoteId>,
}

impl BlockSummary {
    #[instrument(target = COMPONENT, name = "block_builder.summarize_batches", skip_all)]
    fn summarize_batches(batches: &[ProvenBatch]) -> Self {
        let updated_accounts: BTreeSet<AccountId> = batches
            .iter()
            .flat_map(ProvenBatch::account_updates)
            .map(|(account_id, _)| *account_id)
            .collect();

        let output_notes: Vec<_> =
            batches.iter().map(|batch| batch.output_notes().to_vec()).collect();

        let nullifiers: Vec<Nullifier> =
            batches.iter().flat_map(ProvenBatch::produced_nullifiers).collect();

        // Populate set of output notes from all batches
        let output_notes_set: BTreeSet<_> = output_notes
            .iter()
            .flat_map(|output_notes| output_notes.iter().map(OutputNote::id))
            .collect();

        // Build a set of unauthenticated input notes for this block which do not have a
        // matching output note produced in this block
        let dangling_notes: BTreeSet<_> = batches
            .iter()
            .flat_map(ProvenBatch::input_notes)
            .filter_map(InputNoteCommitment::header)
            .map(NoteHeader::id)
            .filter(|note_id| !output_notes_set.contains(note_id))
            .collect();

        Self {
            updated_accounts,
            nullifiers,
            output_notes,
            dangling_notes,
        }
    }
}

struct SelectedBlock {
    block_number: BlockNumber,
    batches: Vec<ProvenBatch>,
}
struct BlockSummaryAndInputs {
    batches: Vec<ProvenBatch>,
    summary: BlockSummary,
    inputs: BlockInputs,
}
struct ProvenBlock {
    block: Block,
}

impl SelectedBlock {
    fn inject_telemetry(&self) {
        let span = Span::current();
        span.set_attribute("block.number", self.block_number.as_u32());
        span.set_attribute("block.batches.count", self.batches.len() as u32);
    }
}

impl BlockSummaryAndInputs {
    fn inject_telemetry(&self) {
        let span = Span::current();

        // SAFETY: We do not expect to have more than u32::MAX of any count per block.
        span.set_attribute(
            "block.updated_accounts.count",
            i64::try_from(self.summary.updated_accounts.len())
                .expect("less than u32::MAX account updates"),
        );
        span.set_attribute(
            "block.output_notes.count",
            i64::try_from(self.summary.output_notes.iter().fold(0, |acc, x| acc.add(x.len())))
                .expect("less than u32::MAX output notes"),
        );
        span.set_attribute(
            "block.nullifiers.count",
            i64::try_from(self.summary.nullifiers.len()).expect("less than u32::MAX nullifiers"),
        );
        span.set_attribute(
            "block.dangling_notes.count",
            i64::try_from(self.summary.dangling_notes.len())
                .expect("less than u32::MAX dangling notes"),
        );
    }
}

impl ProvenBlock {
    fn inject_telemetry(&self) {
        let span = Span::current();
        let header = self.block.header();

        span.set_attribute("block.hash", header.hash());
        span.set_attribute("block.sub_hash", header.sub_hash());
        span.set_attribute("block.parent_hash", header.prev_hash());

        span.set_attribute("block.protocol.version", i64::from(header.version()));

        span.set_attribute("block.commitments.kernel", header.kernel_root());
        span.set_attribute("block.commitments.nullifier", header.nullifier_root());
        span.set_attribute("block.commitments.account", header.account_root());
        span.set_attribute("block.commitments.chain", header.chain_root());
        span.set_attribute("block.commitments.note", header.note_root());
        span.set_attribute("block.commitments.transaction", header.tx_hash());
    }
}
