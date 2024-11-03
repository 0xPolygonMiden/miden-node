use std::time::{SystemTime, UNIX_EPOCH};

use miden_lib::transaction::TransactionKernel;
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

const BLOCK_KERNEL_MASM: &str = include_str!("asm/block_kernel.masm");

#[derive(Debug)]
pub(crate) struct BlockProverKernel {
    kernel: Program,
}

impl BlockProverKernel {
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
            .expect("timestamp must fit in a `u32`");

        Ok(BlockHeader::new(
            version,
            prev_hash,
            block_num,
            chain_root,
            account_root,
            nullifier_root,
            note_root,
            tx_hash,
            TransactionKernel::kernel_root(),
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

            let mut host = DefaultHost::new(advice_provider);
            host.load_mast_forest(StdLibrary::default().mast_forest().clone());

            host
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
