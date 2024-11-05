use std::net::ToSocketAddrs;

use miden_node_proto::generated::{
    block_producer::api_server, requests::SubmitProvenTransactionRequest,
    responses::SubmitProvenTransactionResponse, store::api_client as store_client,
};
use miden_node_utils::{
    errors::ApiError,
    formatting::{format_input_notes, format_output_notes},
};
use miden_objects::{transaction::ProvenTransaction, utils::serde::Deserializable};
use tokio::{net::TcpListener, sync::Mutex};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::Status;
use tracing::{debug, info, instrument};

use crate::{
    batch_builder::BatchBuilder,
    block_builder::BlockBuilder,
    config::BlockProducerConfig,
    domain::transaction::AuthenticatedTransaction,
    errors::{AddTransactionError, VerifyTxError},
    mempool::{BatchBudget, BlockBudget, BlockNumber, MempoolBuilder, SharedMempool},
    store::{DefaultStore, Store},
    COMPONENT, SERVER_MEMPOOL_STATE_RETENTION,
};

/// Represents an initialized block-producer component where the RPC connection is open,
/// but not yet actively responding to requests.
///
/// Separating the connection binding from the server spawning allows the caller to connect other
/// components to the store without resorting to sleeps or other mechanisms to spawn dependent
/// components.
pub struct BlockProducer {
    batch_builder: BatchBuilder,
    block_builder: BlockBuilder,
    batch_limits: BatchBudget,
    block_limits: BlockBudget,
    state_retention: usize,
    rpc_listener: TcpListener,
    store: DefaultStore,
    chain_tip: BlockNumber,
}

impl BlockProducer {
    /// Performs all expensive initialization tasks, and notably begins listening on the rpc
    /// endpoint without serving the API yet. Incoming requests will be queued until
    /// [`serve`](Self::serve) is called.
    pub async fn init(config: BlockProducerConfig) -> Result<Self, ApiError> {
        info!(target: COMPONENT, %config, "Initializing server");

        // TODO: Does this actually need an arc to be properly shared?
        let store = DefaultStore::new(
            store_client::ApiClient::connect(config.store_url.to_string())
                .await
                .map_err(|err| ApiError::DatabaseConnectionFailed(err.to_string()))?,
        );

        let latest_header = store
            .latest_header()
            .await
            .map_err(|err| ApiError::DatabaseConnectionFailed(err.to_string()))?;
        let chain_tip = BlockNumber::new(latest_header.block_num());

        let rpc_listener = config
            .endpoint
            .to_socket_addrs()
            .map_err(ApiError::EndpointToSocketFailed)?
            .next()
            .ok_or_else(|| ApiError::AddressResolutionFailed(config.endpoint.to_string()))
            .map(TcpListener::bind)?
            .await?;

        info!(target: COMPONENT, "Server initialized");

        Ok(Self {
            batch_builder: Default::default(),
            block_builder: BlockBuilder::new(store.clone()),
            batch_limits: Default::default(),
            block_limits: Default::default(),
            state_retention: SERVER_MEMPOOL_STATE_RETENTION,
            store,
            rpc_listener,
            chain_tip,
        })
    }

    pub async fn serve(self) -> Result<(), ApiError> {
        let Self {
            batch_builder,
            block_builder,
            batch_limits,
            block_limits,
            state_retention,
            rpc_listener,
            store,
            chain_tip,
        } = self;

        let mempool = MempoolBuilder {
            block_limits,
            committed_state_retention: state_retention,
            batch_limits,
        }
        .build_shared(chain_tip);

        // Spawn rpc server and batch and block provers.
        //
        // These communicate indirectly via a shared mempool.
        //
        // These should run forever, so we combine them into a joinset so that if
        // any complete or fail, we can shutdown the rest (somewhat) gracefully.
        let mut tasks = tokio::task::JoinSet::new();

        // TODO: improve the error situationship.
        let batch_builder_id = tasks
            .spawn({
                let mempool = mempool.clone();
                async { batch_builder.run(mempool).await }
            })
            .id();
        let block_builder_id = tasks
            .spawn({
                let mempool = mempool.clone();
                async { block_builder.run(mempool).await }
            })
            .id();
        let rpc_id = tasks
            .spawn(async move {
                BlockProducerRpcServer::new(mempool, store)
                    .serve(rpc_listener)
                    .await
                    .expect("Really the rest should throw errors instead of panic'ing.")
            })
            .id();

        // Wait for any task to end. They should run forever, so this is an unexpected result.

        // SAFETY: The JoinSet is definitely not empty.
        let task_result = tasks.join_next_with_id().await.unwrap();
        let task_id = match &task_result {
            Ok((id, _)) => *id,
            Err(err) => err.id(),
        };

        let task_name = match task_id {
            id if id == batch_builder_id => "batch-builder",
            id if id == block_builder_id => "block-builder",
            id if id == rpc_id => "rpc",
            _ => {
                tracing::warn!("An unknown task ID was detected in the block-producer.");
                "unknown"
            },
        };

        tracing::error!(
            task = task_name,
            result = ?task_result,
            "Block-producer task ended unexpectedly, aborting"
        );

        tasks.abort_all();

        Ok(())
    }
}

/// Serves the block producer's RPC [api](api_server::Api).
struct BlockProducerRpcServer {
    /// The mutex effectively rate limits incoming transactions into the mempool by forcing them
    /// through a queue.
    ///
    /// This gives mempool users such as the batch and block builders equal footing with __all__
    /// incoming transactions combined. Without this incoming transactions would greatly restrict
    /// the block-producers usage of the mempool.
    mempool: Mutex<SharedMempool>,

    store: DefaultStore,
}

#[tonic::async_trait]
impl api_server::Api for BlockProducerRpcServer {
    async fn submit_proven_transaction(
        &self,
        request: tonic::Request<SubmitProvenTransactionRequest>,
    ) -> Result<tonic::Response<SubmitProvenTransactionResponse>, Status> {
        self.submit_proven_transaction(request.into_inner())
            .await
            .map(tonic::Response::new)
            // This Status::from mapping takes care of hiding internal errors.
            .map_err(Into::into)
    }
}

impl BlockProducerRpcServer {
    pub fn new(mempool: SharedMempool, store: DefaultStore) -> Self {
        Self { mempool: Mutex::new(mempool), store }
    }

    async fn serve(self, listener: TcpListener) -> Result<(), tonic::transport::Error> {
        tonic::transport::Server::builder()
            .add_service(api_server::ApiServer::new(self))
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
    }

    #[instrument(
        target = "miden-block-producer",
        name = "block_producer:submit_proven_transaction",
        skip_all,
        err
    )]
    async fn submit_proven_transaction(
        &self,
        request: SubmitProvenTransactionRequest,
    ) -> Result<SubmitProvenTransactionResponse, AddTransactionError> {
        debug!(target: COMPONENT, ?request);

        let tx = ProvenTransaction::read_from_bytes(&request.transaction)
            .map_err(|err| AddTransactionError::DeserializationError(err.to_string()))?;

        let tx_id = tx.id();

        info!(
            target: COMPONENT,
            tx_id = %tx_id.to_hex(),
            account_id = %tx.account_id().to_hex(),
            initial_account_hash = %tx.account_update().init_state_hash(),
            final_account_hash = %tx.account_update().final_state_hash(),
            input_notes = %format_input_notes(tx.input_notes()),
            output_notes = %format_output_notes(tx.output_notes()),
            block_ref = %tx.block_ref(),
            "Deserialized transaction"
        );
        debug!(target: COMPONENT, proof = ?tx.proof());

        let inputs = self.store.get_tx_inputs(&tx).await.map_err(VerifyTxError::from)?;

        // SAFETY: we assume that the rpc component has verified the transaction proof already.
        let tx = AuthenticatedTransaction::new(tx, inputs)?;

        self.mempool
            .lock()
            .await
            .lock()
            .await
            .add_transaction(tx)
            .map(|block_height| SubmitProvenTransactionResponse { block_height })
    }
}
