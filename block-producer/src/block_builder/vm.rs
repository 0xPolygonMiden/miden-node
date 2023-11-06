use miden_air::ExecutionOptions;
use miden_objects::{
    accounts::AccountId,
    assembly::Assembler,
    crypto::merkle::{MerklePath, MerkleStore, PartialMerkleTree},
    Digest, Felt, FieldElement,
};
use miden_stdlib::StdLibrary;
use miden_vm::{execute, AdviceInputs, DefaultHost, MemAdviceProvider, StackInputs};

/// Stack inputs:
/// [num_accounts_updated,
///  ACCOUNT_ROOT,
///  NEW_ACCOUNT_HASH_0, account_id_0, ... , NEW_ACCOUNT_HASH_n, account_id_n]
const ACCOUNT_UPDATE_ROOT_MASM: &'static str = "
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

/// `current_account_states`: iterator of (account id, node hash, Merkle path)
/// `account_updates`: iterator of (account id, new account hash)
pub fn compute_new_account_root(
    current_account_states: impl Iterator<Item = (AccountId, Digest, MerklePath)>,
    account_updates: impl Iterator<Item = (AccountId, Digest)>,
    initial_account_root: Digest,
) -> Digest {
    let account_program = {
        let assembler = Assembler::default()
            .with_library(&StdLibrary::default())
            .expect("failed to load std-lib");

        assembler
            .compile(ACCOUNT_UPDATE_ROOT_MASM)
            .expect("failed to load account update program")
    };

    let host = {
        let advice_inputs = {
            let merkle_store =
                MerkleStore::default()
                    .add_merkle_paths(current_account_states.map(
                        |(account_id, node_hash, path)| (u64::from(account_id), node_hash, path),
                    ))
                    .expect("Account SMT has depth 64; all keys are valid");

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
        let mut num_accounts_updated: u32 = 0;
        for (idx, (account_id, new_account_hash)) in account_updates.enumerate() {
            stack_inputs.push(account_id.into());
            stack_inputs.append(new_account_hash.as_elements());

            num_accounts_updated = idx + 1;
        }

        // append initial account root
        stack_inputs.append(initial_account_root.as_elements());

        // append number of accounts updated
        stack_inputs.push(num_accounts_updated.into());

        StackInputs::new(stack_inputs)
    };

    let execution_output =
        execute(&account_program, stack_inputs, host, ExecutionOptions::default())
            .expect("execution error in account update program");

    let new_account_root = {
        let stack_output = execution_output.stack_outputs().stack_truncated(4);

        stack_output
            .into_iter()
            .map(|num| Felt::try_from(num).expect("account update program returned invalid Felt"))
            // We reverse, since a word `[a, b, c, d]` will be stored on the stack as `[d, c, b, a]`
            .rev()
            .try_into()
            .expect("account update program didn't return a proper RpoDigest")
    };

    new_account_root
}
