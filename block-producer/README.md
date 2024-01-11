# Miden block producer

The **Block producer** receives transactions from the RPC component, processes them, creates block containing those transactions before sending created blocks to the store. 

**Block Producer** is one of components of the [Miden node](..). 

## Architecture

`TODO`

## Usage

### Installing the Block Producer

The Block Producer can be installed and run as part of [Miden node](../README.md#installing-the-node). 
But if you intend on running the Block Producer as a separate process, you will need to install and run it as follows:

```sh
# Installs `miden-node-block-producer` executable
cargo install --path block-producer
```

### Running the Block Producer

To run the Block Producer you will need to provide a configuration file. We have an example config file in [block-producer-example.toml](block-producer-example.toml).

Then, to run the Block Producer:

```sh
miden-node-block-producer serve --config <path-to-block-producer-config-file>
```

## API

The **Block Producer** serves connections using the [gRPC protocol](https://grpc.io) on a port, set in the previously mentioned configuration file. 
Here is a brief description of supported methods.

### SubmitProvenTransaction

Submits proven transaction to the Miden network.

**Parameters**

* `transaction`: `bytes` - transaction encoded using Miden's native format.

**Returns**

This method doesn't return any data.

## License
This project is [MIT licensed](../LICENSE).