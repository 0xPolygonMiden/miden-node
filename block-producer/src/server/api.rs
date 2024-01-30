use std::sync::Arc;

use anyhow::Result;
use miden_crypto::utils::Deserializable;
use miden_node_proto::{
    block_producer::api_server, requests::SubmitProvenTransactionRequest,
    responses::SubmitProvenTransactionResponse,
};
use miden_node_utils::logging::{format_input_notes, format_opt, format_output_notes};
use miden_objects::transaction::ProvenTransaction;
use tonic::Status;
use tracing::{debug, info, instrument};

use crate::{
    batch_builder::BatchBuilder,
    txqueue::{TransactionQueue, TransactionVerifier},
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

#[tonic::async_trait]
impl<BB, TV> api_server::Api for BlockProducerApi<BB, TV>
where
    TV: TransactionVerifier,
    BB: BatchBuilder,
{
    #[allow(clippy::blocks_in_conditions)] // Workaround of `instrument` issue
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
            initial_account_hash = %tx.initial_account_hash(),
            final_account_hash = %tx.final_account_hash(),
            input_notes = %format_input_notes(tx.input_notes()),
            output_notes = %format_output_notes(tx.output_notes()),
            tx_script_root = %format_opt(tx.tx_script_root().as_ref()),
            block_ref = %tx.block_ref(),
            "Deserialized transaction"
        );
        debug!(target: COMPONENT, proof = ?tx.proof());

        self.queue
            .add_transaction(Arc::new(tx))
            .await
            .map_err(|err| Status::invalid_argument(format!("{:?}", err)))?;

        Ok(tonic::Response::new(SubmitProvenTransactionResponse {}))
    }
}
