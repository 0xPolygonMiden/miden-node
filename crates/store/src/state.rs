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
    domain::{
        account::{AccountInfo, AccountProofRequest, StorageMapKeysProof},
        batch::BatchInputs,
    },
    generated::responses::{AccountProofsResponse, AccountStateHeader, StorageSlotMapProof},
};
use miden_node_utils::formatting::format_array;
use miden_objects::{
    ACCOUNT_TREE_DEPTH, AccountError,
    account::{AccountDelta, AccountHeader, AccountId, StorageSlot},
    block::{AccountWitness, BlockHeader, BlockInputs, BlockNumber, NullifierWitness, ProvenBlock},
    crypto::{
        hash::rpo::RpoDigest,
        merkle::{
            LeafIndex, Mmr, MmrDelta, MmrError, MmrPeaks, MmrProof, PartialMmr, SimpleSmt,
            SmtProof, ValuePath,
        },
    },
    note::{NoteId, Nullifier},
    transaction::{ChainMmr, OutputNote},
    utils::Serializable,
};
use tokio::{
    sync::{Mutex, RwLock, oneshot},
    time::Instant,
};
use tracing::{info, info_span, instrument};

use crate::{
    COMPONENT,
    blocks::BlockStore,
    db::{Db, NoteRecord, NoteSyncUpdate, NullifierInfo, Page, StateSyncUpdate},
    errors::{
        ApplyBlockError, DatabaseError, GetBatchInputsError, GetBlockHeaderError,
        GetBlockInputsError, InvalidBlockError, NoteSyncError, StateInitializationError,
        StateSyncError,
    },
    nullifier_tree::NullifierTree,
};
// STRUCTURES
// ================================================================================================

#[derive(Debug)]
pub struct TransactionInputs {
    pub account_commitment: RpoDigest,
    pub nullifiers: Vec<NullifierInfo>,
    pub found_unauthenticated_notes: BTreeSet<NoteId>,
}

/// A [Merkle Mountain Range](Mmr) defining a chain of blocks.
#[derive(Debug, Clone)]
pub struct Blockchain(Mmr);

impl Blockchain {
    /// Returns a new Blockchain.
    pub fn new(chain_mmr: Mmr) -> Self {
        Self(chain_mmr)
    }

    /// Returns the tip of the chain, i.e. the number of the latest block in the chain.
    pub fn chain_tip(&self) -> BlockNumber {
        let block_number: u32 = (self.0.forest() - 1)
            .try_into()
            .expect("chain_mmr always has, at least, the genesis block");

        block_number.into()
    }

    /// Returns the chain length.
    pub fn chain_length(&self) -> BlockNumber {
        self.chain_tip().child()
    }

    /// Returns the current peaks of the MMR.
    pub fn peaks(&self) -> MmrPeaks {
        self.0.peaks()
    }

    /// Returns the peaks of the MMR at the state specified by `forest`.
    ///
    /// # Errors
    ///
    /// Returns an error if the specified `forest` value is not valid for this MMR.
    pub fn peaks_at(&self, forest: usize) -> Result<MmrPeaks, MmrError> {
        self.0.peaks_at(forest)
    }

    /// Adds a block commitment to the MMR. The caller must ensure that this commitent is the one
    /// for the next block in the chain.
    pub fn push(&mut self, block_commitment: RpoDigest) {
        self.0.add(block_commitment);
    }

    /// Returns an [`MmrProof`] for the leaf at the specified position.
    pub fn open(&self, pos: usize) -> Result<MmrProof, MmrError> {
        self.0.open_at(pos, self.0.forest())
    }

    /// Returns a reference to the underlying [`Mmr`].
    pub fn as_mmr(&self) -> &Mmr {
        &self.0
    }

    /// Creates a [`PartialMmr`] at the state of the latest block (i.e. the block's chain root will
    /// match the hashed peaks of the returned partial MMR). This MMR will include authentication
    /// paths for all blocks in the provided set.
    pub fn partial_mmr_from_blocks(
        &self,
        blocks: &BTreeSet<BlockNumber>,
        latest_block_number: BlockNumber,
    ) -> PartialMmr {
        // Using latest block as the target forest means we take the state of the MMR one before
        // the latest block. This is because the latest block will be used as the reference
        // block of the batch and will be added to the MMR by the batch kernel.
        let target_forest = latest_block_number.as_usize();
        let peaks = self
            .peaks_at(target_forest)
            .expect("target_forest should be smaller than forest of the chain mmr");
        // Grab the block merkle paths from the inner state.
        let mut partial_mmr = PartialMmr::from_peaks(peaks);

        for block_num in blocks.iter().map(BlockNumber::as_usize) {
            // SAFETY: We have ensured block nums are less than chain length.
            let leaf = self
                .0
                .get(block_num)
                .expect("block num less than chain length should exist in chain mmr");
            let path = self
                .0
                .open_at(block_num, target_forest)
                .expect("block num and target forest should be valid for this mmr")
                .merkle_path;
            // SAFETY: We should be able to fill the partial MMR with data from the chain MMR
            // without errors, otherwise it indicates the chain mmr is invalid.
            partial_mmr
                .track(block_num, leaf, &path)
                .expect("filling partial mmr with data from mmr should succeed");
        }

        partial_mmr
    }
}

/// Container for state that needs to be updated atomically.
struct InnerState {
    nullifier_tree: NullifierTree,
    blockchain: Blockchain,
    account_tree: SimpleSmt<ACCOUNT_TREE_DEPTH>,
}

impl InnerState {
    /// Returns the latest block number.
    fn latest_block_num(&self) -> BlockNumber {
        self.blockchain.chain_tip()
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
    #[instrument(target = COMPONENT, skip_all)]
    pub async fn load(
        mut db: Db,
        block_store: Arc<BlockStore>,
    ) -> Result<Self, StateInitializationError> {
        let nullifier_tree = load_nullifier_tree(&mut db).await?;
        let chain_mmr = load_mmr(&mut db).await?;
        let account_tree = load_accounts(&mut db).await?;

        let inner = RwLock::new(InnerState {
            nullifier_tree,
            blockchain: Blockchain::new(chain_mmr),
            account_tree,
        });

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
    #[instrument(target = COMPONENT, skip_all, err)]
    pub async fn apply_block(&self, block: ProvenBlock) -> Result<(), ApplyBlockError> {
        let _lock = self.writer.try_lock().map_err(|_| ApplyBlockError::ConcurrentWrite)?;

        let header = block.header();

        let tx_commitment = BlockHeader::compute_tx_commitment(block.transactions());
        if header.tx_commitment() != tx_commitment {
            return Err(InvalidBlockError::InvalidBlockTxCommitment {
                expected: tx_commitment,
                actual: header.tx_commitment(),
            }
            .into());
        }

        let block_num = header.block_num();
        let block_commitment = block.commitment();

        // ensures the right block header is being processed
        let prev_block = self
            .db
            .select_block_header_by_block_num(None)
            .await?
            .ok_or(ApplyBlockError::DbBlockHeaderEmpty)?;

        if block_num != prev_block.block_num() + 1 {
            return Err(InvalidBlockError::NewBlockInvalidBlockNum.into());
        }
        if header.prev_block_commitment() != prev_block.commitment() {
            return Err(InvalidBlockError::NewBlockInvalidPrevCommitment.into());
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
                .created_nullifiers()
                .iter()
                .filter(|&n| inner.nullifier_tree.get_block_num(n).is_some())
                .copied()
                .collect();
            if !duplicate_nullifiers.is_empty() {
                return Err(InvalidBlockError::DuplicatedNullifiers(duplicate_nullifiers).into());
            }

            // compute updates for the in-memory data structures

            // new_block.chain_root must be equal to the chain MMR root prior to the update
            let peaks = inner.blockchain.peaks();
            if peaks.hash_peaks() != header.chain_commitment() {
                return Err(InvalidBlockError::NewBlockInvalidChainCommitment.into());
            }

            // compute update for nullifier tree
            let nullifier_tree_update = inner.nullifier_tree.compute_mutations(
                block.created_nullifiers().iter().map(|nullifier| (*nullifier, block_num)),
            );

            if nullifier_tree_update.root() != header.nullifier_root() {
                return Err(InvalidBlockError::NewBlockInvalidNullifierRoot.into());
            }

            // compute update for account tree
            let account_tree_update = inner.account_tree.compute_mutations(
                block.updated_accounts().iter().map(|update| {
                    (
                        LeafIndex::new_max_depth(update.account_id().prefix().into()),
                        update.final_state_commitment().into(),
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
        let note_tree = block.build_output_note_tree();
        if note_tree.root() != header.note_root() {
            return Err(InvalidBlockError::NewBlockInvalidNoteRoot.into());
        }

        let notes = block
            .output_notes()
            .map(|(note_index, note)| {
                let (details, nullifier) = match note {
                    OutputNote::Full(note) => (Some(note.to_bytes()), Some(note.nullifier())),
                    OutputNote::Header(_) => (None, None),
                    note @ OutputNote::Partial(_) => {
                        return Err(InvalidBlockError::InvalidOutputNoteType(Box::new(
                            note.clone(),
                        )));
                    },
                };

                let merkle_path = note_tree.get_note_path(note_index);

                let note_record = NoteRecord {
                    block_num,
                    note_index,
                    note_id: note.id().into(),
                    metadata: *note.metadata(),
                    details,
                    merkle_path,
                };

                Ok((note_record, nullifier))
            })
            .collect::<Result<Vec<_>, InvalidBlockError>>()?;

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
            inner.blockchain.push(block_commitment);
        }

        info!(%block_commitment, block_num = block_num.as_u32(), COMPONENT, "apply_block successful");

        Ok(())
    }

    /// Queries a [BlockHeader] from the database, and returns it alongside its inclusion proof.
    ///
    /// If [None] is given as the value of `block_num`, the data for the latest [BlockHeader] is
    /// returned.
    #[instrument(target = COMPONENT, skip_all, ret(level = "debug"), err)]
    pub async fn get_block_header(
        &self,
        block_num: Option<BlockNumber>,
        include_mmr_proof: bool,
    ) -> Result<(Option<BlockHeader>, Option<MmrProof>), GetBlockHeaderError> {
        let block_header = self.db.select_block_header_by_block_num(block_num).await?;
        if let Some(header) = block_header {
            let mmr_proof = if include_mmr_proof {
                let inner = self.inner.read().await;
                let mmr_proof = inner.blockchain.open(header.block_num().as_usize())?;
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
        block_num: BlockNumber,
    ) -> Result<Vec<NullifierInfo>, DatabaseError> {
        self.db
            .select_nullifiers_by_prefix(prefix_len, nullifier_prefixes, block_num)
            .await
    }

    /// Generates membership proofs for each one of the `nullifiers` against the latest nullifier
    /// tree.
    ///
    /// Note: these proofs are invalidated once the nullifier tree is modified, i.e. on a new block.
    #[instrument(target = COMPONENT, skip_all, ret(level = "debug"))]
    pub async fn check_nullifiers(&self, nullifiers: &[Nullifier]) -> Vec<SmtProof> {
        let inner = self.inner.read().await;
        nullifiers.iter().map(|n| inner.nullifier_tree.open(n)).collect()
    }

    /// Queries a list of [`NoteRecord`] from the database.
    ///
    /// If the provided list of [`NoteId`] given is empty or no [`NoteRecord`] matches the provided
    /// [`NoteId`] an empty list is returned.
    pub async fn get_notes_by_id(
        &self,
        note_ids: Vec<NoteId>,
    ) -> Result<Vec<NoteRecord>, DatabaseError> {
        self.db.select_notes_by_id(note_ids).await
    }

    /// Fetches the inputs for a transaction batch from the database.
    ///
    /// ## Inputs
    ///
    /// The function takes as input:
    /// - The tx reference blocks are the set of blocks referenced by transactions in the batch.
    /// - The unauthenticated note ids are the set of IDs of unauthenticated notes consumed by all
    ///   transactions in the batch. For these notes, we attempt to find note inclusion proofs. Not
    ///   all notes will exist in the DB necessarily, as some notes can be created and consumed
    ///   within the same batch.
    ///
    /// ## Outputs
    ///
    /// The function will return:
    /// - A block inclusion proof for all tx reference blocks and for all blocks which are
    ///   referenced by a note inclusion proof.
    /// - Note inclusion proofs for all notes that were found in the DB.
    /// - The block header that the batch should reference, i.e. the latest known block.
    pub async fn get_batch_inputs(
        &self,
        tx_reference_blocks: BTreeSet<BlockNumber>,
        unauthenticated_note_ids: BTreeSet<NoteId>,
    ) -> Result<BatchInputs, GetBatchInputsError> {
        if tx_reference_blocks.is_empty() {
            return Err(GetBatchInputsError::TransactionBlockReferencesEmpty);
        }

        // First we grab note inclusion proofs for the known notes. These proofs only
        // prove that the note was included in a given block. We then also need to prove that
        // each of those blocks is included in the chain.
        let note_proofs = self
            .db
            .select_note_inclusion_proofs(unauthenticated_note_ids)
            .await
            .map_err(GetBatchInputsError::SelectNoteInclusionProofError)?;

        // The set of blocks that the notes are included in.
        let note_blocks = note_proofs.values().map(|proof| proof.location().block_num());

        // Collect all blocks we need to query without duplicates, which is:
        // - all blocks for which we need to prove note inclusion.
        // - all blocks referenced by transactions in the batch.
        let mut blocks: BTreeSet<BlockNumber> = tx_reference_blocks;
        blocks.extend(note_blocks);

        // Scoped block to automatically drop the read lock guard as soon as we're done.
        // We also avoid accessing the db in the block as this would delay dropping the guard.
        let (batch_reference_block, partial_mmr) = {
            let inner_state = self.inner.read().await;

            let latest_block_num = inner_state.blockchain.chain_tip();

            let highest_block_num =
                *blocks.last().expect("we should have checked for empty block references");
            if highest_block_num > latest_block_num {
                return Err(GetBatchInputsError::UnknownTransactionBlockReference {
                    highest_block_num,
                    latest_block_num,
                });
            }

            // Remove the latest block from the to-be-tracked blocks as it will be the reference
            // block for the batch itself and thus added to the MMR within the batch kernel, so
            // there is no need to prove its inclusion.
            blocks.remove(&latest_block_num);

            (
                latest_block_num,
                inner_state.blockchain.partial_mmr_from_blocks(&blocks, latest_block_num),
            )
        };

        // Fetch the reference block of the batch as part of this query, so we can avoid looking it
        // up in a separate DB access.
        let mut headers = self
            .db
            .select_block_headers(blocks.into_iter().chain(std::iter::once(batch_reference_block)))
            .await
            .map_err(GetBatchInputsError::SelectBlockHeaderError)?;

        // Find and remove the batch reference block as we don't want to add it to the chain MMR.
        let header_index = headers
            .iter()
            .enumerate()
            .find_map(|(index, header)| {
                (header.block_num() == batch_reference_block).then_some(index)
            })
            .expect("DB should have returned the header of the batch reference block");

        // The order doesn't matter for ChainMmr::new, so swap remove is fine.
        let batch_reference_block_header = headers.swap_remove(header_index);

        // SAFETY: This should not error because:
        // - we're passing exactly the block headers that we've added to the partial MMR,
        // - so none of the block headers block numbers should exceed the chain length of the
        //   partial MMR,
        // - and we've added blocks to a BTreeSet, so there can be no duplicates.
        let chain_mmr = ChainMmr::new(partial_mmr, headers)
            .expect("partial mmr and block headers should be consistent");

        Ok(BatchInputs {
            batch_reference_block_header,
            note_proofs,
            chain_mmr,
        })
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
    /// - `account_ids`: Include the account's commitment if their _last change_ was in the result's
    ///   block range.
    /// - `note_tags`: The tags the client is interested in, result is restricted to the first block
    ///   with any matches tags.
    #[instrument(target = COMPONENT, skip_all, ret(level = "debug"), err)]
    pub async fn sync_state(
        &self,
        block_num: BlockNumber,
        account_ids: Vec<AccountId>,
        note_tags: Vec<u32>,
    ) -> Result<(StateSyncUpdate, MmrDelta), StateSyncError> {
        let inner = self.inner.read().await;

        let state_sync = self.db.get_state_sync(block_num, account_ids, note_tags).await?;

        let delta = if block_num == state_sync.block_header.block_num() {
            // The client is in sync with the chain tip.
            MmrDelta {
                forest: block_num.as_usize(),
                data: vec![],
            }
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
            let from_forest = (block_num + 1).as_usize();
            let to_forest = state_sync.block_header.block_num().as_usize();
            inner
                .blockchain
                .as_mmr()
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
    #[instrument(target = COMPONENT, skip_all, ret(level = "debug"), err)]
    pub async fn sync_notes(
        &self,
        block_num: BlockNumber,
        note_tags: Vec<u32>,
    ) -> Result<(NoteSyncUpdate, MmrProof), NoteSyncError> {
        let inner = self.inner.read().await;

        let note_sync = self.db.get_note_sync(block_num, note_tags).await?;

        let mmr_proof = inner.blockchain.open(note_sync.block_header.block_num().as_usize())?;

        Ok((note_sync, mmr_proof))
    }

    /// Returns data needed by the block producer to construct and prove the next block.
    pub async fn get_block_inputs(
        &self,
        account_ids: Vec<AccountId>,
        nullifiers: Vec<Nullifier>,
        unauthenticated_notes: BTreeSet<NoteId>,
        reference_blocks: BTreeSet<BlockNumber>,
    ) -> Result<BlockInputs, GetBlockInputsError> {
        // Get the note inclusion proofs from the DB.
        // We do this first so we have to acquire the lock to the state just once. There we need the
        // reference blocks of the note proofs to get their authentication paths in the chain MMR.
        let unauthenticated_note_proofs = self
            .db
            .select_note_inclusion_proofs(unauthenticated_notes)
            .await
            .map_err(GetBlockInputsError::SelectNoteInclusionProofError)?;

        // The set of blocks that the notes are included in.
        let note_proof_reference_blocks =
            unauthenticated_note_proofs.values().map(|proof| proof.location().block_num());

        // Collect all blocks we need to prove inclusion for, without duplicates.
        let mut blocks = reference_blocks;
        blocks.extend(note_proof_reference_blocks);

        let (latest_block_number, account_witnesses, nullifier_witnesses, partial_mmr) =
            self.get_block_inputs_witnesses(&mut blocks, account_ids, nullifiers).await?;

        // Fetch the block headers for all blocks in the partial MMR plus the latest one which will
        // be used as the previous block header of the block being built.
        let mut headers = self
            .db
            .select_block_headers(blocks.into_iter().chain(std::iter::once(latest_block_number)))
            .await
            .map_err(GetBlockInputsError::SelectBlockHeaderError)?;

        // Find and remove the latest block as we must not add it to the chain MMR, since it is
        // not yet in the chain.
        let latest_block_header_index = headers
            .iter()
            .enumerate()
            .find_map(|(index, header)| {
                (header.block_num() == latest_block_number).then_some(index)
            })
            .expect("DB should have returned the header of the latest block header");

        // The order doesn't matter for ChainMmr::new, so swap remove is fine.
        let latest_block_header = headers.swap_remove(latest_block_header_index);

        // SAFETY: This should not error because:
        // - we're passing exactly the block headers that we've added to the partial MMR,
        // - so none of the block header's block numbers should exceed the chain length of the
        //   partial MMR,
        // - and we've added blocks to a BTreeSet, so there can be no duplicates.
        let chain_mmr = ChainMmr::new(partial_mmr, headers)
            .expect("partial mmr and block headers should be consistent");

        Ok(BlockInputs::new(
            latest_block_header,
            chain_mmr,
            account_witnesses,
            nullifier_witnesses,
            unauthenticated_note_proofs,
        ))
    }

    /// Get account and nullifier witnesses for the requested account IDs and nullifier as well as
    /// the [`PartialMmr`] for the given blocks. The MMR won't contain the latest block and its
    /// number is removed from `blocks` and returned separately.
    ///
    /// This method acquires the lock to the inner state and does not access the DB so we release
    /// the lock asap.
    async fn get_block_inputs_witnesses(
        &self,
        blocks: &mut BTreeSet<BlockNumber>,
        account_ids: Vec<AccountId>,
        nullifiers: Vec<Nullifier>,
    ) -> Result<
        (
            BlockNumber,
            BTreeMap<AccountId, AccountWitness>,
            BTreeMap<Nullifier, NullifierWitness>,
            PartialMmr,
        ),
        GetBlockInputsError,
    > {
        let inner = self.inner.read().await;

        let latest_block_number = inner.latest_block_num();

        // If `blocks` is empty, use the latest block number which will never trigger the error.
        let highest_block_number = blocks.last().copied().unwrap_or(latest_block_number);
        if highest_block_number > latest_block_number {
            return Err(GetBlockInputsError::UnknownBatchBlockReference {
                highest_block_number,
                latest_block_number,
            });
        }

        // The latest block is not yet in the chain MMR, so we can't (and don't need to) prove its
        // inclusion in the chain.
        blocks.remove(&latest_block_number);

        // Fetch the partial MMR at the state of the latest block with authentication paths for the
        // provided set of blocks.
        let partial_mmr = inner.blockchain.partial_mmr_from_blocks(blocks, latest_block_number);

        // Fetch witnesses for all acounts.
        let account_witnesses = account_ids
            .iter()
            .copied()
            .map(|account_id| {
                let ValuePath {
                    value: latest_state_commitment,
                    path: proof,
                } = inner.account_tree.open(&account_id.into());
                (account_id, AccountWitness::new(latest_state_commitment, proof))
            })
            .collect::<BTreeMap<AccountId, AccountWitness>>();

        // Fetch witnesses for all nullifiers. We don't check whether the nullifiers are spent or
        // not as this is done as part of proposing the block.
        let nullifier_witnesses: BTreeMap<Nullifier, NullifierWitness> = nullifiers
            .iter()
            .copied()
            .map(|nullifier| {
                let proof = inner.nullifier_tree.open(&nullifier);
                (nullifier, NullifierWitness::new(proof))
            })
            .collect();

        Ok((latest_block_number, account_witnesses, nullifier_witnesses, partial_mmr))
    }

    /// Returns data needed by the block producer to verify transactions validity.
    #[instrument(target = COMPONENT, skip_all, ret)]
    pub async fn get_transaction_inputs(
        &self,
        account_id: AccountId,
        nullifiers: &[Nullifier],
        unauthenticated_notes: Vec<NoteId>,
    ) -> Result<TransactionInputs, DatabaseError> {
        info!(target: COMPONENT, account_id = %account_id.to_string(), nullifiers = %format_array(nullifiers));

        let inner = self.inner.read().await;

        let account_commitment = inner
            .account_tree
            .open(&LeafIndex::new_max_depth(account_id.prefix().into()))
            .value;

        let nullifiers = nullifiers
            .iter()
            .map(|nullifier| NullifierInfo {
                nullifier: *nullifier,
                block_num: inner.nullifier_tree.get_block_num(nullifier).unwrap_or_default(),
            })
            .collect();

        let found_unauthenticated_notes =
            self.db.select_note_ids(unauthenticated_notes.clone()).await?;

        Ok(TransactionInputs {
            account_commitment,
            nullifiers,
            found_unauthenticated_notes,
        })
    }

    /// Returns details for public (on-chain) account.
    pub async fn get_account_details(&self, id: AccountId) -> Result<AccountInfo, DatabaseError> {
        self.db.select_account(id).await
    }

    /// Returns account proofs with optional account and storage headers.
    pub async fn get_account_proofs(
        &self,
        account_requests: Vec<AccountProofRequest>,
        known_code_commitments: BTreeSet<RpoDigest>,
        include_headers: bool,
    ) -> Result<(BlockNumber, Vec<AccountProofsResponse>), DatabaseError> {
        // Lock inner state for the whole operation. We need to hold this lock to prevent the
        // database, account tree and latest block number from changing during the operation,
        // because changing one of them would lead to inconsistent state.
        let inner_state = self.inner.read().await;

        let account_ids: Vec<AccountId> =
            account_requests.iter().map(|req| req.account_id).collect();

        let state_headers = if include_headers.not() {
            BTreeMap::<AccountId, AccountStateHeader>::default()
        } else {
            let infos = self.db.select_accounts_by_ids(account_ids.clone()).await?;
            if account_ids.len() > infos.len() {
                let found_ids = infos.iter().map(|info| info.summary.account_id).collect();
                return Err(DatabaseError::AccountsNotFoundInDb(
                    BTreeSet::from_iter(account_ids).difference(&found_ids).copied().collect(),
                ));
            }

            let mut headers_map = BTreeMap::new();

            // Iterate and build state headers for public accounts
            for request in account_requests {
                let account_info = infos
                    .iter()
                    .find(|info| info.summary.account_id == request.account_id)
                    .expect("retrieved accounts were validated against request");

                if let Some(details) = &account_info.details {
                    let mut storage_slot_map_keys = Vec::new();

                    for StorageMapKeysProof { storage_index, storage_keys } in
                        &request.storage_requests
                    {
                        if let Some(StorageSlot::Map(storage_map)) =
                            details.storage().slots().get(*storage_index as usize)
                        {
                            for map_key in storage_keys {
                                let proof = storage_map.open(map_key);

                                let slot_map_key = StorageSlotMapProof {
                                    storage_slot: u32::from(*storage_index),
                                    smt_proof: proof.to_bytes(),
                                };
                                storage_slot_map_keys.push(slot_map_key);
                            }
                        } else {
                            return Err(AccountError::StorageSlotNotMap(*storage_index).into());
                        }
                    }

                    // Only include unknown account codes
                    let account_code = known_code_commitments
                        .contains(&details.code().commitment())
                        .not()
                        .then(|| details.code().to_bytes());

                    let state_header = AccountStateHeader {
                        header: Some(AccountHeader::from(details).into()),
                        storage_header: details.storage().get_header().to_bytes(),
                        account_code,
                        storage_maps: storage_slot_map_keys,
                    };

                    headers_map.insert(account_info.summary.account_id, state_header);
                }
            }

            headers_map
        };

        let responses = account_ids
            .into_iter()
            .map(|account_id| {
                let acc_leaf_idx = LeafIndex::new_max_depth(account_id.prefix().into());
                let opening = inner_state.account_tree.open(&acc_leaf_idx);
                let state_header = state_headers.get(&account_id).cloned();

                AccountProofsResponse {
                    account_id: Some(account_id.into()),
                    account_commitment: Some(opening.value.into()),
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
    ) -> Result<Option<AccountDelta>, DatabaseError> {
        self.db.select_account_state_delta(account_id, from_block, to_block).await
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

    /// Runs database optimization.
    pub async fn optimize_db(&self) -> Result<(), DatabaseError> {
        self.db.optimize().await
    }

    /// Returns the unprocessed network notes, along with the next pagination token.
    pub async fn get_unconsumed_network_notes(
        &self,
        page: Page,
    ) -> Result<(Vec<NoteRecord>, Page), DatabaseError> {
        self.db.select_unconsumed_network_notes(page).await
    }
}

// UTILITIES
// ================================================================================================

#[instrument(target = COMPONENT, skip_all)]
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

#[instrument(target = COMPONENT, skip_all)]
async fn load_mmr(db: &mut Db) -> Result<Mmr, StateInitializationError> {
    let block_commitments: Vec<RpoDigest> = db
        .select_all_block_headers()
        .await?
        .iter()
        .map(BlockHeader::commitment)
        .collect();

    Ok(block_commitments.into())
}

#[instrument(target = COMPONENT, skip_all)]
async fn load_accounts(
    db: &mut Db,
) -> Result<SimpleSmt<ACCOUNT_TREE_DEPTH>, StateInitializationError> {
    let account_data: Vec<_> = db
        .select_all_account_commitments()
        .await?
        .into_iter()
        .map(|(id, account_commitment)| (id.prefix().into(), account_commitment.into()))
        .collect();

    SimpleSmt::with_leaves(account_data)
        .map_err(StateInitializationError::FailedToCreateAccountsTree)
}
