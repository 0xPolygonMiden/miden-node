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
Here is a brief description of supported methods.

### CheckNullifiers

Gets a list of proofs for given nullifier hashes, each proof as a sparse Merkle Trees

**Parameters:**

- `nullifiers`: `[Digest]` – array of nullifier hashes.

**Returns:**

- `proofs`: `[NullifierProof]` – array of nullifier proofs, positions correspond to the ones in request.

### GetBlockHeaderByNumber

Retrieves block header by given block number, optionally alongside a Merkle path and the current chain length to validate its inclusion.

**Parameters**

- `block_num`: `uint32` _(optional)_ – the block number of the target block. If not provided, the latest known block will be returned.

**Returns:**

- `block_header`: `BlockHeader` – block header.

### GetBlockByNumber

Retrieves block data by given block number.

**Parameters**

- `block_num`: `uint32` – the block number of the target block.

**Returns:**

- `block`: `Block` – block data encoded in Miden native format.

### GetNotesById

Returns a list of notes matching the provided note IDs.

**Parameters**

- `note_ids`: `[NoteId]` - list of IDs of the notes we want to query.

**Returns**

- `notes`: `[Note]` - List of notes matching the list of requested NoteIds.

### GetAccountDetails

Returns the latest state of an account with the specified ID.

**Parameters**

- `account_id`: `AccountId` – account ID.

**Returns**

- `account`: `AccountInfo` – latest state of the account. For public accounts, this will include full details describing the current account state. For private accounts, only the hash of the latest state and the time of the last update is returned.

### SyncState

Returns info which can be used by the client to sync up to the latest state of the chain
for the objects (accounts, notes, nullifiers) the client is interested in.

This request returns the next block containing requested data. It also returns `chain_tip` which is the latest block number in the chain.
Client is expected to repeat these requests in a loop until `response.block_header.block_num == response.chain_tip`, at which point the client is fully synchronized with the chain.

Each request also returns info about new notes, nullifiers etc. created. It also returns Chain MMR delta that can be used to update the state of Chain MMR.
This includes both chain MMR peaks and chain MMR nodes.

For preserving some degree of privacy, note tags and nullifiers filters contain only high part of hashes. Thus, returned data
contains excessive notes and nullifiers, client can make additional filtering of that data on its side.

**Parameters**

- `block_num`: `uint32` – send updates to the client starting at this block.
- `account_ids`: `[AccountId]` – accounts filter.
- `note_tags`: `[uint32]` – note tags filter. Corresponds to the high 16 bits of the real values.
- `nullifiers`: `[uint32]` – nullifiers filter. Corresponds to the high 16 bits of the real values.

**Returns**

- `chain_tip`: `uint32` – number of the latest block in the chain.
- `block_header`: `BlockHeader` – block header of the block with the first note matching the specified criteria.
- `mmr_delta`: `MmrDelta` – data needed to update the partial MMR from `request.block_num + 1` to `response.block_header.block_num`.
- `accounts`: `[AccountSummary]` – account summaries for accounts updated after `request.block_num + 1` but not after `response.block_header.block_num`.
- `transactions`: `[TransactionSummary]` – transaction summaries for transactions included after `request.block_num + 1` but not after `response.block_header.block_num`.
    - Each `TransactionSummary` consists of the `transaction_id` the transaction identifier, `account_id` of the account that executed that transaction, `block_num` the block number in which the transaction was included.
- `notes`: `[NoteSyncRecord]` – a list of all notes together with the Merkle paths from `response.block_header.note_root`.
- `nullifiers`: `[NullifierUpdate]` – a list of nullifiers created between `request.block_num + 1` and `response.block_header.block_num`.
    - Each `NullifierUpdate` consists of the `nullifier` and `block_num` the block number in which the note corresponding to that nullifier was consumed.

### SubmitProvenTransaction

Submits proven transaction to the Miden network.

**Parameters**

- `transaction`: `bytes` - transaction encoded using Miden's native format.

**Returns**

This method doesn't return any data.

## License

This project is [MIT licensed](../../LICENSE).
