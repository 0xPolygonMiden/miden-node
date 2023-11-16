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

#! TODO: put a version of this in stdlib?
#!
#! Inserts the specified value under the specified key in a Sparse Merkle Tree of depth 8 defined by the
#! specified root. If the insert is successful, the old value located under the specified key
#! is returned via the stack.
#!
#! Inputs:
#!   Operand stack: [VALUE, key, ROOT, ...]
#!
#! Outputs:
#!   Operand stack: [OLD_VALUE, NEW_ROOT, ...]
proc.mtree_8_set
    # prepare the stack for mtree_set operation
    movup.4 movdn.8 swapw movup.8 push.8
    # => [8, key, ROOT, VALUE, ...]

    mtree_set
    # => [OLD_VALUE, NEW_ROOT, ...]
end

#! Compute the note root
#! 
#! Stack: [num_notes_updated, SMT_EMPTY_ROOT_DEPTH_8, NOTE_HASH_0, ... , NOTE_HASH_{n-1}]
#! Output: [NOTES_ROOT]
proc.compute_note_root
    # assess if we should loop
    push.0 dup.1 dup.1 neq
    #=> [should_loop, loop_counter=0, num_notes_updated, SMT_EMPTY_ROOT_DEPTH_8, NOTE_HASH_0, ... , NOTE_HASH_{n-1}]

    while.true
        #=> [loop_counter, num_notes_updated, ROOT_i, NOTE_HASH_i, ... ]

        # Move loop_counter and `num_notes_updated` down for next iteration
        # Keep a copy of `loop_counter`; we use it as the insert key for the ith note hash
        dup movdn.10 swap movdn.10
        #=> [loop_counter, ROOT_i, NOTE_HASH_i, loop_counter, num_notes_updated, ... ]

        # Prepare stack for mtree_8_set
        movdn.4
        #=> [ROOT_i, loop_counter, NOTE_HASH_i, loop_counter, num_notes_updated, ... ]

        exec.mtree_8_set dropw 
        #=> [ROOT_{i+1}, loop_counter, num_notes_updated, ... ]

        # Prepare stack for loop counter check
        movdn.5 movdn.5 movdn.5 movdn.5
        #=> [loop_counter, num_notes_updated, ROOT_{i+1}, ... ]

        # loop counter
        add.1 dup.1 dup.1 neq
        #=> [should_loop, loop_counter + 1, num_notes_updated, ROOT_{i+1}, ... ]
    end

    drop drop
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

        // TODO: Use `StackOutputs::pop_digest()` once merged
        let (account_root_output, note_root_output) = {
            let root_outputs: Vec<_> = execution_output.stack_outputs().stack().chunks(4).collect();

            (root_outputs[0], root_outputs[1])
        };

        let new_account_root = {
            let digest_elements: Vec<Felt> = account_root_output
            .iter()
            .cloned()
            .map(Felt::from)
            // We reverse, since a word `[a, b, c, d]` will be stored on the stack as `[d, c, b, a]`
            .rev()
            .collect();

            let digest_elements: [Felt; 4] =
                digest_elements.try_into().map_err(|_| BlockProverError::InvalidRootReturned)?;

            digest_elements.into()
        };

        let new_note_root = {
            let digest_elements: Vec<Felt> = note_root_output
            .iter()
            .cloned()
            .map(Felt::from)
            // We reverse, since a word `[a, b, c, d]` will be stored on the stack as `[d, c, b, a]`
            .rev()
            .collect();

            let digest_elements: [Felt; 4] =
                digest_elements.try_into().map_err(|_| BlockProverError::InvalidRootReturned)?;

            digest_elements.into()
        };

        Ok((new_account_root, new_note_root))
    }
}

/// Provides inputs to the `BlockKernel` so that it can generate the new header
#[derive(Debug, PartialEq, Eq)]
pub(super) struct BlockWitness {
    updated_accounts: BTreeMap<AccountId, AccountUpdate>,
    /// collection of all batches' created notes SMT roots
    batch_created_notes_roots: Vec<Digest>,
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

        // TODO: Validate that there are less than 2^8 roots (and in StateView)
        let batch_created_notes_roots =
            batches.iter().map(|batch| batch.created_notes_root()).collect();

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
                for batch_created_notes_root in self.batch_created_notes_roots {
                    let root_eles: [Felt; 4] = batch_created_notes_root.into();
                    println!("Adding batch root: {:?}", root_eles);
                    stack_inputs.extend(root_eles);
                }

                let empty_root_depth_8 = EmptySubtreeRoots::entry(CREATED_NOTES_TREE_DEPTH, 0);
                stack_inputs.extend(*empty_root_depth_8);
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
