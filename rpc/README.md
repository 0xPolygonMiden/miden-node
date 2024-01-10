# RPC

**RPC** is an externally-facing component through which clients can interact with the node. It receives client requests 
(e.g., to synchronize with the latest state of the chain, or to submit transactions), performs basic validation, 
and forwards the requests to the appropriate internal components.
**RPC** is one of components of the [Miden node](..).

## Architecture

The Miden node is still under heavy development and current architecture is subject of change. This topic will be 
filled later.

## Usage

### Installing RPC

RPC is being installed and run as a part of [Miden node](../README.md#installing-the-node).
But if you intend on running RPC as a separated process, you need to install and run it separately:

```sh
# Installs `miden-node-rpc` executable
cargo install --path rpc
```

To run the RPC you'll need to provide a configuration file. We have an example config file in [rpc-example.toml](rpc-example.toml).

Then, to run an RPC:

```sh
miden-node-rpc serve --config <path-to-rpc-config-file>
```

## API

**RPC** serves connections using [gRPC protocol](https://grpc.io) on a port, set in configuration file. Here is a brief 
description of supported methods.

### CheckNullifiers

Get a list of proofs for given nullifier hashes, each proof as Tiered Sparse Merkle Trees ([read more](../proto/proto/tsmt.proto)). 

**Parameters:**

* `nullifiers`: `[Digest]` – array of nullifier hashes.

**Returns:**

* `proofs`: `[NullifierProof]` – array of nullifier proofs, positions correspond to the ones in request.

### GetBlockHeaderByNumber

Retrieves block header by given block number.

**Parameters**

* `block_num`: `uint32` *(optional)* – the block number of the target block. If not provided, means latest know block.

**Returns:**

* `block_header`: `BlockHeader` – block header.

### SyncState

State synchronization request.

**Parameters**

* `block_num`: `uint32` – send updates to the client starting at this block.
* `account_ids`: `[AccountId]` 
* `note_tags`: `[uint32]` – note tags filter. Corresponds to the high 16 bits of the real values, shifted right (`value >> 48`). 
* `nullifiers`: `[uint32]` – nullifiers filter. Corresponds to the high 16 bits of the real values, shifted right (`value >> 48`).

**Returns**

* `chain_tip`: `uint32` – number of the latest block in the chain.
* `block_header`: `BlockHeader` – block header of the block with the first note matching the specified criteria.
* `mmr_delta`: `MmrDelta` – data needed to update the partial MMR from `block_ref` to `block_header.block_num`.
* `block_path`: `MerklePath` – Merkle path in the updated chain MMR to the block at `block_header.block_num`.
* `accounts`: `[AccountHashUpdate]` – a list of account hashes updated after `block_ref` but not after `block_header.block_num`.
* `notes`: `[NoteSyncRecord]` – a list of all notes together with the Merkle paths from `block_header.note_root`.
* `nullifiers`: `[NullifierUpdate]` – a list of nullifiers created between `block_ref` and `block_header.block_num`.

### SubmitProvenTransaction

Submits proven transaction to the Miden network.

**Parameters**

* `transaction`: `bytes` - transaction encoded using Miden's native format.

**Returns**

This method doesn't return any data.

## License
This project is [MIT licensed](../LICENSE).