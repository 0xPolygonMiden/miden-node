# Miden node RPC

The **RPC** is an externally-facing component through which clients can interact with the node. It receives client requests
(e.g., to synchronize with the latest state of the chain, or to submit transactions), performs basic validation,
and forwards the requests to the appropriate components.
**RPC** is one of components of the [Miden node](..).

## Architecture

`TODO`

## Usage

### Installing the RPC

The RPC can be installed and run as part of [Miden node](../README.md#installing-the-node).

## API

The **RPC** serves connections using the [gRPC protocol](https://grpc.io) on a port, set in the previously mentioned configuration file.

Full API documentation located [here](../../docs/api.md).

## License

This project is [MIT licensed](../../LICENSE).
