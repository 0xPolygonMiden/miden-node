use miden_objects::{
    assembly::Assembler,
    crypto::merkle::{MerklePath, MerkleStore, PartialMerkleTree},
    Digest, Felt, FieldElement,
};
use miden_stdlib::StdLibrary;
use miden_vm::{AdviceInputs, DefaultHost, MemAdviceProvider};

const ACCOUNT_PROGRAM_SOURCE: &'static str = "
    begin

    end
";

/// Takes an iterator of (account id, node hash, Merkle path)
pub fn compute_new_account_root(
    accounts: impl Iterator<(AccountId, Digest, MerklePath)>
) -> Digest {
    let account_program = {
        let assembler = Assembler::default()
            .with_library(&StdLibrary::default())
            .expect("failed to load std-lib");

        assembler
            .compile(ACCOUNT_PROGRAM_SOURCE)
            .expect("failed to load account update program")
    };

    let host = {
        let advice_inputs = {
            let merkle_store =
                MerkleStore::default()
                    .add_merkle_paths(accounts.map(|(account_id, node_hash, path)| {
                        (u64::from(account_id), node_hash, path)
                    }))
                    .expect("Account SMT has depth 64; all keys are valid");

            AdviceInputs::default().with_merkle_store(merkle_store)
        };

        let advice_provider = MemAdviceProvider::from(advice_inputs);

        DefaultHost::new(advice_provider)
    };

    todo!()
}
