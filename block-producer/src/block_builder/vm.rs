use miden_objects::{assembly::Assembler, Digest};
use miden_stdlib::StdLibrary;

const NULLIFIER_PROGRAM_SOURCE: &'static str = "
    begin

    end
";

pub fn compute_new_nullifier_root() -> Digest {
    let nullifier_program = {
        let assembler = Assembler::default()
            .with_library(&StdLibrary::default())
            .expect("failed to load std-lib");

        assembler
            .compile(NULLIFIER_PROGRAM_SOURCE)
            .expect("failed to load account update program")
    };

    todo!()
}
