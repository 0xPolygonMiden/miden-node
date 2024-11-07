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

Full API documentation located [here](../../docs/api.md).

## License
This project is [MIT licensed](../../LICENSE).