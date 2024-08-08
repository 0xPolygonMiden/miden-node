# Miden node

[![LICENSE](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/0xPolygonMiden/miden-node/blob/main/LICENSE)
[![test](https://github.com/0xPolygonMiden/miden-node/actions/workflows/test.yml/badge.svg)](https://github.com/0xPolygonMiden/miden-node/actions/workflows/test.yml)
[![RUST_VERSION](https://img.shields.io/badge/rustc-1.78+-lightgray.svg)](https://www.rust-lang.org/tools/install)
[![crates.io](https://img.shields.io/crates/v/miden-node)](https://crates.io/crates/miden-node)

This repository holds the Miden node; that is, the software which processes transactions and creates blocks for the Miden rollup.

### Status

The Miden node is still under heavy development and the project can be considered to be in an _alpha_ stage. Many features are yet to be implemented and there is a number of limitations which we will lift in the near future.

At this point, we are developing the Miden node for a centralized operator. As such, the work does not yet include components such as P2P networking and consensus. These will be added in the future.

## Architecture

The Miden node consists of 3 main components, which communicate using gRPC:

- **[RPC](crates/rpc):** an externally-facing component through which clients can interact with the node. It receives client requests (e.g., to synchronize with the latest state of the chain, or to submit transactions), performs basic validation, and forwards the requests to the appropriate internal components.
- **[Store](crates/store):** maintains the state of the chain. It serves as the "source of truth" for the chain - i.e., if it is not in the store, the node does not consider it to be part of the chain.
- **[Block Producer](crates/block-producer):** accepts transactions from the RPC component, creates blocks containing those transactions, and sends them to the store.

All 3 components can either run as one process, or each component can run in its own process. See the [Running the node](#running-the-node) section for more details.

The diagram below illustrates high-level design of each component as well as basic interactions between them (components in light-grey are yet to be built).

![Architecture diagram](./assets/architecture.png)

## Usage

Before you can build and run the Miden node or any of its components, you'll need to make sure you have Rust [installed](https://www.rust-lang.org/tools/install). Miden node requires Rust version **1.78** or later.

Depending on the platform, you may need to install additional libraries. For example, on Ubuntu 22.04 the following command ensures that all required libraries are installed.

```sh
sudo apt install llvm clang bindgen pkg-config libssl-dev libsqlite3-dev
```

### Installing the node

> [!NOTE]
> This guide describes running the node as a single process. To run components in separate processes, please refer to each component's documentation:
> - [RPC](crates/rpc/README.md#usage)
> - [Store](crates/store/README.md#usage)
> - [Block Producer](crates/block-producer/README.md#usage)

Install the node binary for production using `cargo`:


```sh
cargo install miden-node
```

> [!TIP]
> Miden account generation uses a proof-of-work puzzle to prevent DoS attacks. These puzzles can be quite expensive, especially for test purposes. You can lower the difficulty of the puzzle by installing with the `testing` feature enabled:
> ```sh
> cargo install miden-node --features testing
> ```

The resulting binary can be found in `~/.cargo/bin` and should already be available in your `PATH`. Confirm that installation succeeded by checking the node version:

```sh
miden-node --version
```
which should print `miden-node <version>`.

### Configuration

Select a folder to store all the node data and configuration files in. This guide will use the placeholder `<..>` to represent this folder.

We need to configure the node as well as bootstrap the chain by creating the genesis block. Generate the default configurations for both:

```sh
miden-node init \
  --config-path <..>/miden-node.toml \
  --genesis-path <..>/genesis.toml  
```

which will generate `miden-node.toml` and `genesis.toml` files. The latter controls the accounts that the genesis block will be spawned with and by default contains a basic wallet account and a basic fungible faucet account. You can modify this file to add/remove accounts as desired.

Next, bootstrap the chain by generating the genesis data:

```sh
miden-node make-genesis \
  --input-path <..>/genesis.toml \
  --output-path <..>/genesis.dat
```

which will create `genesis.dat` and an `accounts` directory containing account data based on the `genesis.toml` file.

> [!NOTE]
> `make-genesis` will take a long time if you're running the production version of `miden-node`, see the tip in the [installation](#installing-the-node) section.

Modify the `miden-node.toml` configuration file such that the `[store]` paths point to our `<..>` folder:

```toml
[store]
database_filepath = "<..>/miden-store.sqlite3"
genesis_filepath = "<..>/genesis.dat"
blockstore_dir = "<..>/blocks"
```

Finally, configure the node's endpoints to your liking.

### Running the node

Using the node configuration file created in the previous step, start the node:

```sh
miden-node start \
  --config <..>/miden-node.toml \
  node
```

### Updating the node

The node currently has no guarantees about backwards compatibility. Updating the node is therefore a simple matter of stopping the node, removing all data and re-installing it again.

### Running the node using Docker

If you intend on running the node inside a Docker container, you will need to follow these steps:

1. Build the docker image from source

   ```sh
   make docker-build-node
   ```

   This command will build the docker image for the Miden node and save it locally.

2. Run the Docker container

   ```sh
   # Using make
   make docker-run-node

   # Manually
   docker run --name miden-node -p 57291:57291 -d miden-node-image
   ```

   This command will run the node as a container named `miden-node` using the `miden-node-image` and make port `57291` available (rpc endpoint).

3. Monitor container

   ```sh
   docker ps
   ```

    After running this command you should see the name of the container `miden-node` being outputted and marked as `Up`.

### Debian Packages

The debian packages allow for easy install for miden on debian based systems. Note that there are checksums available for the package.
Current support is for amd64, arm64 support coming soon.

To install the debian package:

```sh
sudo dpkg -i $package_name.deb
```

Note, when using the debian package to run the `make-genesis` function, you should define the location of your output:

```sh
miden-node make-genesis -i $input_location_for_genesis.toml -o $output_for_genesis.dat_and_accounts
```

The debian package has a checksum, you can verify this checksum by downloading the debian package and checksum file to the same directory and running the following command:

```sh
sha256sum --check $checksumfile
```

Please make sure you have the sha256sum program installed, for most linux operating systems this is already installed. If you wish to install it on your macOS, you can use brew:

```sh
brew install coreutils
```

## Testing

In order to test the node run the following command:

```sh
make test
```

## License

This project is [MIT licensed](./LICENSE).
