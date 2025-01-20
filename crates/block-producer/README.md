# Miden block producer

Contains code definining the [Miden node's block-producer](/README.md#architecture) component. It is responsible for
ordering transactions into blocks and submitting these for inclusion in the blockchain.

It serves a small [rRPC](htts://grpc.io) API which the node's RPC component uses to submit new transactions. In turn,
the `block-producer` uses the store's gRPC API to submit blocks and query chain state.

For more information on the installation and operation of this component, please see the [node's readme](/README.md).

## API

The full gRPC API can be found [here](../../proto/block_producer.proto).

---

### SubmitProvenTransaction

Submits proven transaction to the Miden network.

---

## License
This project is [MIT licensed](../../LICENSE).
