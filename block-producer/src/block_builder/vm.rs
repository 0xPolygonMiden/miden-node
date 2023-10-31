use miden_objects::{assembly::Assembler, Digest};
use miden_stdlib::StdLibrary;

const ACCOUNT_PROGRAM_SOURCE: &'static str = "

    begin

    end
";

pub fn compute_new_account_root() -> Digest {
    let account_program = {
        let assembler = Assembler::default()
            .with_library(&StdLibrary::default())
            .expect("failed to load std-lib");

        assembler
            .compile(ACCOUNT_PROGRAM_SOURCE)
            .expect("failed to load account update program")
    };

    todo!()
}
