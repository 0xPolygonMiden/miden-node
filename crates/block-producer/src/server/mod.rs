use std::{
    collections::{BTreeMap, BTreeSet},
    net::ToSocketAddrs,
    sync::Arc,
};

use miden_node_proto::{
    domain::nullifiers,
    generated::{
        block_producer::api_server, requests::SubmitProvenTransactionRequest,
        responses::SubmitProvenTransactionResponse, store::api_client as store_client,
    },
};
use miden_node_utils::{
    errors::ApiError,
    formatting::{format_input_notes, format_output_notes},
};
use miden_objects::{
    transaction::ProvenTransaction, utils::serde::Deserializable, MIN_PROOF_SECURITY_LEVEL,
};
use miden_tx::TransactionVerifier;
use tokio::{net::TcpListener, sync::Mutex};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::Status;
use tracing::{debug, info, instrument};

use crate::{
    batch_builder::{DefaultBatchBuilder, DefaultBatchBuilderOptions},
    block_builder::DefaultBlockBuilder,
    config::BlockProducerConfig,
    errors::AddTransactionErrorRework,
    mempool::Mempool,
    state_view::DefaultStateView,
    store::{DefaultStore, Store},
    transaction::VerifiedTransaction,
    txqueue::{TransactionQueue, TransactionQueueOptions},
    COMPONENT, SERVER_BATCH_SIZE, SERVER_BLOCK_FREQUENCY, SERVER_BUILD_BATCH_FREQUENCY,
    SERVER_MAX_BATCHES_PER_BLOCK,
};

pub mod api;

type Api = api::BlockProducerApi<
    DefaultBatchBuilder<
        DefaultStore,
        DefaultBlockBuilder<DefaultStore, DefaultStateView<DefaultStore>>,
    >,
    DefaultStateView<DefaultStore>,
>;

/// Represents an initialized block-producer component where the RPC connection is open,
/// but not yet actively responding to requests.
///
/// Separating the connection binding from the server spawning allows the caller to connect other
/// components to the store without resorting to sleeps or other mechanisms to spawn dependent
/// components.
pub struct BlockProducer {
    api_service: api_server::ApiServer<Api>,
    listener: TcpListener,
}

impl BlockProducer {
    /// Performs all expensive initialization tasks, and notably begins listening on the rpc
    /// endpoint without serving the API yet. Incoming requests will be queued until
    /// [`serve`](Self::serve) is called.
    pub async fn init(config: BlockProducerConfig) -> Result<Self, ApiError> {
        info!(target: COMPONENT, %config, "Initializing server");

        let store = Arc::new(DefaultStore::new(
            store_client::ApiClient::connect(config.store_url.to_string())
                .await
                .map_err(|err| ApiError::DatabaseConnectionFailed(err.to_string()))?,
        ));
        let state_view =
            Arc::new(DefaultStateView::new(Arc::clone(&store), config.verify_tx_proofs));

        let block_builder = DefaultBlockBuilder::new(Arc::clone(&store), Arc::clone(&state_view));
        let batch_builder_options = DefaultBatchBuilderOptions {
            block_frequency: SERVER_BLOCK_FREQUENCY,
            max_batches_per_block: SERVER_MAX_BATCHES_PER_BLOCK,
        };
        let batch_builder = Arc::new(DefaultBatchBuilder::new(
            Arc::clone(&store),
            Arc::new(block_builder),
            batch_builder_options,
        ));

        let transaction_queue_options = TransactionQueueOptions {
            build_batch_frequency: SERVER_BUILD_BATCH_FREQUENCY,
            batch_size: SERVER_BATCH_SIZE,
        };
        let queue = Arc::new(TransactionQueue::new(
            state_view,
            Arc::clone(&batch_builder),
            transaction_queue_options,
        ));

        let api_service =
            api_server::ApiServer::new(api::BlockProducerApi::new(Arc::clone(&queue)));

        tokio::spawn(async move { queue.run().await });
        tokio::spawn(async move { batch_builder.run().await });

        let addr = config
            .endpoint
            .to_socket_addrs()
            .map_err(ApiError::EndpointToSocketFailed)?
            .next()
            .ok_or_else(|| ApiError::AddressResolutionFailed(config.endpoint.to_string()))?;

        let listener = TcpListener::bind(addr).await?;

        info!(target: COMPONENT, "Server initialized");

        Ok(Self { api_service, listener })
    }

    /// Serves the block-producers's RPC API.
    ///
    /// Note: this blocks until the server dies.
    pub async fn serve(self) -> Result<(), ApiError> {
        tonic::transport::Server::builder()
            .add_service(self.api_service)
            .serve_with_incoming(TcpListenerStream::new(self.listener))
            .await
            .map_err(ApiError::ApiServeFailed)
    }
}

pub struct Server {
    /// This outer mutex enforces that the incoming transactions won't crowd out more important
    /// mempool locks.
    ///
    /// The inner mutex will be abstracted away once we are happy with the api.
    mempool: Mutex<Arc<Mutex<Mempool>>>,

    store: DefaultStore,
}

// FIXME: remove the allow when the upstream clippy issue is fixed:
// https://github.com/rust-lang/rust-clippy/issues/12281
#[allow(clippy::blocks_in_conditions)]
#[tonic::async_trait]
impl api_server::Api for Server {
    async fn submit_proven_transaction(
        &self,
        request: tonic::Request<SubmitProvenTransactionRequest>,
    ) -> Result<tonic::Response<SubmitProvenTransactionResponse>, Status> {
        self.submit_proven_transaction(request.into_inner())
            .await
            .map(tonic::Response::new)
            .map_err(|err| match err {
                AddTransactionErrorRework::InvalidAccountState { .. }
                | AddTransactionErrorRework::AuthenticatedNoteNotFound(_)
                | AddTransactionErrorRework::UnauthenticatedNoteNotFound(_)
                | AddTransactionErrorRework::NotesAlreadyConsumed(_)
                | AddTransactionErrorRework::DeserializationError(_)
                | AddTransactionErrorRework::DuplicateOutputNotes(_)
                | AddTransactionErrorRework::ProofVerificationFailed(_) => {
                    Status::invalid_argument(err.to_string())
                },
                AddTransactionErrorRework::StaleInputs { .. }
                | AddTransactionErrorRework::NoteAuthenticationError(..)
                | AddTransactionErrorRework::TxInputsError(_) => Status::internal("Internal error"),
            })
    }
}

impl Server {
    #[instrument(
        target = "miden-block-producer",
        name = "block_producer:submit_proven_transaction",
        skip_all,
        err
    )]
    async fn submit_proven_transaction(
        &self,
        request: SubmitProvenTransactionRequest,
    ) -> Result<SubmitProvenTransactionResponse, AddTransactionErrorRework> {
        debug!(target: COMPONENT, ?request);

        let tx = ProvenTransaction::read_from_bytes(&request.transaction)
            .map_err(|err| AddTransactionErrorRework::DeserializationError(err.to_string()))?;

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

        let mut inputs = self.store.get_tx_inputs(&tx).await?;
        let unauthenticated_notes =
            tx.get_unauthenticated_notes().map(|note| note.id()).collect::<BTreeSet<_>>();
        let auth = self.store.get_note_authentication_info(unauthenticated_notes.iter()).await?;

        // SAFETY: We assume that the RPC component verifies the transaction proof.
        let mut tx = VerifiedTransaction::new_unchecked(tx);

        // Pre-check that all nullifiers are uncomsumed in the store.
        //
        // This will be checked again in the mempool against inflight nullifiers, but this
        // way we early reject transactions without locking the mempool.
        let nullifiers_already_spent = tx
            .nullifiers()
            .iter()
            .filter_map(|nullifier| {
                inputs.nullifiers.get(nullifier).copied().flatten().map(|_| *nullifier)
            })
            .collect::<BTreeSet<_>>();
        if !nullifiers_already_spent.is_empty() {
            return Err(AddTransactionErrorRework::NotesAlreadyConsumed(nullifiers_already_spent));
        }

        // Upgrade unauthenticated notes for which we now have witnesses.
        //
        // This prevents having to re-witness them later, saving on database IO.
        for (note_id, (block_witness, note_witness)) in auth.note_proofs() {
            if !tx.witness_note(note_id, block_witness, note_witness) {
                tracing::warn!(note=%note_id, "Received a witness for a note that was not unauthenticated.");
            }
        }

        self.mempool
            .lock()
            .await
            .lock()
            .await
            .add_transaction(tx, inputs)
            .map(|block_height| SubmitProvenTransactionResponse { block_height })
    }
}
