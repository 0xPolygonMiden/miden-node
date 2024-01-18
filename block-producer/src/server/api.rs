use std::sync::Arc;

use anyhow::Result;
use miden_crypto::utils::Deserializable;
use miden_node_proto::{
    block_producer::api_server, requests::SubmitProvenTransactionRequest,
    responses::SubmitProvenTransactionResponse,
};
use miden_node_utils::logging::gen_request_id;
use miden_objects::transaction::ProvenTransaction;
use tonic::Status;
use tracing::{debug, info_span};

use crate::txqueue::TransactionQueue;

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
    async fn submit_proven_transaction(
        &self,
        request: tonic::Request<SubmitProvenTransactionRequest>,
    ) -> Result<tonic::Response<SubmitProvenTransactionResponse>, Status> {
        let request = request.into_inner();

        let request_id = gen_request_id();
        let _span = info_span!("submit_proven_transaction", request_id, ?request);

        let tx = ProvenTransaction::read_from_bytes(&request.transaction)
            .map_err(|_| Status::invalid_argument("Invalid transaction"))?;

        debug!(request_id, ?tx);

        self.queue
            .add_transaction(Arc::new(tx))
            .await
            .map_err(|err| Status::invalid_argument(format!("{:?}", err)))?;

        Ok(tonic::Response::new(SubmitProvenTransactionResponse {}))
    }
}
