use miden_air::ExecutionOptions;
use miden_objects::{
    accounts::AccountId,
    assembly::Assembler,
    crypto::merkle::{MerklePath, MerkleStore},
    Digest, Felt,
};
use miden_stdlib::StdLibrary;
use miden_vm::{
    crypto::MerkleError, execute, AdviceInputs, DefaultHost, ExecutionError, MemAdviceProvider,
    Program, StackInputs,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AccountRootUpdateError {
    #[error("Received invalid merkle path")]
    InvalidMerklePaths(MerkleError),
    #[error("program execution failed")]
    ProgramExecutionFailed(ExecutionError),
    #[error("invalid return value on stack (not a hash)")]
    InvalidRootReturned,
}

/// Stack inputs:
/// [num_accounts_updated,
///  OLD_ACCOUNT_ROOT,
///  NEW_ACCOUNT_HASH_0, account_id_0, ... , NEW_ACCOUNT_HASH_n, account_id_n]
const ACCOUNT_UPDATE_ROOT_MASM: &str = "
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
pub struct BlockKernel {
    program: Program,
}

impl BlockKernel {
    pub fn new() -> Self {
        let account_program = {
            let assembler = Assembler::default()
                .with_library(&StdLibrary::default())
                .expect("failed to load std-lib");

            assembler
                .compile(ACCOUNT_UPDATE_ROOT_MASM)
                .expect("failed to load account update program")
        };

        Self {
            program: account_program,
        }
    }

    /// `current_account_states`: iterator of (account id, node hash, Merkle path)
    /// `account_updates`: iterator of (account id, new account hash)
    pub fn compute_new_account_root(
        &self,
        current_account_states: impl Iterator<Item = (AccountId, Digest, MerklePath)>,
        account_updates: impl Iterator<Item = (AccountId, Digest)>,
        initial_account_root: Digest,
    ) -> Result<Digest, AccountRootUpdateError> {
        let host = {
            let advice_inputs = {
                let mut merkle_store = MerkleStore::default();
                merkle_store
                    .add_merkle_paths(current_account_states.map(
                        |(account_id, node_hash, path)| (u64::from(account_id), node_hash, path),
                    ))
                    .map_err(AccountRootUpdateError::InvalidMerklePaths)?;

                AdviceInputs::default().with_merkle_store(merkle_store)
            };

            let advice_provider = MemAdviceProvider::from(advice_inputs);

            DefaultHost::new(advice_provider)
        };

        let stack_inputs = {
            // Note: `StackInputs::new()` reverses the input vector, so we need to construct the stack
            // from the bottom to the top
            let mut stack_inputs = Vec::new();

            // append all insert key/values
            let mut num_accounts_updated: u64 = 0;
            for (idx, (account_id, new_account_hash)) in account_updates.enumerate() {
                stack_inputs.push(account_id.into());
                stack_inputs.extend(new_account_hash);

                let idx = u64::try_from(idx).expect("can't be more than 2^64 - 1 accounts");
                num_accounts_updated = idx + 1;
            }

            // append initial account root
            stack_inputs.extend(initial_account_root);

            // append number of accounts updated
            stack_inputs.push(num_accounts_updated.into());

            StackInputs::new(stack_inputs)
        };

        let execution_output =
            execute(&self.program, stack_inputs, host, ExecutionOptions::default())
                .map_err(AccountRootUpdateError::ProgramExecutionFailed)?;

        let new_account_root = {
            let stack_output = execution_output.stack_outputs().stack_truncated(4);

            let digest_elements: Vec<Felt> = stack_output
            .iter()
            .map(|&num| Felt::try_from(num).map_err(|_|AccountRootUpdateError::InvalidRootReturned))
            // We reverse, since a word `[a, b, c, d]` will be stored on the stack as `[d, c, b, a]`
            .rev()
            .collect::<Result<_, AccountRootUpdateError>>()?;

            let digest_elements: [Felt; 4] = digest_elements
                .try_into()
                .map_err(|_| AccountRootUpdateError::InvalidRootReturned)?;

            digest_elements.into()
        };

        Ok(new_account_root)
    }
}
