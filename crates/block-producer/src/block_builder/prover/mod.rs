use std::time::{SystemTime, UNIX_EPOCH};

use miden_objects::{assembly::Assembler, block::compute_tx_hash, BlockHeader, Digest};
use miden_processor::{execute, DefaultHost, ExecutionOptions, MemAdviceProvider, Program};
use miden_stdlib::StdLibrary;

use self::block_witness::BlockWitness;
use crate::errors::{BlockProverError, BuildBlockError};

/// The index of the word at which the account root is stored on the output stack.
pub const ACCOUNT_ROOT_WORD_IDX: usize = 0;

/// The index of the word at which the note root is stored on the output stack.
pub const NOTE_ROOT_WORD_IDX: usize = 4;

/// The index of the word at which the nullifier root is stored on the output stack.
pub const NULLIFIER_ROOT_WORD_IDX: usize = 8;

/// The index of the word at which the note root is stored on the output stack.
pub const CHAIN_MMR_ROOT_WORD_IDX: usize = 12;

pub mod block_witness;

#[cfg(test)]
mod tests;

/// Note: For now, the "block kernel" only computes the account root. Eventually, it will compute
/// the entire block header.
///
/// Stack inputs: [num_accounts_updated, OLD_ACCOUNT_ROOT, NEW_ACCOUNT_HASH_0, account_id_0, ... ,
/// NEW_ACCOUNT_HASH_n, account_id_n]
const BLOCK_KERNEL_MASM: &str = "
use.std::collections::smt
use.std::collections::mmr

const.ACCOUNT_TREE_DEPTH=64
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

        # Prepare stack for `mtree_set`
        movup.8 push.ACCOUNT_TREE_DEPTH
        # => [account_tree_depth, account_id_i, ROOT_i, NEW_ACCOUNT_HASH_i, counter, ...]

        # set new value in SMT
        mtree_set dropw
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

#! Stack: [num_produced_nullifiers, OLD_NULLIFIER_ROOT, NULLIFIER_VALUE,
#!         NULLIFIER_0, ..., NULLIFIER_n]
#! Output: [NULLIFIER_ROOT]
proc.compute_nullifier_root
    # assess if we should loop
    dup neq.0
    #=> [0 or 1, num_produced_nullifiers, OLD_NULLIFIER_ROOT, NULLIFIER_VALUE, NULLIFIER_0, ..., NULLIFIER_n ]

    while.true
        #=> [num_nullifiers_left_to_update, ROOT_i, NULLIFIER_VALUE, NULLIFIER_i, ... ]

        # Prepare stack for `smt::set`
        movdn.12 movupw.2 dupw.2
        #=> [NULLIFIER_VALUE, NULLIFIER_i, ROOT_i, NULLIFIER_VALUE, num_nullifiers_left_to_update, ... ]

        exec.smt::set
        #=> [OLD_VALUE, ROOT_{i+1}, NULLIFIER_VALUE, num_nullifiers_left_to_update, ... ]

        # Check that OLD_VALUE == 0 (i.e. that nullifier was indeed not previously produced)
        assertz assertz assertz assertz
        #=> [ROOT_{i+1}, NULLIFIER_VALUE, num_nullifiers_left_to_update, ... ]

        # loop counter
        movup.8 sub.1 dup neq.0
        #=> [0 or 1, num_nullifiers_left_to_update - 1, ROOT_{i+1}, NULLIFIER_VALUE, ... ]
    end
    #=> [0, ROOT_{n-1}, NULLIFIER_VALUE ]

    drop swapw dropw
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

# Stack: [<account root inputs>, <note root inputs>, <nullifier root inputs>, <chain mmr root inputs>]
begin
    exec.compute_account_root mem_storew.0 dropw
    # => [<note root inputs>, <nullifier root inputs>, <chain mmr root inputs>]

    exec.compute_note_root mem_storew.1 dropw
    # => [ <nullifier root inputs>, <chain mmr root inputs> ]

    exec.compute_nullifier_root mem_storew.2 dropw
    # => [ <chain mmr root inputs> ]

    exec.compute_chain_mmr_root
    # => [ CHAIN_MMR_ROOT ]

    # Load output on stack
    padw mem_loadw.2 padw mem_loadw.1 padw mem_loadw.0
    #=> [ ACCOUNT_ROOT, NOTE_ROOT, NULLIFIER_ROOT, CHAIN_MMR_ROOT ]
end
";

#[derive(Debug)]
pub(crate) struct BlockProver {
    kernel: Program,
}

impl BlockProver {
    pub fn new() -> Self {
        let account_program = {
            let assembler = Assembler::default()
                .with_library(StdLibrary::default())
                .expect("failed to load std-lib");

            assembler
                .assemble_program(BLOCK_KERNEL_MASM)
                .expect("failed to load account update program")
        };

        Self { kernel: account_program }
    }

    // Note: this will eventually all be done in the VM, and also return an `ExecutionProof`
    pub fn prove(&self, witness: BlockWitness) -> Result<BlockHeader, BuildBlockError> {
        let prev_hash = witness.prev_header.hash();
        let block_num = witness.prev_header.block_num() + 1;
        let version = witness.prev_header.version();

        let tx_hash = compute_tx_hash(witness.transactions());
        let (account_root, note_root, nullifier_root, chain_root) = self.compute_roots(witness)?;

        let proof_hash = Digest::default();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("today is expected to be after 1970")
            .as_secs()
            .try_into()
            .expect("timestamp must fit to `u32`");

        Ok(BlockHeader::new(
            version,
            prev_hash,
            block_num,
            chain_root,
            account_root,
            nullifier_root,
            note_root,
            tx_hash,
            proof_hash,
            timestamp,
        ))
    }

    fn compute_roots(
        &self,
        witness: BlockWitness,
    ) -> Result<(Digest, Digest, Digest, Digest), BlockProverError> {
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
            .ok_or(BlockProverError::InvalidRootOutput("account"))?;

        let new_note_root = execution_output
            .stack_outputs()
            .get_stack_word(NOTE_ROOT_WORD_IDX)
            .ok_or(BlockProverError::InvalidRootOutput("note"))?;

        let new_nullifier_root = execution_output
            .stack_outputs()
            .get_stack_word(NULLIFIER_ROOT_WORD_IDX)
            .ok_or(BlockProverError::InvalidRootOutput("nullifier"))?;

        let new_chain_mmr_root = execution_output
            .stack_outputs()
            .get_stack_word(CHAIN_MMR_ROOT_WORD_IDX)
            .ok_or(BlockProverError::InvalidRootOutput("chain mmr"))?;

        Ok((
            new_account_root.into(),
            new_note_root.into(),
            new_nullifier_root.into(),
            new_chain_mmr_root.into(),
        ))
    }
}
