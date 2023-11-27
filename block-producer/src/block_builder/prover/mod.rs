use std::{
    collections::{BTreeMap, BTreeSet},
    time::{SystemTime, UNIX_EPOCH},
};

use miden_air::ExecutionOptions;
use miden_node_proto::domain::BlockInputs;
use miden_objects::{
    accounts::AccountId,
    assembly::Assembler,
    crypto::merkle::{EmptySubtreeRoots, MerkleStore},
    BlockHeader, Digest, Felt,
};
use miden_stdlib::StdLibrary;
use miden_vm::{
    crypto::MerklePath, execute, AdviceInputs, DefaultHost, MemAdviceProvider, Program, StackInputs,
};

use crate::{batch_builder, SharedTxBatch};

use super::{errors::BlockProverError, BuildBlockError};

/// The index of the word at which the account root is stored on the output stack.
pub const ACCOUNT_ROOT_WORD_IDX: usize = 0;

/// The index of the word at which the note root is stored on the output stack.
pub const NOTE_ROOT_WORD_IDX: usize = 4;

/// The depth at which we insert roots from the batches.
pub(crate) const CREATED_NOTES_TREE_INSERTION_DEPTH: u8 = 8;

/// The depth of the created notes tree in the block.
pub(crate) const CREATED_NOTES_TREE_DEPTH: u8 =
    CREATED_NOTES_TREE_INSERTION_DEPTH + batch_builder::CREATED_NOTES_SMT_DEPTH;

#[cfg(test)]
mod tests;

/// Note: For now, the "block kernel" only computes the account root. Eventually, it will compute
/// the entire block header.
///
/// Stack inputs: [num_accounts_updated, OLD_ACCOUNT_ROOT, NEW_ACCOUNT_HASH_0, account_id_0, ... ,
/// NEW_ACCOUNT_HASH_n, account_id_n]
const BLOCK_KERNEL_MASM: &str = "
use.std::collections::smt64

#! Compute the account root
#! 
#! Stack: [num_accounts_updated, OLD_ACCOUNT_ROOT, 
#!         NEW_ACCOUNT_HASH_0, account_id_0, ... , NEW_ACCOUNT_HASH_n, account_id_n]
#! Output: [NEW_ACCOUNT_ROOT]
proc.compute_account_root
    dup neq.0 
    # => [0 or 1, num_accounts_updated, OLD_ACCOUNT_ROOT, 
    #     NEW_ACCOUNT_HASH_0, account_id_0, ... , NEW_ACCOUNT_HASH_n, account_id_n]

    while.true
        # stack: [counter, ROOT_0, ..., NEW_ACCOUNT_HASH_i, account_id_i , ...]

        # Move counter down for next iteration
        movdn.9
        # => [ROOT_i, NEW_ACCOUNT_HASH_i, account_id_i, counter, ...]

        # Move root down (for smt64.set)
        movdn.8 movdn.8 movdn.8 movdn.8
        # => [NEW_ACCOUNT_HASH_i, account_id_i, ROOT_i, counter, ...]

        # set new value in SMT
        exec.smt64::set dropw
        # => [ROOT_{i+1}, counter, ...]

        # loop counter
        movup.4 sub.1 dup neq.0
        # => [0 or 1, counter-1, ROOT_{i+1}, ...]
    end

    drop
    # => [ROOT_{n-1}]
end

#! Compute the note root.
#!
#! Each batch contains a tree of depth 12 for its created notes. The block's created notes tree is created
#! by aggregating up to 2^8 tree roots coming from the batches contained in the block.
#! 
#! `SMT_EMPTY_ROOT` must be `E20`, the root of the empty tree of depth 20. If less than 2^8 batches are
#! contained in the block, `E12` is used as the padding value; this is derived from the fact that
#! `SMT_EMPTY_ROOT` is `E20`, and that our tree has depth 8.
#! 
#! Stack: [num_notes_updated, SMT_EMPTY_ROOT, note_key_0, NOTE_HASH_0, ... , note_key_{n-1}, NOTE_HASH_{n-1}]
#! Output: [NOTES_ROOT]
proc.compute_note_root
    # assess if we should loop
    dup neq.0 
    #=> [0 or 1, num_notes_updated, SMT_EMPTY_ROOT, ... ]

    while.true
        #=> [notes_left_to_update, ROOT_i, note_key_i, NOTE_HASH_i, ... ]

        # Prepare stack for mtree_set
        movdn.9 movup.4 push.8
        #=> [depth=8, note_key_i, ROOT_i, NOTE_HASH_i, notes_left_to_update, ... ]

        mtree_set dropw 
        #=> [ROOT_{i+1}, notes_left_to_update, ... ]

        # loop counter
        movup.4 sub.1 dup neq.0
        #=> [0 or 1, notes_left_to_update - 1, ROOT_{i+1}, ... ]
    end

    drop
    # => [ROOT_{n-1}]
end

# Stack: [<account root inputs>, <note root inputs>]
proc.main.2
    exec.compute_account_root loc_storew.0 dropw
    #=> [<note root inputs>]

    exec.compute_note_root loc_storew.1 dropw
    #=> [ ]

    # Load output on stack
    loc_loadw.1 padw loc_loadw.0
    #=> [ ACCOUNT_ROOT, NOTE_ROOT]
end

begin
    exec.main
end
";

#[derive(Debug)]
pub(super) struct BlockProver {
    kernel: Program,
}

impl BlockProver {
    pub fn new() -> Self {
        let account_program = {
            let assembler = Assembler::default()
                .with_library(&StdLibrary::default())
                .expect("failed to load std-lib");

            assembler
                .compile(BLOCK_KERNEL_MASM)
                .expect("failed to load account update program")
        };

        Self {
            kernel: account_program,
        }
    }

    // Note: this will eventually all be done in the VM, and also return an `ExecutionProof`
    pub fn prove(
        &self,
        witness: BlockWitness,
    ) -> Result<BlockHeader, BuildBlockError> {
        let prev_hash = witness.prev_header.prev_hash();
        let block_num = witness.prev_header.block_num();
        let version = witness.prev_header.version();

        let (account_root, note_root) = self.compute_roots(witness)?;

        let chain_root = Digest::default();
        let nullifier_root = Digest::default();
        let batch_root = Digest::default();
        let proof_hash = Digest::default();
        let timestamp: Felt = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("today is expected to be before 1970")
            .as_millis()
            .into();

        Ok(BlockHeader::new(
            prev_hash,
            block_num,
            chain_root,
            account_root,
            nullifier_root,
            note_root,
            batch_root,
            proof_hash,
            version,
            timestamp,
        ))
    }

    fn compute_roots(
        &self,
        witness: BlockWitness,
    ) -> Result<(Digest, Digest), BlockProverError> {
        let (advice_inputs, stack_inputs) = witness.into_parts()?;
        let host = {
            let advice_provider = MemAdviceProvider::from(advice_inputs);

            DefaultHost::new(advice_provider)
        };

        let execution_output =
            execute(&self.kernel, stack_inputs, host, ExecutionOptions::default())
                .map_err(BlockProverError::ProgramExecutionFailed)?;

        let new_account_root = execution_output
            .stack_outputs()
            .get_stack_word(ACCOUNT_ROOT_WORD_IDX)
            .ok_or(BlockProverError::InvalidRootOutput("account".to_string()))?;

        let new_note_root = execution_output
            .stack_outputs()
            .get_stack_word(NOTE_ROOT_WORD_IDX)
            .ok_or(BlockProverError::InvalidRootOutput("note".to_string()))?;

        Ok((new_account_root.into(), new_note_root.into()))
    }
}

// BLOCK WITNESS
// =================================================================================================

/// Provides inputs to the `BlockKernel` so that it can generate the new header
#[derive(Debug, PartialEq, Eq)]
pub(super) struct BlockWitness {
    updated_accounts: BTreeMap<AccountId, AccountUpdate>,
    /// (batch_index, created_notes_root) for batches that contain notes
    batch_created_notes_roots: Vec<(usize, Digest)>,
    prev_header: BlockHeader,
}

impl BlockWitness {
    pub(super) fn new(
        block_inputs: BlockInputs,
        batches: Vec<SharedTxBatch>,
    ) -> Result<Self, BuildBlockError> {
        Self::validate_inputs(&block_inputs, &batches)?;

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
            prev_header: block_inputs.block_header,
        })
    }

    fn validate_inputs(
        block_inputs: &BlockInputs,
        batches: &[SharedTxBatch],
    ) -> Result<(), BuildBlockError> {
        // TODO:
        // - Block height returned for each nullifier is 0.

        // Validate that there aren't too many batches in the block.
        if batches.len() > 2usize.pow(CREATED_NOTES_TREE_INSERTION_DEPTH.into()) {
            return Err(BuildBlockError::TooManyBatchesInBlock(batches.len()));
        }

        Self::validate_account_states(block_inputs, batches)?;

        Ok(())
    }

    /// Validate that initial account states coming from the batches are the same as the account
    /// states returned from the store
    fn validate_account_states(
        block_inputs: &BlockInputs,
        batches: &[SharedTxBatch],
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

    fn into_parts(self) -> Result<(AdviceInputs, StackInputs), BlockProverError> {
        let stack_inputs = {
            // Note: `StackInputs::new()` reverses the input vector, so we need to construct the stack
            // from the bottom to the top
            let mut stack_inputs = Vec::new();

            // Notes stack inputs
            {
                let num_created_notes_roots = self.batch_created_notes_roots.len();
                for (batch_index, batch_created_notes_root) in self.batch_created_notes_roots {
                    stack_inputs.extend(batch_created_notes_root);

                    let batch_index = u64::try_from(batch_index)
                        .expect("can't be more than 2^64 - 1 notes created");
                    stack_inputs.push(Felt::from(batch_index));
                }

                let empty_root = EmptySubtreeRoots::entry(CREATED_NOTES_TREE_DEPTH, 0);
                stack_inputs.extend(*empty_root);
                stack_inputs.push(Felt::from(num_created_notes_roots as u64));
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

            AdviceInputs::default().with_merkle_store(merkle_store)
        };

        Ok((advice_inputs, stack_inputs))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct AccountUpdate {
    pub initial_state_hash: Digest,
    pub final_state_hash: Digest,
    pub proof: MerklePath,
}
