# Store

**Store** maintains the state of the chain. It serves as the "source of truth" for the chain - i.e., if it is not in 
the store, the node does not consider it to be a part of the chain. 
**Store** is one of components of the [Miden node](..).

## Architecture

The Miden node is still under heavy development and current architecture is subject of change. This topic will be 
filled later.

## Usage

### Installing the Store

Store is being installed and run as a part of [Miden node](../README.md#installing-the-node).
But if you intend on running Store as a separated process, you need to install and run it separately:

```sh
# Installs `miden-node-store` executable
cargo install --path store
```

In order to run Store, you must provide genesis file. To generate genesis file you need to use [Miden node](../README.md#generating-the-genesis-file)'s `make-genesis` command. 


You'll also need to provide a configuration file. We have an example config file in [store-example.toml](store-example.toml).

Then, to run the Store:

```sh
miden-node-store serve --config <path-to-store-config-file>
```

## API

**Store** serves connections using [gRPC protocol](https://grpc.io) on a port, set in configuration file. Here is a brief
description of supported methods.

### ApplyBlock

Applies changes of a new block to the DB and in-memory data structures.

**Parameters**

* `block`: `BlockHeader` – block header ([src](../proto/proto/block_header.proto)).
* `accounts`: `[AccountUpdate]` – a list of account updates.
* `nullifiers`: `[Digest]` – a list of nullifier hashes.
* `notes`: `[NoteCreated]` – a list of notes created.

**Returns**

This method doesn't return any data.

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

### GetBlockInputs

Returns data needed by the block producer to construct and prove the next block.

**Parameters**

* `account_ids`: `[AccountId]` – array of account IDs. 
* `nullifiers`: `[Digest]` – array of nullifier hashes (not currently in use).

**Returns**

* `block_header`: `[BlockHeader]` – the latest block header.
* `mmr_peaks`: `[Digest]` – peaks of the above block's mmr, The `forest` value is equal to the block number.
* `account_states`: `[AccountBlockInputRecord]` – the hashes of the requested accouts and their authentication paths.
* `nullifiers`: `[NullifierBlockInputRecord]` – the requested nullifiers and their authentication paths.

### GetTransactionInputs

Returns account and nullifiers descriptors. 

**Parameters**

* `account_id`: `AccountId` – account ID.
* `nullifiers`: `[Digest]` – array of nullifier hashes.

**Returns**

* `account_state`: `AccountTransactionInputRecord` – account's descriptors. 
* `nullifiers`: `[NullifierTransactionInputRecord]` – the requested nullifiers' blocks at which ones have been consumed, zero if not consumed.

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

### ListNullifiers

Lists all nullifiers of the current chain.

**Parameters**

This request doesn't have any request parameters.

**Returns**

* `nullifiers`: `[NullifierLeaf]` – lists of all nullifiers of the current chain. 

### ListAccounts

Lists all accounts of the current chain.

**Parameters**

This request doesn't have any request parameters.

**Returns**

* `accounts`: `[AccountInfo]` – list of all accounts of the current chain.

### ListNotes

Lists all notes of the current chain.

**Parameters**

This request doesn't have any request parameters.

**Returns**

* `notes`: `[Note]` – list of all notes of the current chain.

## License
This project is [MIT licensed](../LICENSE).