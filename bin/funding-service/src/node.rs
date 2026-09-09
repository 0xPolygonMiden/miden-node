//! Node access. The RPC handling is copied from the network monitor.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use backon::ExponentialBuilder;
use miden_node_proto::clients::{Builder, RpcClient};
use miden_node_proto::domain::account::{AccountResponse, AccountVaultDetails, StorageMapEntries};
use miden_node_proto::domain::encryption::{
    TransactionInputsSealer,
    TrustedTransactionEncryptionState,
    verify_transaction_encryption_key,
};
use miden_node_proto::generated::note::NoteIdList;
use miden_node_proto::generated::rpc::{
    AccountRequest as ProtoAccountRequest,
    BlockHeaderByNumberRequest,
    BlockRange,
    FinalityLevel,
    SyncChainMmrRequest,
    SyncNotesRequest,
    SyncNullifiersRequest,
};
use miden_node_proto::generated::transaction::ProvenTransaction as ProtoProvenTransaction;
use miden_node_tracing::warn;
use miden_node_utils::retry::{self, Retryable};
use miden_protocol::Word;
use miden_protocol::account::{
    Account,
    AccountCode,
    AccountId,
    AccountStorage,
    StorageMap,
    StorageSlot,
    StorageSlotType,
};
use miden_protocol::block::account_tree::AccountWitness;
use miden_protocol::block::{BlockHeader, BlockNumber};
use miden_protocol::crypto::dsa::ecdsa_k256_keccak::PublicKey as ValidatorPublicKey;
use miden_protocol::crypto::merkle::mmr::{Forest, MmrDelta, MmrPeaks, PartialMmr};
use miden_protocol::note::{Note, NoteId, NoteInclusionProof, NoteTag, Nullifier};
use miden_protocol::transaction::{PartialBlockchain, ProvenTransaction};
use miden_protocol::utils::serde::{Deserializable, Serializable};
use tokio::sync::Mutex;
use url::Url;

use crate::COMPONENT;

// RPC NODE CLIENT
// ================================================================================================

/// Reads chain state from the node's RPC API and submits transactions to it.
#[derive(Clone)]
pub struct RpcNodeClient {
    rpc_client: RpcClient,
    genesis_header: BlockHeader,
    trusted_validator_signing_keys: Arc<[ValidatorPublicKey]>,
    sealer: Arc<Mutex<Option<TransactionInputsSealer>>>,
}

impl RpcNodeClient {
    /// Connects to the node's RPC API and verifies the attested transaction encryption key.
    pub async fn connect(
        rpc_url: &Url,
        timeout: Duration,
        trusted_validator_signing_keys: Vec<ValidatorPublicKey>,
    ) -> Result<Self> {
        anyhow::ensure!(
            !trusted_validator_signing_keys.is_empty(),
            "at least one trusted validator signing key is required to verify the transaction \
             encryption key",
        );

        let (mut rpc_client, _genesis_commitment) =
            create_genesis_aware_rpc_client(rpc_url, timeout).await?;
        let genesis_header = fetch_genesis_block_header(&mut rpc_client).await?;

        let client = Self {
            rpc_client,
            genesis_header,
            trusted_validator_signing_keys: Arc::from(trusted_validator_signing_keys),
            sealer: Arc::new(Mutex::new(None)),
        };
        // Fetch and verify the encryption key eagerly so an untrusted key fails at startup.
        client.sealer().await?;

        Ok(client)
    }

    /// The genesis block header, which commits to the chain's fee parameters.
    pub fn genesis_header(&self) -> &BlockHeader {
        &self.genesis_header
    }

    /// The committed chain tip header with a partial blockchain which proves it.
    pub async fn tip_chain_state(&self) -> Result<(BlockHeader, PartialBlockchain)> {
        fetch_tip_chain_state(&mut self.rpc_client.clone(), self.genesis_header.commitment()).await
    }

    /// A public account in full with its account-tree witness at `block_num`.
    pub async fn public_account(
        &self,
        account_id: AccountId,
        block_num: BlockNumber,
    ) -> Result<(Account, AccountWitness)> {
        fetch_public_account(&mut self.rpc_client.clone(), account_id, block_num).await
    }

    /// The chain tip which bounds whether a transaction can still be committed.
    pub async fn chain_tip(&self) -> Result<BlockNumber> {
        let status = self
            .rpc_client
            .clone()
            .status(())
            .await
            .context("failed to fetch the node status")?
            .into_inner();

        // The block producer's tip leads the store's tip, so it is the tighter bound.
        let tip = status.block_producer.map_or(status.chain_tip, |producer| producer.chain_tip);

        Ok(tip.into())
    }

    /// The inclusion proofs of the notes which are committed, keyed by note ID.
    pub async fn committed_notes(
        &self,
        note_ids: &[NoteId],
    ) -> Result<HashMap<NoteId, NoteInclusionProof>> {
        let ids = note_ids.iter().map(|note_id| note_id.as_word().into()).collect();

        let response = self
            .rpc_client
            .clone()
            .get_notes_by_id(NoteIdList { ids })
            .await
            .context("failed to fetch the funding notes from RPC")?
            .into_inner();

        response
            .notes
            .iter()
            .map(|committed| {
                let proof = committed
                    .inclusion_proof
                    .as_ref()
                    .context("committed note response is missing the inclusion proof")?;
                <(NoteId, NoteInclusionProof)>::try_from(proof)
                    .context("failed to convert the note inclusion proof")
            })
            .collect()
    }

    /// The IDs of the committed notes which carry `tag`, from `from_block` up to the chain tip.
    pub async fn sync_note_ids(
        &self,
        tag: NoteTag,
        from_block: BlockNumber,
    ) -> Result<SyncedNotes> {
        let tip = self.chain_tip().await?;
        // An empty range is rejected, and there is nothing to scan past the tip anyway.
        if from_block > tip {
            return Ok(SyncedNotes {
                note_ids: Vec::new(),
                last_checked_block: tip,
            });
        }

        let response = self
            .rpc_client
            .clone()
            .sync_notes(SyncNotesRequest {
                block_range: Some(BlockRange {
                    block_from: from_block.as_u32(),
                    block_to: tip.as_u32(),
                }),
                note_tags: vec![u32::from(tag)],
            })
            .await
            .context("failed to synchronize notes")?
            .into_inner();

        let last_checked_block = response
            .pagination_info
            .context("the sync_notes response did not include pagination information")?
            .block_num
            .into();

        let mut note_ids = Vec::new();
        for block in response.blocks {
            for record in block.notes {
                let proof = record
                    .inclusion_proof
                    .context("a note sync record did not include an inclusion proof")?;
                let note_id =
                    proof.note_id.context("a note inclusion proof did not include a note ID")?;
                note_ids
                    .push(NoteId::try_from(note_id).context("failed to convert a synced note ID")?);
            }
        }

        Ok(SyncedNotes { note_ids, last_checked_block })
    }

    /// The notes among `note_ids` whose details the node stores, with their inclusion proofs.
    pub async fn public_notes(
        &self,
        note_ids: &[NoteId],
    ) -> Result<Vec<(Note, NoteInclusionProof)>> {
        let ids = note_ids.iter().map(|note_id| note_id.as_word().into()).collect();

        let response = self
            .rpc_client
            .clone()
            .get_notes_by_id(NoteIdList { ids })
            .await
            .context("failed to fetch notes from RPC")?
            .into_inner();

        let mut notes = Vec::new();
        for committed in response.notes {
            let holds_details = committed.note.as_ref().is_some_and(|note| note.details.is_some());
            if !holds_details {
                continue;
            }

            notes.push(
                <(Note, NoteInclusionProof)>::try_from(committed)
                    .context("failed to convert a committed note")?,
            );
        }

        Ok(notes)
    }

    /// The nullifiers among `nullifiers` which are already spent.
    pub async fn spent_nullifiers(&self, nullifiers: &[Nullifier]) -> Result<HashSet<Nullifier>> {
        if nullifiers.is_empty() {
            return Ok(HashSet::new());
        }

        let tip = self.chain_tip().await?;
        // The node matches on prefixes, so the response holds every nullifier which shares a prefix
        // with one of ours. The exact matches are picked out below.
        let prefixes = nullifiers
            .iter()
            .map(|nullifier| u32::from(nullifier.prefix()))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let response = self
            .rpc_client
            .clone()
            .sync_nullifiers(SyncNullifiersRequest {
                block_range: Some(BlockRange {
                    block_from: BlockNumber::GENESIS.as_u32(),
                    block_to: tip.as_u32(),
                }),
                prefix_len: NULLIFIER_PREFIX_LEN,
                nullifiers: prefixes,
            })
            .await
            .context("failed to synchronize nullifiers")?
            .into_inner();

        let requested: HashSet<Nullifier> = nullifiers.iter().copied().collect();
        let mut spent = HashSet::new();
        for update in response.nullifiers {
            let nullifier =
                update.nullifier.context("a nullifier update did not include a nullifier")?;
            let nullifier: Nullifier =
                nullifier.try_into().context("failed to convert a nullifier")?;
            if requested.contains(&nullifier) {
                spent.insert(nullifier);
            }
        }

        Ok(spent)
    }

    /// Seals and submits one proven transaction, and returns the block it was accepted at.
    pub async fn submit(
        &self,
        proven_tx: &ProvenTransaction,
        transaction_inputs: &[u8],
    ) -> Result<BlockNumber> {
        let transaction = proven_tx.to_bytes();
        let tx_id = proven_tx.id();
        let stale_key = AtomicBool::new(false);

        let result = (|| {
            let transaction = transaction.clone();
            async {
                if stale_key.swap(false, Ordering::Relaxed) {
                    *self.sealer.lock().await = None;
                }

                let sealed = self
                    .sealer()
                    .await?
                    .seal(tx_id, transaction_inputs)
                    .context("failed to seal the transaction inputs")?;
                self.rpc_client
                    .clone()
                    .submit_proven_tx(ProtoProvenTransaction {
                        transaction,
                        sealed_transaction_inputs: Some(sealed),
                    })
                    .await
                    .context("failed to submit the proven transaction to RPC")
            }
        })
        .retry(retry::constant(Duration::ZERO, Some(1)))
        .when(|err: &anyhow::Error| {
            err.downcast_ref::<tonic::Status>()
                .is_some_and(|status| status.code() == tonic::Code::FailedPrecondition)
        })
        .notify(|status: &anyhow::Error, _| {
            stale_key.store(true, Ordering::Relaxed);
            warn!(
                status,
                target: COMPONENT,
                "Transaction inputs rejected as stale, refreshing the encryption key and retrying",
                transaction.id = tx_id
            );
        })
        .await;

        Ok(result?.into_inner().block_num.into())
    }

    /// The cached verified sealer. The attested key is fetched and checked on first use.
    async fn sealer(&self) -> Result<TransactionInputsSealer> {
        if let Some(sealer) = self.sealer.lock().await.clone() {
            return Ok(sealer);
        }

        let key = self
            .rpc_client
            .clone()
            .get_transaction_encryption_key(())
            .await
            .context("failed to fetch the transaction encryption key")?
            .into_inner();
        let verified = verify_transaction_encryption_key(
            key,
            TrustedTransactionEncryptionState::new(
                self.genesis_header.commitment(),
                &self.trusted_validator_signing_keys,
            ),
        )
        .context("untrusted transaction encryption key")?;
        let sealer = TransactionInputsSealer::new(verified);

        let mut cached = self.sealer.lock().await;
        if let Some(sealer) = cached.clone() {
            return Ok(sealer);
        }
        *cached = Some(sealer.clone());
        Ok(sealer)
    }
}

/// The only nullifier prefix length the node supports.
const NULLIFIER_PREFIX_LEN: u32 = 16;

/// The result of one note synchronization.
pub struct SyncedNotes {
    /// The notes which carry the requested tag.
    pub note_ids: Vec<NoteId>,
    /// The last block the node checked. A scan resumes from the next block.
    pub last_checked_block: BlockNumber,
}

// TRANSIENT ERRORS
// ================================================================================================

/// Returns `true` for gRPC status codes that indicate a transient transport- or server-side problem
/// worth retrying. Content-rejection codes (`InvalidArgument`, `FailedPrecondition`, ...) reflect
/// the request itself and are not retried.
pub fn is_transient_status(status: &tonic::Status) -> bool {
    matches!(
        status.code(),
        tonic::Code::Unavailable
            | tonic::Code::DeadlineExceeded
            | tonic::Code::Cancelled
            | tonic::Code::Aborted
            | tonic::Code::Unknown
            | tonic::Code::Internal
            | tonic::Code::ResourceExhausted,
    )
}

/// Returns `true` when the error chain holds a transient gRPC status.
pub fn is_transient_error(err: &anyhow::Error) -> bool {
    err.chain()
        .filter_map(|cause| cause.downcast_ref::<tonic::Status>())
        .any(is_transient_status)
}

// RPC HELPERS
// ================================================================================================

/// Backoff for the genesis-discovery handshake, so a node which is still starting does not abort
/// the service.
const GENESIS_DISCOVERY_BACKOFF_INITIAL: Duration = Duration::from_secs(1);
const GENESIS_DISCOVERY_BACKOFF_MAX: Duration = Duration::from_secs(30);
const GENESIS_DISCOVERY_MAX_RETRIES: usize = 10;

fn genesis_discovery_backoff() -> ExponentialBuilder {
    ExponentialBuilder::default()
        .with_min_delay(GENESIS_DISCOVERY_BACKOFF_INITIAL)
        .with_max_delay(GENESIS_DISCOVERY_BACKOFF_MAX)
        .with_factor(2.0)
        .with_max_times(GENESIS_DISCOVERY_MAX_RETRIES)
        .with_jitter()
}

/// Creates an RPC client configured with the correct genesis metadata in the `Accept` header so
/// that write RPCs such as `SubmitProvenTx` are accepted by the node.
async fn create_genesis_aware_rpc_client(
    rpc_url: &Url,
    timeout: Duration,
) -> Result<(RpcClient, Word)> {
    (|| async {
        // First, create a temporary client without genesis metadata to discover the genesis block
        // header and its commitment.
        let mut rpc: RpcClient = Builder::new(rpc_url.clone())
            .with_tls()
            .context("failed to configure TLS for the RPC client")?
            .with_timeout(timeout)
            .without_metadata_version()
            .without_metadata_genesis()
            .without_auth_header()
            .with_otel_context_injection()
            .connect()
            .await
            .context("failed to create an RPC client for genesis discovery")?;

        let genesis_header = fetch_genesis_block_header(&mut rpc).await?;
        let genesis_commitment = genesis_header.commitment();

        // Rebuild the client, this time including the required genesis metadata so that write RPCs
        // like SubmitProvenTx are accepted by the node.
        let rpc_client = Builder::new(rpc_url.clone())
            .with_tls()
            .context("failed to configure TLS for the RPC client")?
            .with_timeout(timeout)
            .without_metadata_version()
            .with_metadata_genesis(genesis_commitment)
            .without_auth_header()
            .with_otel_context_injection()
            .connect()
            .await
            .context("failed to connect to the RPC server with genesis metadata")?;

        Ok((rpc_client, genesis_commitment))
    })
    .retry(genesis_discovery_backoff())
    .notify(|err: &anyhow::Error, sleep: Duration| {
        warn!(
            err,
            target: COMPONENT,
            "RPC genesis discovery failed; retrying after backoff",
            retry.delay_ms = sleep.as_millis() as u64
        );
    })
    .await
}

/// Fetches the genesis block header from RPC.
async fn fetch_genesis_block_header(rpc_client: &mut RpcClient) -> Result<BlockHeader> {
    let request = BlockHeaderByNumberRequest {
        block_num: Some(BlockNumber::GENESIS.as_u32()),
        include_mmr_proof: None,
    };

    let response = rpc_client
        .get_block_header_by_number(request)
        .await
        .context("failed to get the genesis block header from RPC")?;

    let block_header = response
        .into_inner()
        .block_header
        .context("the genesis block header response holds no header")?;

    block_header.try_into().context("failed to convert the genesis block header")
}

/// Fetches the chain tip header together with a [`PartialBlockchain`] whose peaks hash to that
/// header's chain commitment, making the pair usable as a transaction reference block.
async fn fetch_tip_chain_state(
    rpc_client: &mut RpcClient,
    genesis_commitment: Word,
) -> Result<(BlockHeader, PartialBlockchain)> {
    let response = rpc_client
        .sync_chain_mmr(SyncChainMmrRequest {
            // The MMR is seeded with the genesis block below, so the delta starts at block 1.
            current_client_block_height: BlockNumber::GENESIS.as_u32(),
            finality_level: FinalityLevel::Committed.into(),
        })
        .await
        .context("failed to sync the chain MMR")?
        .into_inner();

    let tip_header: BlockHeader = response
        .block_header
        .context("the sync_chain_mmr response did not include a block header")?
        .try_into()
        .context("failed to convert the sync target block header")?;

    let delta: MmrDelta = response
        .mmr_delta
        .context("the sync_chain_mmr response did not include an MMR delta")?
        .try_into()
        .context("failed to convert the MMR delta")?;

    let mut mmr = PartialMmr::from_peaks(
        MmrPeaks::new(Forest::new(0).context("an empty forest should be valid")?, Vec::new())
            .context("empty MMR peaks should be valid")?,
    );

    if tip_header.block_num() != BlockNumber::GENESIS {
        mmr.add(genesis_commitment, false)
            .context("failed to seed the MMR with the genesis block")?;
        mmr.apply(delta).context("failed to apply the MMR delta")?;
    }

    anyhow::ensure!(
        mmr.peaks().hash_peaks() == tip_header.chain_commitment(),
        "the synced MMR peaks do not match the chain commitment of block {}",
        tip_header.block_num()
    );

    let blockchain = PartialBlockchain::new(mmr, Vec::new())
        .context("failed to build the partial blockchain")?;

    Ok((tip_header, blockchain))
}

/// Fetches a public account in full, with code, vault and storage maps, plus its account-tree
/// witness at the given block.
async fn fetch_public_account(
    rpc_client: &mut RpcClient,
    account_id: AccountId,
    block_num: BlockNumber,
) -> Result<(Account, AccountWitness)> {
    use miden_node_proto::generated::rpc::account_request::AccountDetailRequest;
    use miden_node_proto::generated::rpc::account_request::account_detail_request::StorageRequest;

    let id_bytes: [u8; 15] = account_id.into();
    // Dummy commitments force the server to include code and vault data in the response.
    let dummy: miden_node_proto::generated::primitives::Digest = Word::default().into();
    let request = ProtoAccountRequest {
        account_id: Some(miden_node_proto::generated::account::AccountId { id: id_bytes.to_vec() }),
        block_num: Some(block_num.into()),
        details: Some(AccountDetailRequest {
            code_commitment: Some(dummy),
            asset_vault_commitment: Some(dummy),
            storage_request: Some(StorageRequest::AllStorageMaps(true)),
        }),
    };

    let response = rpc_client
        .get_account(request)
        .await
        .with_context(|| format!("failed to fetch account {account_id}"))?
        .into_inner();
    let response =
        AccountResponse::try_from(response).context("failed to convert the account response")?;

    let witness = response.witness;
    anyhow::ensure!(
        witness.id() == account_id,
        "the account tree returned a witness for {} when {account_id} was requested",
        witness.id(),
    );

    let details = response
        .details
        .with_context(|| format!("no details returned for public account {account_id}"))?;

    let code = AccountCode::read_from_bytes(
        &details.account_code.context("the server did not return the account code")?,
    )
    .context("failed to deserialize the account code")?;

    let vault = match details.vault_details {
        AccountVaultDetails::Assets(assets) => {
            miden_protocol::asset::AssetVault::new(&assets).context("failed to build the vault")?
        },
        AccountVaultDetails::LimitExceeded => {
            anyhow::bail!("account {account_id} holds too many assets to fetch in full")
        },
    };

    // Value slots come from the header, map slots from the map details.
    let mut map_entries = HashMap::new();
    for map_detail in details.storage_details.map_details {
        let StorageMapEntries::AllEntries(entries) = map_detail.entries else {
            anyhow::bail!("storage map {} was not returned in full", map_detail.slot_name);
        };
        map_entries.insert(map_detail.slot_name, entries);
    }

    let mut slots = Vec::new();
    for slot in details.storage_details.header.slots() {
        match slot.slot_type() {
            StorageSlotType::Value => {
                slots.push(StorageSlot::with_value(slot.name().clone(), slot.value()));
            },
            StorageSlotType::Map => {
                let entries = map_entries.remove(slot.name()).with_context(|| {
                    format!("no map entries returned for storage slot {}", slot.name())
                })?;
                let map =
                    StorageMap::with_entries(entries).context("failed to build the storage map")?;
                anyhow::ensure!(
                    map.root() == slot.value(),
                    "the storage map root for slot {} does not match the storage header",
                    slot.name()
                );
                slots.push(StorageSlot::with_map(slot.name().clone(), map));
            },
        }
    }
    let storage = AccountStorage::new(slots).context("failed to build the account storage")?;

    let account =
        Account::new(account_id, vault, storage, code, details.account_header.nonce(), None)
            .context("failed to build the account")?;

    // The witness and the details come from one response, so a mismatch means a bad reconstruction.
    anyhow::ensure!(
        account.to_commitment() == witness.state_commitment(),
        "the reconstructed account {account_id} does not match its witness at block {block_num}",
    );

    Ok((account, witness))
}
