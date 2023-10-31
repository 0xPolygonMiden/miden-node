use miden_objects::{
    assembly::Assembler,
    crypto::merkle::{MerkleStore, PartialMerkleTree},
    Digest, Felt, FieldElement,
};
use miden_stdlib::StdLibrary;
use miden_vm::{AdviceInputs, DefaultHost, MemAdviceProvider};

const NULLIFIER_PROGRAM_SOURCE: &'static str = "
    begin

    end
";

/// This method assumes that all the notes associated with the nullifier have *not* been consumed.
pub fn compute_new_nullifier_root(
    pmt: &PartialMerkleTree,
    nullifiers: impl Iterator<Item = [u8; 32]>,
) -> Digest {
    let nullifier_program = {
        let assembler = Assembler::default()
            .with_library(&StdLibrary::default())
            .expect("failed to load std-lib");

        assembler
            .compile(NULLIFIER_PROGRAM_SOURCE)
            .expect("failed to load account update program")
    };

    let host = {
        let advice_inputs = {
            let merkle_store: MerkleStore = pmt.into();
            let map = nullifiers.map(|nullifier| (nullifier, vec![Felt::ZERO; 4]));

            AdviceInputs::default().with_merkle_store(merkle_store).with_map(map)
        };

        let advice_provider = MemAdviceProvider::from(advice_inputs);

        DefaultHost::new(advice_provider)
    };

    todo!()
}
