# Miden stress test

This crate contains a binary for running Miden node stress tests.

This binary seeds the store with newly generated accounts. Also generates a faucet and sends asset to each new account. The result is the database dump file. To run it, a genesis file is needed (see [make-genesis](../../README.md#setup) command of the Miden node).

Once it's finished, it prints the average block insertion time.

## License
This project is [MIT licensed](../../LICENSE).
