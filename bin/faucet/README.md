# Miden node

This crate contains a binary for running a Miden rollup faucet.

## Running the faucet
1. Run a local node, for example using the docker image. From the "miden-node" repo root run the following commands:
```bash
make docker-build-node
make docker-run-node
```

2. Install the faucet (with the "testing" feature):
```bash
make install-faucet-testing
```

3. Create the faucet configuration file:
```bash
miden-faucet init
```

4. Start the faucet server:
```bash
miden-faucet start
```

After a few seconds you may go to `http://localhost:8080` and see the faucet UI.

## License
This project is [MIT licensed](../../LICENSE).
