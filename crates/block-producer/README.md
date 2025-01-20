# Miden block producer

The **Block producer** receives transactions from the RPC component, processes them, creates block containing those transactions before sending created blocks to the store. 

**Block Producer** is one of components of the [Miden node](..). 

## Architecture

`TODO`

## Usage

### Installing the Block Producer

The Block Producer can be installed and run as part of [Miden node](../README.md#installing-the-node). 

## API

The **Block Producer** serves connections using the [gRPC protocol](https://grpc.io) on a port, set in the previously mentioned configuration file. 
Here is a brief description of supported methods.

### SubmitProvenTransaction

Submits proven transaction to the Miden network.

**Parameters**

* `transaction`: `bytes` - transaction encoded using [winter_utils::Serializable](https://github.com/facebook/winterfell/blob/main/utils/core/src/serde/mod.rs#L26) implementation for [miden_objects::transaction::proven_tx::ProvenTransaction](https://github.com/0xPolygonMiden/miden-base/blob/main/objects/src/transaction/proven_tx.rs#L22).

**Returns**

This method doesn't return any data.

## Crate Features

- `tracing-forest`: sets logging using tracing-forest. Disabled by default.
- `testing`: includes testing util functions to mock block-producer behaviour, meant to be used during development and not on production. Disabled by default.

## License
This project is [MIT licensed](../../LICENSE).
