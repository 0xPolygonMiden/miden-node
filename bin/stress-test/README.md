# Miden stress test

This crate contains a binary for running Miden node stress tests.

The binary seeds a store with newly generated accounts. For each block, it first creates a faucet transaction that sends assets to multiple accounts by emitting notes, then adds transactions that consume these notes for each new account.

Once it's finished, it prints out several metrics.

## Example

After building the binary, you can run the following command to generate one million accounts:

`miden-node-stress-test seed-store --data-directory ./data --num-accounts 1000000`

The store file will then be located at `./data/miden-store.sqlite3`.

## License
This project is [MIT licensed](../../LICENSE).
