use std::{
    collections::{BTreeMap, BTreeSet},
    time::{SystemTime, UNIX_EPOCH},
};

use miden_air::ExecutionOptions;
use miden_node_proto::domain::{AccountInputRecord, BlockInputs};
use miden_objects::{
    accounts::AccountId, assembly::Assembler, crypto::merkle::MerkleStore, BlockHeader, Digest,
    Felt,
};
use miden_stdlib::StdLibrary;
use miden_vm::{
    crypto::MerkleError, execute, AdviceInputs, DefaultHost, ExecutionError, MemAdviceProvider,
    Program, StackInputs,
};
use thiserror::Error;

use crate::SharedTxBatch;

use super::BuildBlockError;

#[derive(Error, Debug)]
pub enum BlockKernelError {
    #[error("Received invalid merkle path")]
    InvalidMerklePaths(MerkleError),
    #[error("program execution failed")]
    ProgramExecutionFailed(ExecutionError),
    #[error("invalid return value on stack (not a hash)")]
    InvalidRootReturned,
}

/// Note: For now, the "block kernel" only computes the account root. Eventually, it will compute
/// the entire block header.
///
/// Stack inputs: [num_accounts_updated, OLD_ACCOUNT_ROOT, NEW_ACCOUNT_HASH_0, account_id_0, ... ,
/// NEW_ACCOUNT_HASH_n, account_id_n]
const BLOCK_KERNEL_MASM: &str = "
use.std::collections::smt64

begin
    push.1
    while.true
        # stack: [counter, ROOT_0, ..., NEW_ACCOUNT_HASH_i, account_id_i , ...]

        # Move counter down for next iteration
        movdn.9
        # => [ROOT_i, NEW_ACCOUNT_HASH_i, account_id_i, counter, ...]

        # Move root down (for smt64.set)
        movdn.8 movdn.8 movdn.8 movdn.8
        # => [NEW_ACCOUNT_HASH_i, account_id_i, ROOT_i, counter, ...]

        # set new value in SMT
        smt64.set dropw
        # => [ROOT_{i+1}, counter, ...]

        # loop counter
        movup.4 sub.1 dup neq.0
        # => [0 or 1, counter-1, ROOT_{i+1}, ...]
    end

    drop
    # => [ROOT_{n-1}]
end
";

#[derive(Debug)]
pub(super) struct BlockKernel {
    program: Program,
}

impl BlockKernel {
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
            program: account_program,
        }
    }

    // Note: this will eventually all be done in the VM
    pub fn compute_block_header(
        &self,
        witness: BlockWitness,
    ) -> Result<BlockHeader, BuildBlockError> {
        let prev_hash = witness.prev_header.prev_hash();
        let block_num = witness.prev_header.block_num();
        let version = witness.prev_header.version();

        let chain_root = Digest::default();
        let account_root = self.compute_new_account_root(witness)?;
        let nullifier_root = Digest::default();
        let note_root = Digest::default();
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

    /// `current_account_states`: iterator of (account id, node hash, Merkle path)
    /// `account_updates`: iterator of (account id, new account hash)
    fn compute_new_account_root(
        &self,
        witness: BlockWitness,
    ) -> Result<Digest, BlockKernelError> {
        let (advice_inputs, stack_inputs) = witness.into_parts()?;
        let host = {
            let advice_provider = MemAdviceProvider::from(advice_inputs);

            DefaultHost::new(advice_provider)
        };

        let execution_output =
            execute(&self.program, stack_inputs, host, ExecutionOptions::default())
                .map_err(BlockKernelError::ProgramExecutionFailed)?;

        let new_account_root = {
            let stack_output = execution_output.stack_outputs().stack_truncated(4);

            let digest_elements: Vec<Felt> = stack_output
            .iter()
            .map(|&num| Felt::try_from(num).map_err(|_|BlockKernelError::InvalidRootReturned))
            // We reverse, since a word `[a, b, c, d]` will be stored on the stack as `[d, c, b, a]`
            .rev()
            .collect::<Result<_, BlockKernelError>>()?;

            let digest_elements: [Felt; 4] =
                digest_elements.try_into().map_err(|_| BlockKernelError::InvalidRootReturned)?;

            digest_elements.into()
        };

        Ok(new_account_root)
    }
}

/// Provides inputs to the `BlockKernel` so that it can generate the new header
pub(super) struct BlockWitness {
    account_states: Vec<AccountInputRecord>,
    account_updates: Vec<(AccountId, Digest)>,
    prev_header: BlockHeader,
}

impl BlockWitness {
    pub(super) fn new(
        block_inputs: BlockInputs,
        batches: Vec<SharedTxBatch>,
    ) -> Result<Self, BuildBlockError> {
        Self::validate_inputs(&block_inputs, &batches)?;

        let account_updates: Vec<(AccountId, Digest)> =
            batches.iter().flat_map(|batch| batch.updated_accounts()).collect();

        Ok(Self {
            account_states: block_inputs.account_states,
            account_updates,
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

    fn into_parts(self) -> Result<(AdviceInputs, StackInputs), BlockKernelError> {
        let advice_inputs = {
            let mut merkle_store = MerkleStore::default();
            merkle_store
                .add_merkle_paths(self.account_states.into_iter().map(
                    |AccountInputRecord {
                         account_id,
                         account_hash,
                         proof,
                     }| (u64::from(account_id), account_hash, proof),
                ))
                .map_err(BlockKernelError::InvalidMerklePaths)?;

            AdviceInputs::default().with_merkle_store(merkle_store)
        };

        let stack_inputs = {
            // Note: `StackInputs::new()` reverses the input vector, so we need to construct the stack
            // from the bottom to the top
            let mut stack_inputs = Vec::new();

            // append all insert key/values
            let mut num_accounts_updated: u64 = 0;
            for (idx, (account_id, new_account_hash)) in
                self.account_updates.into_iter().enumerate()
            {
                stack_inputs.push(account_id.into());
                stack_inputs.extend(new_account_hash);

                let idx = u64::try_from(idx).expect("can't be more than 2^64 - 1 accounts");
                num_accounts_updated = idx + 1;
            }

            // append initial account root
            stack_inputs.extend(self.prev_header.account_root());

            // append number of accounts updated
            stack_inputs.push(num_accounts_updated.into());

            StackInputs::new(stack_inputs)
        };

        Ok((advice_inputs, stack_inputs))
    }
}
