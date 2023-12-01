use std::time::{SystemTime, UNIX_EPOCH};

use miden_air::{ExecutionOptions, Felt};
use miden_objects::{assembly::Assembler, BlockHeader, Digest, ONE};
use miden_stdlib::StdLibrary;
use miden_vm::{execute, DefaultHost, MemAdviceProvider, Program};

use self::block_witness::BlockWitness;

use super::{errors::BlockProverError, BuildBlockError};

/// The index of the word at which the account root is stored on the output stack.
pub const ACCOUNT_ROOT_WORD_IDX: usize = 0;

/// The index of the word at which the note root is stored on the output stack.
pub const NOTE_ROOT_WORD_IDX: usize = 4;

/// The index of the word at which the note root is stored on the output stack.
pub const CHAIN_MMR_ROOT_WORD_IDX: usize = 8;

pub mod block_witness;

#[cfg(test)]
mod tests;

/// Note: For now, the "block kernel" only computes the account root. Eventually, it will compute
/// the entire block header.
///
/// Stack inputs: [num_accounts_updated, OLD_ACCOUNT_ROOT, NEW_ACCOUNT_HASH_0, account_id_0, ... ,
/// NEW_ACCOUNT_HASH_n, account_id_n]
const BLOCK_KERNEL_MASM: &str = "
use.std::collections::smt64
use.std::collections::mmr

const.CHAIN_MMR_PTR=1000

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
#! Each batch contains a tree of depth 13 for its created notes. The block's created notes tree is created
#! by aggregating up to 2^8 tree roots coming from the batches contained in the block.
#! 
#! `SMT_EMPTY_ROOT` must be `E21`, the root of the empty tree of depth 21. If less than 2^8 batches are
#! contained in the block, `E13` is used as the padding value; this is derived from the fact that
#! `SMT_EMPTY_ROOT` is `E21`, and that our tree has depth 8.
#! 
#! Stack: [num_notes_updated, SMT_EMPTY_ROOT, 
#!         batch_note_root_idx_0, BATCH_NOTE_TREE_ROOT_0, 
#!         ... , 
#!         batch_note_root_idx_{n-1}, BATCH_NOTE_TREE_ROOT_{n-1}]
#! Output: [NOTES_ROOT]
proc.compute_note_root
    # assess if we should loop
    dup neq.0 
    #=> [0 or 1, num_notes_updated, SMT_EMPTY_ROOT, ... ]

    while.true
        #=> [note_roots_left_to_update, ROOT_i, batch_note_root_idx_i, BATCH_NOTE_TREE_ROOT_i, ... ]

        # Prepare stack for mtree_set
        movdn.9 movup.4 push.8
        #=> [depth=8, batch_note_root_idx_i, ROOT_i, BATCH_NOTE_TREE_ROOT_i, note_roots_left_to_update, ... ]

        mtree_set dropw 
        #=> [ROOT_{i+1}, note_roots_left_to_update, ... ]

        # loop counter
        movup.4 sub.1 dup neq.0
        #=> [0 or 1, note_roots_left_to_update - 1, ROOT_{i+1}, ... ]
    end

    drop
    # => [ROOT_{n-1}]
end

#! Compute the chain MMR root
#! 
#! Stack: [ PREV_CHAIN_MMR_HASH, PREV_BLOCK_HASH_TO_INSERT ]
#! Advice map: PREV_CHAIN_MMR_HASH -> NUM_LEAVES || peak_0 || .. || peak_{n-1} || <maybe padding>
#!
#! Output: [ CHAIN_MMR_ROOT ]
proc.compute_chain_mmr_root
    push.CHAIN_MMR_PTR movdn.4
    # => [ PREV_CHAIN_MMR_HASH, chain_mmr_ptr, PREV_BLOCK_HASH_TO_INSERT ]

    # load the chain MMR (as of previous block) at memory location CHAIN_MMR_PTR
    exec.mmr::unpack
    # => [ PREV_BLOCK_HASH_TO_INSERT ]

    push.CHAIN_MMR_PTR movdn.4
    # => [ PREV_BLOCK_HASH_TO_INSERT, chain_mmr_ptr ]

    # add PREV_BLOCK_HASH_TO_INSERT to chain MMR
    exec.mmr::add
    # => [ ]

    # Compute new MMR root
    push.CHAIN_MMR_PTR exec.mmr::pack
    # => [ CHAIN_MMR_ROOT ]
end

# Stack: [<account root inputs>, <note root inputs>, <chain mmr root inputs>]
begin
    exec.compute_account_root mem_storew.0 dropw
    # => [<note root inputs>, <chain mmr root inputs>]

    exec.compute_note_root mem_storew.1 dropw
    # => [ <chain mmr root inputs> ]

    exec.compute_chain_mmr_root
    # => [ ]

    # Load output on stack
    padw mem_loadw.1 padw mem_loadw.0
    #=> [ ACCOUNT_ROOT, NOTE_ROOT, CHAIN_MMR_ROOT ]
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
        let prev_hash = witness.prev_header.hash();
        let block_num = witness.prev_header.block_num() + ONE;
        let version = witness.prev_header.version();

        let (account_root, note_root, chain_root) = self.compute_roots(witness)?;

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
    ) -> Result<(Digest, Digest, Digest), BlockProverError> {
        let (advice_inputs, stack_inputs) = witness.into_program_inputs()?;
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

        let new_chain_mmr_root = execution_output
            .stack_outputs()
            .get_stack_word(CHAIN_MMR_ROOT_WORD_IDX)
            .ok_or(BlockProverError::InvalidRootOutput("chain mmr".to_string()))?;

        Ok((new_account_root.into(), new_note_root.into(), new_chain_mmr_root.into()))
    }
}
