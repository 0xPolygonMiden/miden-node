use std::sync::Arc;

use miden_node_proto::generated::{
    block_producer::api_server, requests::SubmitProvenTransactionRequest,
    responses::SubmitProvenTransactionResponse,
};
use miden_node_utils::formatting::{format_input_notes, format_output_notes};
use miden_objects::{transaction::ProvenTransaction, utils::serde::Deserializable};
use tonic::Status;
use tracing::{debug, info, instrument};

use crate::{
    batch_builder::BatchBuilder,
    txqueue::{TransactionQueue, TransactionValidator},
    COMPONENT,
};

// BLOCK PRODUCER
// ================================================================================================

pub struct BlockProducerApi<BB, TV> {
    queue: Arc<TransactionQueue<BB, TV>>,
}

impl<BB, TV> BlockProducerApi<BB, TV> {
    pub fn new(queue: Arc<TransactionQueue<BB, TV>>) -> Self {
        Self { queue }
    }
}

// FIXME: remove the allow when the upstream clippy issue is fixed:
// https://github.com/rust-lang/rust-clippy/issues/12281
#[allow(clippy::blocks_in_conditions)]
#[tonic::async_trait]
impl<BB, TV> api_server::Api for BlockProducerApi<BB, TV>
where
    TV: TransactionValidator,
    BB: BatchBuilder,
{
    #[instrument(
        target = "miden-block-producer",
        name = "block_producer:submit_proven_transaction",
        skip_all,
        err
    )]
    async fn submit_proven_transaction(
        &self,
        request: tonic::Request<SubmitProvenTransactionRequest>,
    ) -> Result<tonic::Response<SubmitProvenTransactionResponse>, Status> {
        let request = request.into_inner();
        debug!(target: COMPONENT, ?request);

        let tx = ProvenTransaction::read_from_bytes(&request.transaction)
            .map_err(|_| Status::invalid_argument("Invalid transaction"))?;

        info!(
            target: COMPONENT,
            tx_id = %tx.id().to_hex(),
            account_id = %tx.account_id().to_hex(),
            initial_account_hash = %tx.account_update().init_state_hash(),
            final_account_hash = %tx.account_update().final_state_hash(),
            input_notes = %format_input_notes(tx.input_notes()),
            output_notes = %format_output_notes(tx.output_notes()),
            block_ref = %tx.block_ref(),
            "Deserialized transaction"
        );
        debug!(target: COMPONENT, proof = ?tx.proof());

        let block_height = self
            .queue
            .add_transaction(tx)
            .await
            .map_err(|err| Status::invalid_argument(format!("{:?}", err)))?
            .ok_or(Status::internal("Missing block height"))?;

        Ok(tonic::Response::new(SubmitProvenTransactionResponse { block_height }))
    }
}
