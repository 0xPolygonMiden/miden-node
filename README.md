# Miden node

[![LICENSE](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/0xPolygonMiden/miden-node/blob/main/LICENSE)
[![test](https://github.com/0xPolygonMiden/miden-node/actions/workflows/test.yml/badge.svg)](https://github.com/0xPolygonMiden/miden-node/actions/workflows/test.yml)
[![RUST_VERSION](https://img.shields.io/badge/rustc-1.80+-lightgray.svg)](https://www.rust-lang.org/tools/install)
[![crates.io](https://img.shields.io/crates/v/miden-node)](https://crates.io/crates/miden-node)

This repository holds the Miden node; that is, the software which processes transactions and creates blocks for the Miden rollup.

### Status

The Miden node is still under heavy development and the project can be considered to be in an _alpha_ stage. Many features are yet to be implemented and there are a number of limitations which we will lift in the near future.

At this point, we are developing the Miden node for a centralized operator. As such, the work does not yet include components such as P2P networking and consensus. These will be added in the future.

## Documentation

The documentation can be found [here](./docs/index.md).

## Installation

The node software can be installed as a Debian package or using Rust's package manager `cargo`.

Official releases are available as debian packages which can be found under our [releases](https://github.com/0xPolygonMiden/miden-node/releases) page.

Alternatively, the Rust package manager `cargo` can be used to install on non-debian distributions or to compile from source.

### Debian package

Debian packages are available and are the fastest way to install the node on a Debian-based system. Currently only `amd64` architecture are supported.

These packages can be found under our [releases](https://github.com/0xPolygonMiden/miden-node/releases) page along with a checksum.

Note that this includes a `systemd` service called `miden-node` (disabled by default).

To install, download the desired releases `.deb` package and checksum files. Install using

```sh
sudo dpkg -i $package_name.deb
```

> [!TIP]
> You should verify the checksum using a SHA256 utility. This differs from platform to platform, but on most linux distros:
> ```sh
> sha256sum --check $checksum_file.deb.checksum
> ```
> can be used so long as the checksum file and the package file are in the same folder.

### Install using `cargo`

Install Rust version **1.80** or greater using the official Rust installation [instructions](https://www.rust-lang.org/tools/install).

Depending on the platform, you may need to install additional libraries. For example, on Ubuntu 22.04 the following command ensures that all required libraries are installed.

```sh
sudo apt install llvm clang bindgen pkg-config libssl-dev libsqlite3-dev
```

Install the node binary for production using `cargo`:

```sh
cargo install miden-node --locked
```

This will install the latest official version of the node. You can install a specific version using `--version <x.y.z>`:

```sh
cargo install miden-node --locked --version x.y.z
```

You can also use `cargo` to compile the node from the source code if for some reason you need a specific git revision. Note that since these aren't official releases we cannot provide much support for any issues you run into, so consider this for advanced users only. The incantation is a little different as you'll be targetting this repo instead: 

```sh
# Install from a specific branch
cargo install --locked --git https://github.com/0xPolygonMiden/miden-node miden-node --branch <branch>

# Install a specific tag
cargo install --locked --git https://github.com/0xPolygonMiden/miden-node miden-node --tag <tag>

# Install a specific git revision
cargo install --locked --git https://github.com/0xPolygonMiden/miden-node miden-node --rev <git-sha>
```

More information on the various options can be found [here](https://doc.rust-lang.org/cargo/commands/cargo-install.html#install-options).

> [!TIP]
> Miden account generation uses a proof-of-work puzzle to prevent DoS attacks. These puzzles can be quite expensive, especially for test purposes. You can lower the difficulty of the puzzle by appending `--features testing` to the `cargo install ..` invocation. For example:
> ```sh
> cargo install miden-node --locked --features testing
> ```

### Verify installation

You can verify the installation by checking the node's version:

```sh
miden-node --version
```

## Usage

### Setup

Decide on a location to store all the node data and configuration files in. This guide will use the placeholder `<STORAGE>` and `<CONFIG>` to represent these directories. They are allowed to be the same, though most unix distributions have conventions for these being `/opt/miden` and `/etc/miden` respectively. Note that if you intend to use the `systemd` service then by default it expects these conventions to be upheld.

We need to configure the node as well as bootstrap the chain by creating the genesis block. Generate the default configurations for both:

```sh
miden-node init \
  --config-path  <CONFIG>/miden-node.toml \
  --genesis-path <CONFIG>/genesis.toml  
```

which will generate `miden-node.toml` and `genesis.toml` files. The latter controls the accounts that the genesis block will be spawned with and by default includes a basic wallet account and a basic fungible faucet account. You can modify this file to add/remove accounts as desired.

Next, bootstrap the chain by generating the genesis data:

```sh
miden-node make-genesis \
  --input-path  <CONFIG>/genesis.toml \
  --output-path <STORAGE>/genesis.dat
```

which will create `genesis.dat` and an `accounts` directory containing account data based on the `genesis.toml` file.

> [!NOTE]
> `make-genesis` will take a long time if you're running the production version of `miden-node`, see the tip in the [installation](#install-using-`cargo`) section.

Modify the `miden-node.toml` configuration file such that the `[store]` paths point to our `<STORAGE>` folder:

```toml
[store]
database_filepath = "<STORAGE>/miden-store.sqlite3"
genesis_filepath  = "<STORAGE>/genesis.dat"
blockstore_dir    = "<STORAGE>/blocks"
```

Finally, configure the node's endpoints to your liking.

### Systemd

An example service file is provided [here](packaging/miden-node.service). If you used the Debian package installer then this service was already installed alongside it.

### Running the node

Using the node configuration file created in the previous step, start the node:

```sh
miden-node start \
  --config <CONFIG>/miden-node.toml \
  node
```

or alternatively start the systemd service if that's how you wish to operate:

```sh
systemctl start miden-node.service
```

## Updating

We currently make no guarantees about backwards compatibility. Updating the node software therefore consists of wiping all existing data and re-installing the node's software again. This includes regenerating the configuration files and genesis block as these formats may have changed. This effectively means every update is a complete reset of the blockchain.

First stop the currently running node or systemd service then remove all existing data. If you followed the [Setup](#setup) section, then this can be achieved by deleting all information in `<STORAGE>`:

```sh
rm -rf <STORAGE>
```

> [!WARNING]
> Failure to remove existing node data could result in strange behaviour.

## Development

See our [contributing](CONTRIBUTING.md) guidelines and our [makefile](Makefile) for example workflows e.g. run the testsuite using

```sh
make test
``` 

## License

This project is [MIT licensed](./LICENSE).
