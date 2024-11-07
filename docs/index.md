# Miden Node Documentation

## Architecture

The Miden node consists of 3 main components, which communicate using gRPC:

- **[RPC](../crates/rpc):** an externally-facing component through which clients can interact with the node. It receives client requests (e.g., to synchronize with the latest state of the chain, or to submit transactions), performs basic validation, and forwards the requests to the appropriate internal components.
- **[Store](../crates/store):** maintains the state of the chain. It serves as the "source of truth" for the chain - i.e., if it is not in the store, the node does not consider it to be part of the chain.
- **[Block Producer](../crates/block-producer):** accepts transactions from the RPC component, creates blocks containing those transactions, and sends them to the store.

All 3 components can either run as one process, or each component can run in its own process. See the [Running the node](#running-the-node) section for more details.

The diagram below illustrates high-level design of each component as well as basic interactions between them (components in light-grey are yet to be built).

![Architecture diagram](../assets/architecture.png)

## API Reference

Node components serve connections using the [gRPC protocol](https://grpc.io). Full gRPC API documentation reference located [here](api.md).