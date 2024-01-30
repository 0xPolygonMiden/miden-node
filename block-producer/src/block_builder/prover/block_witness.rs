use std::collections::{BTreeMap, BTreeSet};

use miden_crypto::ZERO;
use miden_node_proto::domain::BlockInputs;
use miden_objects::{
    accounts::AccountId,
    crypto::merkle::{EmptySubtreeRoots, MerkleStore, MmrPeaks},
    BlockHeader, Digest, Felt,
};
use miden_vm::{crypto::MerklePath, AdviceInputs, StackInputs};

use crate::{
    block_builder::errors::{BlockProverError, BuildBlockError},
    TransactionBatch, CREATED_NOTES_SMT_DEPTH,
};

// CONSTANTS
// =================================================================================================

/// The depth at which we insert roots from the batches.
pub(crate) const CREATED_NOTES_TREE_INSERTION_DEPTH: u8 = 8;

/// The depth of the created notes tree in the block.
pub(crate) const CREATED_NOTES_TREE_DEPTH: u8 =
    CREATED_NOTES_TREE_INSERTION_DEPTH + CREATED_NOTES_SMT_DEPTH;

pub(crate) const MAX_BATCHES_PER_BLOCK: usize =
    2_usize.pow(CREATED_NOTES_TREE_INSERTION_DEPTH as u32);

// BLOCK WITNESS
// =================================================================================================

/// Provides inputs to the `BlockKernel` so that it can generate the new header.
#[derive(Debug, PartialEq)]
pub struct BlockWitness {
    pub(super) updated_accounts: BTreeMap<AccountId, AccountUpdate>,
    /// (batch_index, created_notes_root) for batches that contain notes
    pub(super) batch_created_notes_roots: BTreeMap<usize, Digest>,
    pub(super) chain_peaks: MmrPeaks,
    pub(super) prev_header: BlockHeader,
}

impl BlockWitness {
    pub fn new(
        block_inputs: BlockInputs,
        batches: &[TransactionBatch],
    ) -> Result<Self, BuildBlockError> {
        Self::validate_inputs(&block_inputs, batches)?;

        let updated_accounts = {
            let mut account_initial_states: BTreeMap<AccountId, Digest> =
                batches.iter().flat_map(|batch| batch.account_initial_states()).collect();

            let mut account_merkle_proofs: BTreeMap<AccountId, MerklePath> = block_inputs
                .account_states
                .into_iter()
                .map(|record| (record.account_id, record.proof))
                .collect();

            batches
                .iter()
                .flat_map(|batch| batch.updated_accounts())
                .map(|(account_id, final_state_hash)| {
                    let initial_state_hash = account_initial_states
                        .remove(&account_id)
                        .expect("already validated that key exists");
                    let proof = account_merkle_proofs
                        .remove(&account_id)
                        .expect("already validated that key exists");

                    (
                        account_id,
                        AccountUpdate {
                            initial_state_hash,
                            final_state_hash,
                            proof,
                        },
                    )
                })
                .collect()
        };

        let batch_created_notes_roots = batches
            .iter()
            .enumerate()
            .filter_map(|(batch_index, batch)| {
                if batch.created_notes().next().is_none() {
                    None
                } else {
                    Some((batch_index, batch.created_notes_root()))
                }
            })
            .collect();

        Ok(Self {
            updated_accounts,
            batch_created_notes_roots,
            chain_peaks: block_inputs.chain_peaks,
            prev_header: block_inputs.block_header,
        })
    }

    pub(super) fn into_program_inputs(
        self
    ) -> Result<(AdviceInputs, StackInputs), BlockProverError> {
        let stack_inputs = {
            // Note: `StackInputs::new()` reverses the input vector, so we need to construct the stack
            // from the bottom to the top
            let mut stack_inputs = Vec::new();

            // Chain MMR stack inputs
            {
                stack_inputs.extend(self.prev_header.hash());
                stack_inputs.extend(self.chain_peaks.hash_peaks());
            }

            // Notes stack inputs
            {
                let num_created_notes_roots = self.batch_created_notes_roots.len();
                for (batch_index, batch_created_notes_root) in self.batch_created_notes_roots.iter()
                {
                    stack_inputs.extend(batch_created_notes_root.iter());

                    let batch_index = u64::try_from(*batch_index)
                        .expect("can't be more than 2^64 - 1 notes created");
                    stack_inputs.push(Felt::from(batch_index));
                }

                let empty_root = EmptySubtreeRoots::entry(CREATED_NOTES_TREE_DEPTH, 0);
                stack_inputs.extend(*empty_root);
                stack_inputs.push(Felt::from(
                    u64::try_from(num_created_notes_roots)
                        .expect("can't be more than 2^64 - 1 notes created"),
                ));
            }

            // Account stack inputs
            let mut num_accounts_updated: u64 = 0;
            for (idx, (&account_id, account_update)) in self.updated_accounts.iter().enumerate() {
                stack_inputs.push(account_id.into());
                stack_inputs.extend(account_update.final_state_hash);

                let idx = u64::try_from(idx).expect("can't be more than 2^64 - 1 accounts");
                num_accounts_updated = idx + 1;
            }

            // append initial account root
            stack_inputs.extend(self.prev_header.account_root());

            // append number of accounts updated
            stack_inputs.push(num_accounts_updated.into());

            StackInputs::new(stack_inputs)
        };

        let advice_inputs = {
            let mut merkle_store = MerkleStore::default();
            merkle_store
                .add_merkle_paths(self.updated_accounts.into_iter().map(
                    |(
                        account_id,
                        AccountUpdate {
                            initial_state_hash,
                            final_state_hash: _,
                            proof,
                        },
                    )| { (u64::from(account_id), initial_state_hash, proof) },
                ))
                .map_err(BlockProverError::InvalidMerklePaths)?;

            let mut advice_inputs = AdviceInputs::default().with_merkle_store(merkle_store);
            add_mmr_peaks_to_advice_inputs(&self.chain_peaks, &mut advice_inputs);

            advice_inputs
        };

        Ok((advice_inputs, stack_inputs))
    }

    // HELPERS
    // ---------------------------------------------------------------------------------------------

    fn validate_inputs(
        block_inputs: &BlockInputs,
        batches: &[TransactionBatch],
    ) -> Result<(), BuildBlockError> {
        // TODO:
        // - Block height returned for each nullifier is 0.

        if batches.len() > MAX_BATCHES_PER_BLOCK {
            return Err(BuildBlockError::TooManyBatchesInBlock(batches.len()));
        }

        Self::validate_account_states(block_inputs, batches)?;

        Ok(())
    }

    /// Validate that initial account states coming from the batches are the same as the account
    /// states returned from the store
    fn validate_account_states(
        block_inputs: &BlockInputs,
        batches: &[TransactionBatch],
    ) -> Result<(), BuildBlockError> {
        let batches_initial_states: BTreeMap<AccountId, Digest> =
            batches.iter().flat_map(|batch| batch.account_initial_states()).collect();

        let accounts_in_batches: BTreeSet<AccountId> =
            batches_initial_states.keys().cloned().collect();
        let accounts_in_store: BTreeSet<AccountId> = block_inputs
            .account_states
            .iter()
            .map(|record| &record.account_id)
            .cloned()
            .collect();

        if accounts_in_batches == accounts_in_store {
            let accounts_with_different_hashes: Vec<AccountId> = block_inputs
                .account_states
                .iter()
                .filter_map(|record| {
                    let hash_in_store = record.account_hash;
                    let hash_in_batches = batches_initial_states
                        .get(&record.account_id)
                        .expect("we already verified that account id is contained in batches");

                    if hash_in_store == *hash_in_batches {
                        None
                    } else {
                        Some(record.account_id)
                    }
                })
                .collect();

            if accounts_with_different_hashes.is_empty() {
                Ok(())
            } else {
                Err(BuildBlockError::InconsistentAccountStates(accounts_with_different_hashes))
            }
        } else {
            // The batches and store don't modify the same set of accounts
            let union: BTreeSet<AccountId> =
                accounts_in_batches.union(&accounts_in_store).cloned().collect();
            let intersection: BTreeSet<AccountId> =
                accounts_in_batches.intersection(&accounts_in_store).cloned().collect();

            let difference: Vec<AccountId> = union.difference(&intersection).cloned().collect();

            Err(BuildBlockError::InconsistentAccountIds(difference))
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct AccountUpdate {
    pub initial_state_hash: Digest,
    pub final_state_hash: Digest,
    pub proof: MerklePath,
}

// HELPERS
// =================================================================================================

/// insert MMR peaks info into the advice map
/// FIXME: This was copy/pasted from `miden-lib`'s `add_chain_mmr_to_advice_inputs`. Once we move the block kernel
/// into `miden-lib`, we should use that function instead
fn add_mmr_peaks_to_advice_inputs(
    peaks: &MmrPeaks,
    inputs: &mut AdviceInputs,
) {
    let mut elements = vec![Felt::new(peaks.num_leaves() as u64), ZERO, ZERO, ZERO];
    elements.extend(peaks.flatten_and_pad_peaks());
    inputs.extend_map([(peaks.hash_peaks().into(), elements)]);
}
