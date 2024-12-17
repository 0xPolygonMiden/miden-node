# Miden block producer

The **Block producer** receives transactions from the RPC component, processes them, creates block containing those transactions before sending created blocks to the store. 

**Block Producer** is one of the components of the [Miden node](..). 

## Architecture

`TODO`

## Usage

### Installing the Block Producer

The Block Producer can be installed and run as part of [Miden node](../README.md#installing-the-node). 

## API

The **Block Producer** serves connections using the [gRPC protocol](https://grpc.io) on a port, set in the previously mentioned configuration file. 
Here is a brief description of supported methods.

### SubmitProvenTransaction

Submits a proven transaction to the Miden network.

**Parameters**

* `transaction`: `bytes` - transaction encoded using Miden's native format.

**Returns**

This method doesn't return any data.

## License
This project is [MIT licensed](../../LICENSE).
