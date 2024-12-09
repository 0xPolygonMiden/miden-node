# Miden node store

The **Store** maintains the state of the chain. It serves as the "source of truth" for the chain - i.e., if it is not in
the store, the node does not consider it to be part of the chain.
**Store** is one of components of the [Miden node](..).

## Architecture

`TODO`

## Usage

### Installing the Store

The Store can be installed and run as part of [Miden node](../README.md#installing-the-node).

## API

The **Store** serves connections using the [gRPC protocol](https://grpc.io) on a port, set in the previously mentioned configuration file.

Full API documentation located [here](../../docs/api.md).

## License

This project is [MIT licensed](../../LICENSE).
