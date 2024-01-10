# Block Producer

**Block producer** accepts transactions from the RPC component, creates blocks containing those transactions, and 
sends them to the store. 
**Block Producer** is one of components of the [Miden node](..). 

## Architecture

The Miden node is still under heavy development and current architecture is subject of change. 
This topic will be filled later.

## Usage

### Installing the Block Producer

Block Producer can be installed and run as a part of [Miden node](../README.md#installing-the-node). 
But if you intend on running Block Producer as a separated process, you need to install and run it separately:

```sh
# Installs `miden-node-block-producer` executable
cargo install --path block-producer
```

To run the Block Producer you'll need to provide a configuration file. We have an example config file in [block-producer-example.toml](block-producer-example.toml).

Then, to run the Block Producer:

```sh
miden-node-block-producer serve --config <path-to-block-producer-config-file>
```

## API

**Block Producer** serves connections using [gRPC protocol](https://grpc.io) on a port, set in configuration file. Here is a brief
description of supported methods.

### SubmitProvenTransaction

Submits proven transaction to the Miden network.

**Parameters**

* `transaction`: `bytes` - transaction encoded using Miden's native format.

**Returns**

This method doesn't return any data.

## License
This project is [MIT licensed](../LICENSE).