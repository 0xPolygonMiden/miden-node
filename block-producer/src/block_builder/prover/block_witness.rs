use std::collections::{BTreeMap, BTreeSet};

use miden_node_proto::domain::accounts::AccountUpdateDetails;
use miden_objects::{
    accounts::AccountId,
    crypto::merkle::{EmptySubtreeRoots, MerklePath, MerkleStore, MmrPeaks, SmtProof},
    notes::Nullifier,
    vm::{AdviceInputs, StackInputs},
    BlockHeader, Digest, Felt, BLOCK_OUTPUT_NOTES_TREE_DEPTH, MAX_BATCHES_PER_BLOCK, ZERO,
};

use crate::{
    block::BlockInputs,
    errors::{BlockProverError, BuildBlockError},
    TransactionBatch,
};

// BLOCK WITNESS
// =================================================================================================

/// Provides inputs to the `BlockKernel` so that it can generate the new header.
#[derive(Debug, PartialEq)]
pub struct BlockWitness {
    pub(super) updated_accounts: BTreeMap<AccountId, AccountUpdate>,
    /// (batch_index, created_notes_root) for batches that contain notes
    pub(super) batch_created_notes_roots: BTreeMap<usize, Digest>,
    pub(super) produced_nullifiers: BTreeMap<Nullifier, SmtProof>,
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
                batches.iter().flat_map(TransactionBatch::account_initial_states).collect();

            let mut account_merkle_proofs: BTreeMap<AccountId, MerklePath> = block_inputs
                .accounts
                .into_iter()
                .map(|(account_id, witness)| (account_id, witness.proof))
                .collect();

            batches
                .iter()
                .flat_map(TransactionBatch::updated_accounts)
                .map(|AccountUpdateDetails { account_id, final_state_hash, .. }| {
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
            .filter(|(_, batch)| !batch.created_notes().is_empty())
            .map(|(batch_index, batch)| (batch_index, batch.created_notes_root()))
            .collect();

        Ok(Self {
            updated_accounts,
            batch_created_notes_roots,
            produced_nullifiers: block_inputs.nullifiers,
            chain_peaks: block_inputs.chain_peaks,
            prev_header: block_inputs.block_header,
        })
    }

    /// Converts [`BlockWitness`] into inputs to the block kernel program
    pub(super) fn into_program_inputs(
        self,
    ) -> Result<(AdviceInputs, StackInputs), BlockProverError> {
        let stack_inputs = self.build_stack_inputs();
        let advice_inputs = self.build_advice_inputs()?;

        Ok((advice_inputs, stack_inputs))
    }

    // HELPERS
    // ---------------------------------------------------------------------------------------------

    fn validate_inputs(
        block_inputs: &BlockInputs,
        batches: &[TransactionBatch],
    ) -> Result<(), BuildBlockError> {
        if batches.len() > MAX_BATCHES_PER_BLOCK {
            return Err(BuildBlockError::TooManyBatchesInBlock(batches.len()));
        }

        Self::validate_account_states(block_inputs, batches)?;
        Self::validate_nullifiers(block_inputs, batches)?;

        Ok(())
    }

    /// Validates that initial account states coming from the batches are the same as the account
    /// states returned from the store
    fn validate_account_states(
        block_inputs: &BlockInputs,
        batches: &[TransactionBatch],
    ) -> Result<(), BuildBlockError> {
        let batches_initial_states: BTreeMap<AccountId, Digest> =
            batches.iter().flat_map(|batch| batch.account_initial_states()).collect();

        let accounts_in_batches: BTreeSet<AccountId> =
            batches_initial_states.keys().cloned().collect();
        let accounts_in_store: BTreeSet<AccountId> =
            block_inputs.accounts.keys().copied().collect();

        if accounts_in_batches == accounts_in_store {
            let accounts_with_different_hashes: Vec<AccountId> = block_inputs
                .accounts
                .iter()
                .filter_map(|(account_id, witness)| {
                    let hash_in_store = witness.hash;
                    let hash_in_batches = batches_initial_states
                        .get(account_id)
                        .expect("we already verified that account id is contained in batches");

                    if hash_in_store == *hash_in_batches {
                        None
                    } else {
                        Some(*account_id)
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

    /// Validates that the nullifiers returned from the store are the same the produced nullifiers in the batches.
    /// Note that validation that the value of the nullifiers is `0` will be done in MASM.
    fn validate_nullifiers(
        block_inputs: &BlockInputs,
        batches: &[TransactionBatch],
    ) -> Result<(), BuildBlockError> {
        let produced_nullifiers_from_store: BTreeSet<Nullifier> =
            block_inputs.nullifiers.keys().copied().collect();

        let produced_nullifiers_from_batches: BTreeSet<Nullifier> =
            batches.iter().flat_map(|batch| batch.produced_nullifiers()).collect();

        if produced_nullifiers_from_store == produced_nullifiers_from_batches {
            Ok(())
        } else {
            let differing_nullifiers: Vec<Nullifier> = produced_nullifiers_from_store
                .symmetric_difference(&produced_nullifiers_from_batches)
                .copied()
                .collect();

            Err(BuildBlockError::InconsistentNullifiers(differing_nullifiers))
        }
    }

    /// Builds the stack inputs to the block kernel
    fn build_stack_inputs(&self) -> StackInputs {
        // Note: `StackInputs::new()` reverses the input vector, so we need to construct the stack
        // from the bottom to the top
        let mut stack_inputs = Vec::new();

        // Chain MMR stack inputs
        {
            stack_inputs.extend(self.prev_header.hash());
            stack_inputs.extend(self.chain_peaks.hash_peaks());
        }

        // Nullifiers stack inputs
        {
            let num_produced_nullifiers: Felt = (self.produced_nullifiers.len() as u64)
                .try_into()
                .expect("nullifiers number is greater than or equal to the field modulus");

            for nullifier in self.produced_nullifiers.keys() {
                stack_inputs.extend(nullifier.inner());
            }

            // append nullifier value (`[block_num, 0, 0, 0]`)
            let block_num = self.prev_header.block_num() + 1;
            stack_inputs.extend([block_num.into(), ZERO, ZERO, ZERO]);

            // append initial nullifier root
            stack_inputs.extend(self.prev_header.nullifier_root());

            // append number of nullifiers
            stack_inputs.push(num_produced_nullifiers);
        }

        // Notes stack inputs
        {
            for (batch_index, batch_created_notes_root) in self.batch_created_notes_roots.iter() {
                stack_inputs.extend(batch_created_notes_root.iter());

                let batch_index = Felt::try_from(*batch_index as u64)
                    .expect("batch index is greater than or equal to the field modulus");
                stack_inputs.push(batch_index);
            }

            let empty_root = EmptySubtreeRoots::entry(BLOCK_OUTPUT_NOTES_TREE_DEPTH, 0);
            stack_inputs.extend(*empty_root);
            stack_inputs.push(
                Felt::try_from(self.batch_created_notes_roots.len() as u64)
                    .expect("notes roots number is greater than or equal to the field modulus"),
            );
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
        stack_inputs.push(
            num_accounts_updated
                .try_into()
                .expect("updated accounts number is greater than or equal to the field modulus"),
        );

        // TODO: We need provide produced nullifier different way, because such big stack inputs
        //       will cause problem in recursive proofs
        StackInputs::new(stack_inputs).expect("Stack inputs count extends max limit")
    }

    /// Builds the advice inputs to the block kernel
    fn build_advice_inputs(self) -> Result<AdviceInputs, BlockProverError> {
        let merkle_store = {
            let mut merkle_store = MerkleStore::default();

            // add accounts merkle paths
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

            // add nullifiers merkle paths
            merkle_store
                .add_merkle_paths(self.produced_nullifiers.iter().map(|(nullifier, proof)| {
                    // Note: the initial value for all nullifiers in the tree is `[0, 0, 0, 0]`
                    (
                        u64::from(nullifier.most_significant_felt()),
                        Digest::default(),
                        proof.path().clone(),
                    )
                }))
                .map_err(BlockProverError::InvalidMerklePaths)?;

            merkle_store
        };

        let advice_map: Vec<_> = self
            .produced_nullifiers
            .values()
            .map(|proof| (proof.leaf().hash(), proof.leaf().to_elements()))
            .chain(std::iter::once(mmr_peaks_advice_map_key_value(&self.chain_peaks)))
            .collect();

        let advice_inputs =
            AdviceInputs::default().with_merkle_store(merkle_store).with_map(advice_map);

        Ok(advice_inputs)
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

// Generates the advice map key/value for Mmr peaks
fn mmr_peaks_advice_map_key_value(peaks: &MmrPeaks) -> (Digest, Vec<Felt>) {
    let mut elements = vec![Felt::new(peaks.num_leaves() as u64), ZERO, ZERO, ZERO];
    elements.extend(peaks.flatten_and_pad_peaks());

    (peaks.hash_peaks(), elements)
}
