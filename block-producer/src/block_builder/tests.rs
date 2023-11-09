// prover tests
// 1. account validation works
// 2. `BlockProver::compute_account_root()` works
//   + make the updates outside the VM, and compare root

// block builder tests (higher level)
// 1. `apply_block()` is called
// 2. if `apply_block()` fails, you fail too
