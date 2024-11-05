//! Abstraction to synchronize state modifications.
//!
//! The [State] provides data access and modifications methods, its main purpose is to ensure that
//! data is atomically written, and that reads are consistent.

use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Not,
    sync::Arc,
};

use miden_node_proto::{
    convert,
    domain::{accounts::AccountInfo, blocks::BlockInclusionProof, notes::NoteAuthenticationInfo},
    generated::responses::{AccountProofsResponse, AccountStateHeader, GetBlockInputsResponse},
    AccountInputRecord, NullifierWitness,
};
use miden_node_utils::formatting::{format_account_id, format_array};
use miden_objects::{
    accounts::{AccountDelta, AccountHeader},
    block::Block,
    crypto::{
        hash::rpo::RpoDigest,
        merkle::{
            LeafIndex, Mmr, MmrDelta, MmrError, MmrPeaks, MmrProof, SimpleSmt, SmtProof, ValuePath,
        },
    },
    notes::{NoteId, Nullifier},
    transaction::OutputNote,
    utils::Serializable,
    AccountError, BlockHeader, ACCOUNT_TREE_DEPTH,
};
use tokio::{
    sync::{oneshot, Mutex, RwLock},
    time::Instant,
};
use tracing::{info, info_span, instrument};

use crate::{
    blocks::BlockStore,
    db::{Db, NoteRecord, NoteSyncUpdate, NullifierInfo, StateSyncUpdate},
    errors::{
        ApplyBlockError, DatabaseError, GetBlockHeaderError, GetBlockInputsError,
        GetNoteInclusionProofError, InvalidBlockError, NoteSyncError, StateInitializationError,
        StateSyncError,
    },
    nullifier_tree::NullifierTree,
    types::{AccountId, BlockNumber},
    COMPONENT,
};
// STRUCTURES
// ================================================================================================

/// Information needed from the store to validate and build a block
#[derive(Debug)]
pub struct BlockInputs {
    /// Previous block header
    pub block_header: BlockHeader,

    /// MMR peaks for the current chain state
    pub chain_peaks: MmrPeaks,

    /// The hashes of the requested accounts and their authentication paths
    pub account_states: Vec<AccountInputRecord>,

    /// The requested nullifiers and their authentication paths
    pub nullifiers: Vec<NullifierWitness>,

    /// List of notes found in the store
    pub found_unauthenticated_notes: NoteAuthenticationInfo,
}

impl From<BlockInputs> for GetBlockInputsResponse {
    fn from(value: BlockInputs) -> Self {
        Self {
            block_header: Some(value.block_header.into()),
            mmr_peaks: convert(value.chain_peaks.peaks()),
            account_states: convert(value.account_states),
            nullifiers: convert(value.nullifiers),
            found_unauthenticated_notes: Some(value.found_unauthenticated_notes.into()),
        }
    }
}

#[derive(Debug)]
pub struct TransactionInputs {
    pub account_hash: RpoDigest,
    pub nullifiers: Vec<NullifierInfo>,
    pub missing_unauthenticated_notes: Vec<NoteId>,
}

/// Container for state that needs to be updated atomically.
struct InnerState {
    nullifier_tree: NullifierTree,
    chain_mmr: Mmr,
    account_tree: SimpleSmt<ACCOUNT_TREE_DEPTH>,
}

impl InnerState {
    /// Returns the latest block number.
    fn latest_block_num(&self) -> BlockNumber {
        (self.chain_mmr.forest() - 1)
            .try_into()
            .expect("chain_mmr always has, at least, the genesis block")
    }
}

/// The rollup state
pub struct State {
    /// The database which stores block headers, nullifiers, notes, and the latest states of
    /// accounts.
    db: Arc<Db>,

    /// The block store which stores full block contents for all blocks.
    block_store: Arc<BlockStore>,

    /// Read-write lock used to prevent writing to a structure while it is being used.
    ///
    /// The lock is writer-preferring, meaning the writer won't be starved.
    inner: RwLock<InnerState>,

    /// To allow readers to access the tree data while an update in being performed, and prevent
    /// TOCTOU issues, there must be no concurrent writers. This locks to serialize the writers.
    writer: Mutex<()>,
}

impl State {
    /// Loads the state from the `db`.
    #[instrument(target = "miden-store", skip_all)]
    pub async fn load(
        mut db: Db,
        block_store: Arc<BlockStore>,
    ) -> Result<Self, StateInitializationError> {
        let nullifier_tree = load_nullifier_tree(&mut db).await?;
        let chain_mmr = load_mmr(&mut db).await?;
        let account_tree = load_accounts(&mut db).await?;

        let inner = RwLock::new(InnerState { nullifier_tree, chain_mmr, account_tree });

        let writer = Mutex::new(());
        let db = Arc::new(db);

        Ok(Self { db, block_store, inner, writer })
    }

    /// Apply changes of a new block to the DB and in-memory data structures.
    ///
    /// ## Note on state consistency
    ///
    /// The server contains in-memory representations of the existing trees, the in-memory
    /// representation must be kept consistent with the committed data, this is necessary so to
    /// provide consistent results for all endpoints. In order to achieve consistency, the
    /// following steps are used:
    ///
    /// - the request data is validated, prior to starting any modifications.
    /// - block is being saved into the store in parallel with updating the DB, but before
    ///   committing. This block is considered as candidate and not yet available for reading
    ///   because the latest block pointer is not updated yet.
    /// - a transaction is open in the DB and the writes are started.
    /// - while the transaction is not committed, concurrent reads are allowed, both the DB and the
    ///   in-memory representations, which are consistent at this stage.
    /// - prior to committing the changes to the DB, an exclusive lock to the in-memory data is
    ///   acquired, preventing concurrent reads to the in-memory data, since that will be
    ///   out-of-sync w.r.t. the DB.
    /// - the DB transaction is committed, and requests that read only from the DB can proceed to
    ///   use the fresh data.
    /// - the in-memory structures are updated, including the latest block pointer and the lock is
    ///   released.
    // TODO: This span is logged in a root span, we should connect it to the parent span.
    #[instrument(target = "miden-store", skip_all, err)]
    pub async fn apply_block(&self, block: Block) -> Result<(), ApplyBlockError> {
        let _lock = self.writer.try_lock().map_err(|_| ApplyBlockError::ConcurrentWrite)?;

        let header = block.header();

        let tx_hash = block.compute_tx_hash();
        if header.tx_hash() != tx_hash {
            return Err(InvalidBlockError::InvalidTxHash {
                expected: tx_hash,
                actual: header.tx_hash(),
            }
            .into());
        }

        let block_num = header.block_num();
        let block_hash = block.hash();

        // ensures the right block header is being processed
        let prev_block = self
            .db
            .select_block_header_by_block_num(None)
            .await?
            .ok_or(ApplyBlockError::DbBlockHeaderEmpty)?;

        if block_num != prev_block.block_num() + 1 {
            return Err(InvalidBlockError::NewBlockInvalidBlockNum.into());
        }
        if header.prev_hash() != prev_block.hash() {
            return Err(InvalidBlockError::NewBlockInvalidPrevHash.into());
        }

        let block_data = block.to_bytes();

        // Save the block to the block store. In a case of a rolled-back DB transaction, the
        // in-memory state will be unchanged, but the block might still be written into the
        // block store. Thus, such block should be considered as block candidates, but not
        // finalized blocks. So we should check for the latest block when getting block from
        // the store.
        let store = Arc::clone(&self.block_store);
        let block_save_task =
            tokio::spawn(async move { store.save_block(block_num, &block_data).await });

        // scope to read in-memory data, compute mutations required for updating account
        // and nullifier trees, and validate the request
        let (
            nullifier_tree_old_root,
            nullifier_tree_update,
            account_tree_old_root,
            account_tree_update,
        ) = {
            let inner = self.inner.read().await;

            let _span = info_span!(target: COMPONENT, "update_in_memory_structs").entered();

            // nullifiers can be produced only once
            let duplicate_nullifiers: Vec<_> = block
                .nullifiers()
                .iter()
                .filter(|&n| inner.nullifier_tree.get_block_num(n).is_some())
                .cloned()
                .collect();
            if !duplicate_nullifiers.is_empty() {
                return Err(InvalidBlockError::DuplicatedNullifiers(duplicate_nullifiers).into());
            }

            // compute updates for the in-memory data structures

            // new_block.chain_root must be equal to the chain MMR root prior to the update
            let peaks = inner.chain_mmr.peaks();
            if peaks.hash_peaks() != header.chain_root() {
                return Err(InvalidBlockError::NewBlockInvalidChainRoot.into());
            }

            // compute update for nullifier tree
            let nullifier_tree_update = inner.nullifier_tree.compute_mutations(
                block.nullifiers().iter().map(|nullifier| (*nullifier, block_num)),
            );

            if nullifier_tree_update.root() != header.nullifier_root() {
                return Err(InvalidBlockError::NewBlockInvalidNullifierRoot.into());
            }

            // compute update for account tree
            let account_tree_update = inner.account_tree.compute_mutations(
                block.updated_accounts().iter().map(|update| {
                    (
                        LeafIndex::new_max_depth(update.account_id().into()),
                        update.new_state_hash().into(),
                    )
                }),
            );

            if account_tree_update.root() != header.account_root() {
                return Err(InvalidBlockError::NewBlockInvalidAccountRoot.into());
            }

            (
                inner.nullifier_tree.root(),
                nullifier_tree_update,
                inner.account_tree.root(),
                account_tree_update,
            )
        };

        // build note tree
        let note_tree = block.build_note_tree();
        if note_tree.root() != header.note_root() {
            return Err(InvalidBlockError::NewBlockInvalidNoteRoot.into());
        }

        let notes = block
            .notes()
            .map(|(note_index, note)| {
                let details = match note {
                    OutputNote::Full(note) => Some(note.to_bytes()),
                    OutputNote::Header(_) => None,
                    note => {
                        return Err(InvalidBlockError::InvalidOutputNoteType(Box::new(
                            note.clone(),
                        )))
                    },
                };

                let merkle_path = note_tree.get_note_path(note_index);

                Ok(NoteRecord {
                    block_num,
                    note_index,
                    note_id: note.id().into(),
                    metadata: *note.metadata(),
                    details,
                    merkle_path,
                })
            })
            .collect::<Result<Vec<NoteRecord>, InvalidBlockError>>()?;

        // Signals the transaction is ready to be committed, and the write lock can be acquired
        let (allow_acquire, acquired_allowed) = oneshot::channel::<()>();
        // Signals the write lock has been acquired, and the transaction can be committed
        let (inform_acquire_done, acquire_done) = oneshot::channel::<()>();

        // The DB and in-memory state updates need to be synchronized and are partially
        // overlapping. Namely, the DB transaction only proceeds after this task acquires the
        // in-memory write lock. This requires the DB update to run concurrently, so a new task is
        // spawned.
        let db = Arc::clone(&self.db);
        let db_update_task =
            tokio::spawn(
                async move { db.apply_block(allow_acquire, acquire_done, block, notes).await },
            );

        // Wait for the message from the DB update task, that we ready to commit the DB transaction
        acquired_allowed.await.map_err(ApplyBlockError::ClosedChannel)?;

        // Awaiting the block saving task to complete without errors
        block_save_task.await??;

        // Scope to update the in-memory data
        {
            // We need to hold the write lock here to prevent inconsistency between the in-memory
            // state and the DB state. Thus, we need to wait for the DB update task to complete
            // successfully.
            let mut inner = self.inner.write().await;

            // We need to check that neither the nullifier tree nor the account tree have changed
            // while we were waiting for the DB preparation task to complete. If either of them
            // did change, we do not proceed with in-memory and database updates, since it may
            // lead to an inconsistent state.
            if inner.nullifier_tree.root() != nullifier_tree_old_root
                || inner.account_tree.root() != account_tree_old_root
            {
                return Err(ApplyBlockError::ConcurrentWrite);
            }

            // Notify the DB update task that the write lock has been acquired, so it can commit
            // the DB transaction
            inform_acquire_done
                .send(())
                .map_err(|_| ApplyBlockError::DbUpdateTaskFailed("Receiver was dropped".into()))?;

            // TODO: shutdown #91
            // Await for successful commit of the DB transaction. If the commit fails, we mustn't
            // change in-memory state, so we return a block applying error and don't proceed with
            // in-memory updates.
            db_update_task
                .await?
                .map_err(|err| ApplyBlockError::DbUpdateTaskFailed(err.to_string()))?;

            // Update the in-memory data structures after successful commit of the DB transaction
            inner
                .nullifier_tree
                .apply_mutations(nullifier_tree_update)
                .expect("Unreachable: old nullifier tree root must be checked before this step");
            inner
                .account_tree
                .apply_mutations(account_tree_update)
                .expect("Unreachable: old account tree root must be checked before this step");
            inner.chain_mmr.add(block_hash);
        }

        info!(%block_hash, block_num, COMPONENT, "apply_block successful");

        Ok(())
    }

    /// Queries a [BlockHeader] from the database, and returns it alongside its inclusion proof.
    ///
    /// If [None] is given as the value of `block_num`, the data for the latest [BlockHeader] is
    /// returned.
    #[instrument(target = "miden-store", skip_all, ret(level = "debug"), err)]
    pub async fn get_block_header(
        &self,
        block_num: Option<BlockNumber>,
        include_mmr_proof: bool,
    ) -> Result<(Option<BlockHeader>, Option<MmrProof>), GetBlockHeaderError> {
        let block_header = self.db.select_block_header_by_block_num(block_num).await?;
        if let Some(header) = block_header {
            let mmr_proof = if include_mmr_proof {
                let inner = self.inner.read().await;
                let mmr_proof = inner.chain_mmr.open(header.block_num() as usize)?;
                Some(mmr_proof)
            } else {
                None
            };
            Ok((Some(header), mmr_proof))
        } else {
            Ok((None, None))
        }
    }

    pub async fn check_nullifiers_by_prefix(
        &self,
        prefix_len: u32,
        nullifier_prefixes: Vec<u32>,
    ) -> Result<Vec<NullifierInfo>, DatabaseError> {
        self.db.select_nullifiers_by_prefix(prefix_len, nullifier_prefixes).await
    }

    /// Generates membership proofs for each one of the `nullifiers` against the latest nullifier
    /// tree.
    ///
    /// Note: these proofs are invalidated once the nullifier tree is modified, i.e. on a new block.
    #[instrument(target = "miden-store", skip_all, ret(level = "debug"))]
    pub async fn check_nullifiers(&self, nullifiers: &[Nullifier]) -> Vec<SmtProof> {
        let inner = self.inner.read().await;
        nullifiers.iter().map(|n| inner.nullifier_tree.open(n)).collect()
    }

    /// Queries a list of [NoteRecord] from the database.
    ///
    /// If the provided list of [NoteId] given is empty or no [NoteRecord] matches the provided
    /// [NoteId] an empty list is returned.
    pub async fn get_notes_by_id(
        &self,
        note_ids: Vec<NoteId>,
    ) -> Result<Vec<NoteRecord>, DatabaseError> {
        self.db.select_notes_by_id(note_ids).await
    }

    /// Queries all the note inclusion proofs matching a certain Note IDs from the database.
    pub async fn get_note_authentication_info(
        &self,
        note_ids: BTreeSet<NoteId>,
    ) -> Result<NoteAuthenticationInfo, GetNoteInclusionProofError> {
        // First we grab block-inclusion proofs for the known notes. These proofs only
        // prove that the note was included in a given block. We then also need to prove that
        // each of those blocks is included in the chain.
        let note_proofs = self.db.select_note_inclusion_proofs(note_ids).await?;

        // The set of blocks that the notes are included in.
        let blocks = note_proofs
            .values()
            .map(|proof| proof.location().block_num())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        // Grab the block merkle paths from the inner state.
        //
        // NOTE: Scoped block to automatically drop the mutex guard asap.
        //
        // We also avoid accessing the db in the block as this would delay
        // dropping the guard.
        let (chain_length, merkle_paths) = {
            let state = self.inner.read().await;
            let chain_length = state.chain_mmr.forest();

            let paths = blocks
                .iter()
                .map(|&block_num| {
                    let proof = state.chain_mmr.open(block_num as usize)?.merkle_path;

                    Ok::<_, MmrError>((block_num, proof))
                })
                .collect::<Result<BTreeMap<_, _>, MmrError>>()?;

            let chain_length = BlockNumber::try_from(chain_length)
                .expect("Forest is a chain length so should fit into block number");

            (chain_length, paths)
        };

        let headers = self.db.select_block_headers(blocks).await?;
        let headers = headers
            .into_iter()
            .map(|header| (header.block_num(), header))
            .collect::<BTreeMap<BlockNumber, _>>();

        let mut block_proofs = Vec::with_capacity(merkle_paths.len());
        for (block_num, mmr_path) in merkle_paths {
            let block_header =
                *headers.get(&block_num).ok_or(DatabaseError::BlockNotFoundInDb(block_num))?;

            block_proofs.push(BlockInclusionProof { block_header, mmr_path, chain_length });
        }

        Ok(NoteAuthenticationInfo { block_proofs, note_proofs })
    }

    /// Loads data to synchronize a client.
    ///
    /// The client's request contains a list of tag prefixes, this method will return the first
    /// block with a matching tag, or the chain tip. All the other values are filter based on this
    /// block range.
    ///
    /// # Arguments
    ///
    /// - `block_num`: The last block *known* by the client, updates start from the next block.
    /// - `account_ids`: Include the account's hash if their _last change_ was in the result's block
    ///   range.
    /// - `note_tags`: The tags the client is interested in, result is restricted to the first block
    ///   with any matches tags.
    /// - `nullifier_prefixes`: Only the 16 high bits of the nullifiers the client is interested in,
    ///   results will include nullifiers matching prefixes produced in the given block range.
    #[instrument(target = "miden-store", skip_all, ret(level = "debug"), err)]
    pub async fn sync_state(
        &self,
        block_num: BlockNumber,
        account_ids: Vec<AccountId>,
        note_tags: Vec<u32>,
        nullifier_prefixes: Vec<u32>,
    ) -> Result<(StateSyncUpdate, MmrDelta), StateSyncError> {
        let inner = self.inner.read().await;

        let state_sync = self
            .db
            .get_state_sync(block_num, account_ids, note_tags, nullifier_prefixes)
            .await?;

        let delta = if block_num == state_sync.block_header.block_num() {
            // The client is in sync with the chain tip.
            MmrDelta { forest: block_num as usize, data: vec![] }
        } else {
            // Important notes about the boundary conditions:
            //
            // - The Mmr forest is 1-indexed whereas the block number is 0-indexed. The Mmr root
            // contained in the block header always lag behind by one block, this is because the Mmr
            // leaves are hashes of block headers, and we can't have self-referential hashes. These
            // two points cancel out and don't require adjusting.
            // - Mmr::get_delta is inclusive, whereas the sync_state request block_num is defined to
            //   be
            // exclusive, so the from_forest has to be adjusted with a +1
            let from_forest = (block_num + 1) as usize;
            let to_forest = state_sync.block_header.block_num() as usize;
            inner
                .chain_mmr
                .get_delta(from_forest, to_forest)
                .map_err(StateSyncError::FailedToBuildMmrDelta)?
        };

        Ok((state_sync, delta))
    }

    /// Loads data to synchronize a client's notes.
    ///
    /// The client's request contains a list of tags, this method will return the first
    /// block with a matching tag, or the chain tip. All the other values are filter based on this
    /// block range.
    ///
    /// # Arguments
    ///
    /// - `block_num`: The last block *known* by the client, updates start from the next block.
    /// - `note_tags`: The tags the client is interested in, resulting notes are restricted to the
    ///   first block containing a matching note.
    #[instrument(target = "miden-store", skip_all, ret(level = "debug"), err)]
    pub async fn sync_notes(
        &self,
        block_num: BlockNumber,
        note_tags: Vec<u32>,
    ) -> Result<(NoteSyncUpdate, MmrProof), NoteSyncError> {
        let inner = self.inner.read().await;

        let note_sync = self.db.get_note_sync(block_num, note_tags).await?;

        let mmr_proof = inner.chain_mmr.open(note_sync.block_header.block_num() as usize)?;

        Ok((note_sync, mmr_proof))
    }

    /// Returns data needed by the block producer to construct and prove the next block.
    pub async fn get_block_inputs(
        &self,
        account_ids: &[AccountId],
        nullifiers: &[Nullifier],
        unauthenticated_notes: BTreeSet<NoteId>,
    ) -> Result<BlockInputs, GetBlockInputsError> {
        let inner = self.inner.read().await;

        let latest = self
            .db
            .select_block_header_by_block_num(None)
            .await?
            .ok_or(GetBlockInputsError::DbBlockHeaderEmpty)?;

        // sanity check
        if inner.chain_mmr.forest() != latest.block_num() as usize + 1 {
            return Err(GetBlockInputsError::IncorrectChainMmrForestNumber {
                forest: inner.chain_mmr.forest(),
                block_num: latest.block_num(),
            });
        }

        // using current block number gets us the peaks of the chain MMR as of one block ago;
        // this is done so that latest.chain_root matches the returned peaks
        let chain_peaks =
            inner.chain_mmr.peaks_at(latest.block_num() as usize).map_err(|error| {
                GetBlockInputsError::FailedToGetMmrPeaksForForest {
                    forest: latest.block_num() as usize,
                    error,
                }
            })?;
        let account_states = account_ids
            .iter()
            .cloned()
            .map(|account_id| {
                let ValuePath { value: account_hash, path: proof } =
                    inner.account_tree.open(&LeafIndex::new_max_depth(account_id));
                Ok(AccountInputRecord {
                    account_id: account_id.try_into()?,
                    account_hash,
                    proof,
                })
            })
            .collect::<Result<_, AccountError>>()?;

        let nullifiers: Vec<NullifierWitness> = nullifiers
            .iter()
            .map(|nullifier| {
                let proof = inner.nullifier_tree.open(nullifier);

                NullifierWitness { nullifier: *nullifier, proof }
            })
            .collect();

        let found_unauthenticated_notes =
            self.get_note_authentication_info(unauthenticated_notes).await?;

        Ok(BlockInputs {
            block_header: latest,
            chain_peaks,
            account_states,
            nullifiers,
            found_unauthenticated_notes,
        })
    }

    /// Returns data needed by the block producer to verify transactions validity.
    #[instrument(target = "miden-store", skip_all, ret)]
    pub async fn get_transaction_inputs(
        &self,
        account_id: AccountId,
        nullifiers: &[Nullifier],
        unauthenticated_notes: Vec<NoteId>,
    ) -> Result<TransactionInputs, DatabaseError> {
        info!(target: COMPONENT, account_id = %format_account_id(account_id), nullifiers = %format_array(nullifiers));

        let inner = self.inner.read().await;

        let account_hash = inner.account_tree.open(&LeafIndex::new_max_depth(account_id)).value;

        let nullifiers = nullifiers
            .iter()
            .map(|nullifier| NullifierInfo {
                nullifier: *nullifier,
                block_num: inner.nullifier_tree.get_block_num(nullifier).unwrap_or_default(),
            })
            .collect();

        let found_unauthenticated_notes =
            self.db.select_note_ids(unauthenticated_notes.clone()).await?;

        let missing_unauthenticated_notes = unauthenticated_notes
            .iter()
            .filter(|note_id| !found_unauthenticated_notes.contains(note_id))
            .copied()
            .collect();

        Ok(TransactionInputs {
            account_hash,
            nullifiers,
            missing_unauthenticated_notes,
        })
    }

    /// Lists all known nullifiers with their inclusion blocks, intended for testing.
    pub async fn list_nullifiers(&self) -> Result<Vec<(Nullifier, u32)>, DatabaseError> {
        self.db.select_all_nullifiers().await
    }

    /// Lists all known accounts, with their ids, latest state hash, and block at which the account
    /// was last modified, intended for testing.
    pub async fn list_accounts(&self) -> Result<Vec<AccountInfo>, DatabaseError> {
        self.db.select_all_accounts().await
    }

    /// Lists all known notes, intended for testing.
    pub async fn list_notes(&self) -> Result<Vec<NoteRecord>, DatabaseError> {
        self.db.select_all_notes().await
    }

    /// Returns details for public (on-chain) account.
    pub async fn get_account_details(&self, id: AccountId) -> Result<AccountInfo, DatabaseError> {
        self.db.select_account(id).await
    }

    /// Returns account proofs with optional account and storage headers.
    pub async fn get_account_proofs(
        &self,
        account_ids: Vec<AccountId>,
        request_code_commitments: BTreeSet<RpoDigest>,
        include_headers: bool,
    ) -> Result<(BlockNumber, Vec<AccountProofsResponse>), DatabaseError> {
        // Lock inner state for the whole operation. We need to hold this lock to prevent the
        // database, account tree and latest block number from changing during the operation,
        // because changing one of them would lead to inconsistent state.
        let inner_state = self.inner.read().await;

        let state_headers = if !include_headers {
            BTreeMap::<AccountId, AccountStateHeader>::default()
        } else {
            let infos = self.db.select_accounts_by_ids(account_ids.clone()).await?;

            if account_ids.len() > infos.len() {
                let found_ids = infos.iter().map(|info| info.summary.account_id.into()).collect();
                return Err(DatabaseError::AccountsNotFoundInDb(
                    BTreeSet::from_iter(account_ids).difference(&found_ids).copied().collect(),
                ));
            }

            infos
                .into_iter()
                .filter_map(|info| {
                    info.details.map(|details| {
                        (
                            info.summary.account_id.into(),
                            AccountStateHeader {
                                header: Some(AccountHeader::from(&details).into()),
                                storage_header: details.storage().get_header().to_bytes(),
                                // Only include account code if the request did not contain it
                                // (known by the caller)
                                account_code: request_code_commitments
                                    .contains(&details.code().commitment())
                                    .not()
                                    .then_some(details.code().to_bytes()),
                            },
                        )
                    })
                })
                .collect()
        };

        let responses = account_ids
            .into_iter()
            .map(|account_id| {
                let acc_leaf_idx = LeafIndex::new_max_depth(account_id);
                let opening = inner_state.account_tree.open(&acc_leaf_idx);
                let state_header = state_headers.get(&account_id).cloned();

                AccountProofsResponse {
                    account_id: Some(account_id.into()),
                    account_hash: Some(opening.value.into()),
                    account_proof: Some(opening.path.into()),
                    state_header,
                }
            })
            .collect();

        Ok((inner_state.latest_block_num(), responses))
    }

    /// Returns the state delta between `from_block` (exclusive) and `to_block` (inclusive) for the
    /// given account.
    pub(crate) async fn get_account_state_delta(
        &self,
        account_id: AccountId,
        from_block: BlockNumber,
        to_block: BlockNumber,
    ) -> Result<AccountDelta, DatabaseError> {
        let deltas = self.db.select_account_state_deltas(account_id, from_block, to_block).await?;

        deltas
            .into_iter()
            .try_fold(AccountDelta::default(), |mut accumulator, delta| {
                accumulator.merge(delta).map(|_| accumulator)
            })
            .map_err(Into::into)
    }

    /// Loads a block from the block store. Return `Ok(None)` if the block is not found.
    pub async fn load_block(
        &self,
        block_num: BlockNumber,
    ) -> Result<Option<Vec<u8>>, DatabaseError> {
        if block_num > self.latest_block_num().await {
            return Ok(None);
        }
        self.block_store.load_block(block_num).await.map_err(Into::into)
    }

    /// Returns the latest block number.
    pub async fn latest_block_num(&self) -> BlockNumber {
        self.inner.read().await.latest_block_num()
    }
}

// UTILITIES
// ================================================================================================

#[instrument(target = "miden-store", skip_all)]
async fn load_nullifier_tree(db: &mut Db) -> Result<NullifierTree, StateInitializationError> {
    let nullifiers = db.select_all_nullifiers().await?;
    let len = nullifiers.len();

    let now = Instant::now();
    let nullifier_tree = NullifierTree::with_entries(nullifiers)
        .map_err(StateInitializationError::FailedToCreateNullifierTree)?;
    let elapsed = now.elapsed().as_secs();

    info!(
        num_of_leaves = len,
        tree_construction = elapsed,
        COMPONENT,
        "Loaded nullifier tree"
    );
    Ok(nullifier_tree)
}

#[instrument(target = "miden-store", skip_all)]
async fn load_mmr(db: &mut Db) -> Result<Mmr, StateInitializationError> {
    let block_hashes: Vec<RpoDigest> =
        db.select_all_block_headers().await?.iter().map(BlockHeader::hash).collect();

    Ok(block_hashes.into())
}

#[instrument(target = "miden-store", skip_all)]
async fn load_accounts(
    db: &mut Db,
) -> Result<SimpleSmt<ACCOUNT_TREE_DEPTH>, StateInitializationError> {
    let account_data: Vec<_> = db
        .select_all_account_hashes()
        .await?
        .into_iter()
        .map(|(id, account_hash)| (id, account_hash.into()))
        .collect();

    SimpleSmt::with_leaves(account_data)
        .map_err(StateInitializationError::FailedToCreateAccountsTree)
}
