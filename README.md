# Miden node

<a href="https://github.com/0xPolygonMiden/miden-node/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
<img src="https://github.com/0xPolygonMiden/miden-node/workflows/CI/badge.svg?branch=main">
<a href="https://crates.io/crates/miden-node"><img src="https://img.shields.io/crates/v/miden-node"></a>

This repository holds the Miden node; that is, the software which processes transactions and creates blocks for the Miden rollup.

### Status

The Miden node is still under heavy development and the project can be considered to be in an *alpha* stage. Many features are yet to be implemented and there is a number of limitations which we will lift in the near future.

At this point, we are developing the Miden node for a centralized operator. Thus, the work does not yet include such components as P2P networking and consensus. These also will be added in the future.

## Architecture


The Miden node is made up of 3 main components, which communicate over gRPC: 
- **store:** stores the current state of the chain.
- **rpc:** serves client requests such as to synchronize with the latest state of the chain or to submit transactions.
- **block producer:** accepts transactions from the RPC component, creates blocks containing those transactions, and sends them to the store.

![Architecture diagram](./assets/architecture.png)

All 3 components can either run in one process, or each component can run in its own process. See the [Running the node](#running-the-node) section for more details.

## Usage

Before you can run the Miden node, you'll need to make sure you have Rust [installed](https://www.rust-lang.org/tools/install). Miden node v0.1 requires Rust version **1.73** or later.

Before running the node, you must first generate the genesis file. 

The `miden-node` executable is used to both generate the genesis file, and running the node.

### Installing the node

To install for production use cases, run

```sh
$ cargo install --path node
```

This will install the executable `miden-node` in your PATH, at `~/.cargo/bin/miden-node`.

Otherwise, if only to try the node out for testing, run

```sh
$ cargo install --features testing --path node
```

Currently, the only difference between the 2 is how long the `make-genesis` command will take to run (see next subsection).

### Generating the genesis file

The contents of the genesis file are currently hardcoded in Rust, but we intend to make those configurable shortly. The genesis block currently sets up 2 accounts: a faucet account for a `POL` token, as well as a wallet account.

To generate the genesis file, run 

```sh
$ miden-node make-genesis
```

This will generate 3 files in the current directory: 
- `genesis.dat`: the genesis file.
- `faucet.fsk` and `wallet.fsk`: the public/private keys of the faucet and wallet accounts, respectively.

### Running the node

Each executable will require a configuration file. Each directory containing the executables also contains an example configuration file. For example, [`node/miden.toml`](/node/miden.toml) is the example configuration file for running all the components in the same process. Notably, the`store.genesis_filepath` field must point to the `genesis.dat` file that you generated in the previous step.

To run all components in the same process:

```sh
$ miden-node start -c <path-to-config-file>
```

### Advanced usage

If you intend on running the node in different processes, you need to install each component separately:

```sh
# Installs `miden-node-store` executable
$ cargo install --path store

# Installs `miden-node-rpc` executable
$ cargo install --path rpc

# Installs `miden-node-block-producer` executable
$ cargo install --path block-producer
```

Then, to run each component,

```sh
$ miden-node-store serve --sqlite <path-to-sqlite3-database-file> --config <path-to-store-config-file>

# In a separate terminal
$ miden-node-rpc serve --config <path-to-rpc-config-file>

# In a separate terminal
$ miden-node-block-producer serve --config <path-to-block-producer-config-file>
```

Make sure that the configuration files are mutually consistent. That is, make sure that the URLs are valid and point to the right endpoint.

## License
This project is [MIT licensed](./LICENSE).
