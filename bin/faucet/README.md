# Miden node

This crate contains a binary for running a Miden rollup faucet.

## License
This project is [MIT licensed](../../LICENSE).

## Running the faucet
1. Run a local node, for example using the docker image. From the "miden-node" repo root run the following commands:
```bash
cargo make docker-build-node
cargo make docker-run-node
```

2. From the "miden-node" repo root run the faucet:
```bash
cargo run --bin miden-faucet  --features testing --release
```

After a few seconds you may go to `http://localhost:8080` and see the faucet UI.
