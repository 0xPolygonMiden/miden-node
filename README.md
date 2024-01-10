# Miden node

<a href="https://github.com/0xPolygonMiden/miden-node/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
<a href="https://github.com/0xPolygonMiden/miden-node/actions/workflows/ci.yml"><img src="https://github.com/0xPolygonMiden/miden-node/actions/workflows/ci.yml/badge.svg?branch=main"></a>
<a href="https://crates.io/crates/miden-node"><img src="https://img.shields.io/crates/v/miden-node"></a>

This repository holds the Miden node; that is, the software which processes transactions and creates blocks for the Miden rollup.

### Status

The Miden node is still under heavy development and the project can be considered to be in an *alpha* stage. Many features are yet to be implemented and there is a number of limitations which we will lift in the near future.

At this point, we are developing the Miden node for a centralized operator. Thus, the work does not yet include such components as P2P networking and consensus. These will also be added in the future.

## Architecture

The Miden node is made up of 3 main components, which communicate over gRPC:
- **[RPC](rpc):** an externally-facing component through which clients can interact with the node. It receives client requests (e.g., to synchronize with the latest state of the chain, or to submit transactions), performs basic validation, and forwards the requests to the appropriate internal components.
- **[Store](store):** maintains the state of the chain. It serves as the "source of truth" for the chain - i.e., if it is not in the store, the node does not consider it to be a part of the chain.
- **[Block Producer](block-producer):** accepts transactions from the RPC component, creates blocks containing those transactions, and sends them to the store.

All 3 components can either run in one process, or each component can run in its own process. See the [Running the node](#running-the-node) section for more details.

The diagram below illustrates high-level design of each component as well as basic interactions between them (components in light-grey are yet to be built).

![Architecture diagram](./assets/architecture.png)

## Usage

### Prerequisites

Before you can build and run the Miden node or any of its components, you'll need to make sure you have Rust [installed](https://www.rust-lang.org/tools/install). Miden node v0.1 requires Rust version **1.73** or later.

Also make sure all of this libraries are installed:

Ubuntu 22.04:

```sh
sudo apt install gcc llvm clang bindgen pkg-config libssl-dev libsqlite3-dev
```

macOS:

```sh
xcode-select --install
brew install buf llvm openssl pkg-config sqlite 
```

### Installing the node

To install for production use cases, run:
```sh
cargo install --path node
```

This will install the executable `miden-node` in your PATH, at `~/.cargo/bin/miden-node`.

Otherwise, if only to try the node out for testing, run:
```sh
cargo install --features testing --path node
```

Currently, the only difference between the two is how long the `make-genesis` command will take to run (see next subsection).

### Generating the genesis file

Before running the node, you must first generate the genesis file. The contents of the genesis file are currently hardcoded in Rust, but we intend to make these configurable shortly. The genesis block currently sets up 2 accounts: a faucet account for a `POL` token, as well as a wallet account.

To generate the genesis file, run:
```sh
miden-node make-genesis
```

This will generate 3 files in the current directory: 
- `genesis.dat`: the genesis file.
- `faucet.fsk` and `wallet.fsk`: the public/private keys of the faucet and wallet accounts, respectively.

### Running the node

To run the node you'll need to provide a configuration file. We have an example config file in [node/miden.toml](/node/miden.toml). Then, to run the node, run:

```sh
miden-node start --config <path-to-config-file>
```

Or, if your config file is in named `miden.toml` and is in the current directory, you can simply run:
```sh
miden-node start
```

Note that the`store.genesis_filepath` field in the config file must point to the `genesis.dat` file that you generated in the previous step.

### Running the node as separate components

If you intend on running the node in different processes, you need to install and run each component separately.
Please, refer to each component's documentation:

* [RPC](rpc/README.md#usage)
* [Store](store/README.md#usage)
* [Block Producer](block-producer/README.md#usage)

Each directory containing the executables also contains an example configuration file. Make sure that the configuration files are mutually consistent. That is, make sure that the URLs are valid and point to the right endpoint.

## License
This project is [MIT licensed](./LICENSE).
