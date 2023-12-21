# Miden node

This repository holds the Miden node; that is, the software which processes transactions and creates
blocks.

The node is made up of 3 main components: 
- **store:** manages the databases, 
- **rpc:** listens for new transactions to be added to blocks
- **block producer:** takes new transactions from the store, creates blocks containing those
  transactions, and sends them to the store

We currently have a restriction that for any account `A`, only one transaction per block can be
about `A`. We intend to lift that restriction in the near future.

# Usage

Before running the node, you must first generate the genesis file. 

## Generating the genesis file

The contents of the genesis file are currently hardcoded in Rust, but we intend to make those
configurable shortly. The genesis block currently sets up 2 accounts: a faucet account for a `POL`
token, as well as a wallet account.

To generate the file for production, run 

```sh
$ cargo run -p miden-node -- make-genesis
```

However, you will notice that this can take many minutes to execute. To generate the file for
testing purposes, run

```sh
$ cargo run -p miden-node --features testing -- make-genesis
```

This will generate 3 files in the current directory: 
- `genesis.dat`: the genesis file
- `faucet.fsk` and `wallet.fsk`: the public/private keys of the faucet and wallet accounts, respectively

## Running the node

There are 2 ways to run the node: all 3 components in one process, or each component in its own
process. Each executable will require a configuration file. Each directory containing the
executables also contains an example configuration file. For example, `node/miden-node-example.toml`
is the example configuration file for running all the components in the same process. Notably, the
`store.genesis_filepath` field must point to the `genesis.dat` file that you generated in the
previous step.

To run all components in the same process:

```sh
$ cd node
$ cargo run -- start -c <path-to-config-file>
```


To run components separately, run

```sh
$ cargo run -p miden-node-store -- serve --config <path-to-store-config-file>

# In a separate terminal
$ cargo run -p miden-node-rpc -- serve --config <path-to-rpc-config-file>

# In a separate terminal
$ cargo run -p miden-node-block-producer -- serve --config <path-to-block-producer-config-file>
```

Make sure that the configuration files are mutually consistent. That is, make sure that the URLs are
valid and point to the right endpoint.
