use std::sync::Arc;

use anyhow::Result;
use miden_crypto::utils::Deserializable;
use miden_node_proto::{
    block_producer::api_server, requests::SubmitProvenTransactionRequest,
    responses::SubmitProvenTransactionResponse,
};
use miden_objects::transaction::ProvenTransaction;
use tonic::Status;
use tracing::{debug, info, instrument};

use crate::{txqueue::TransactionQueue, COMPONENT};

// BLOCK PRODUCER
// ================================================================================================

pub struct BlockProducerApi<T> {
    queue: Arc<T>,
}

impl<T> BlockProducerApi<T>
where
    T: TransactionQueue,
{
    pub fn new(queue: Arc<T>) -> Self {
        Self { queue }
    }
}

#[tonic::async_trait]
impl<T> api_server::Api for BlockProducerApi<T>
where
    T: TransactionQueue,
{
    #[allow(clippy::blocks_in_conditions)] // Workaround of `instrument` issue
    #[instrument(skip_all, err, fields(COMPONENT))]
    async fn submit_proven_transaction(
        &self,
        request: tonic::Request<SubmitProvenTransactionRequest>,
    ) -> Result<tonic::Response<SubmitProvenTransactionResponse>, Status> {
        let request = request.into_inner();
        debug!(?request.transaction, COMPONENT, "Submitting proven transaction");

        let tx = ProvenTransaction::read_from_bytes(&request.transaction)
            .map_err(|_| Status::invalid_argument("Invalid transaction"))?;

        info!(
            tx_id = ?tx.id(),
            account_id = ?tx.account_id(),
            initial_account_hash = ?tx.initial_account_hash(),
            final_account_hash = ?tx.final_account_hash(),
            input_notes = ?tx.input_notes(),
            output_notes = ?tx.output_notes(),
            tx_script_root = ?tx.tx_script_root(),
            block_ref = ?tx.block_ref(),
            COMPONENT,
            "Submitting proven transaction",
        );
        debug!(proof = ?tx.proof(), COMPONENT, "Submitting proven transaction");

        self.queue
            .add_transaction(Arc::new(tx))
            .await
            .map_err(|err| Status::invalid_argument(format!("{:?}", err)))?;

        Ok(tonic::Response::new(SubmitProvenTransactionResponse {}))
    }
}
