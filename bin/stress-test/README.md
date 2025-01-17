# Miden stress test

This crate contains a binary for running Miden node stress tests.

## Running seed-store
This binary seeds the store with newly generated accounts. Also generates a faucet and sends asset to each new account. The result is the database dump file. To run it, a genesis file is needed (see [make-genesis](../../README.md#setup) command of the Miden node)

Run the following command:
```bash
cargo run --release seed-store --accounts-number <ACCOUNTS_NUMBER> --dump-file <DUMP_FILE> --genesis-file <GENESIS_FILE>
```

Once it's finished, it prints the average block insertion time.

## License
This project is [MIT licensed](../../LICENSE).
