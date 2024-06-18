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
Here is a brief description of supported methods.

### ApplyBlock

Applies changes of a new block to the DB and in-memory data structures.

**Parameters**

- `block`: `BlockHeader` – block header ([src](../proto/proto/block_header.proto)).
- `accounts`: `[AccountUpdate]` – a list of account updates.
- `nullifiers`: `[Digest]` – a list of nullifier hashes.
- `notes`: `[NoteCreated]` – a list of notes created.

**Returns**

This method doesn't return any data.

### CheckNullifiers

Get a list of proofs for given nullifier hashes, each proof as a sparse Merkle Tree

**Parameters:**

- `nullifiers`: `[Digest]` – array of nullifier hashes.

**Returns:**

- `proofs`: `[NullifierProof]` – array of nullifier proofs, positions correspond to the ones in request.

### GetBlockHeaderByNumber

Retrieves block header by given block number. Optionally, it also returns the MMR path and current chain length to authenticate the block's inclusion.

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

### GetBlockInputs

Returns data needed by the block producer to construct and prove the next block.

**Parameters**

- `account_ids`: `[AccountId]` – array of account IDs.
- `nullifiers`: `[Digest]` – array of nullifier hashes (not currently in use).

**Returns**

- `block_header`: `[BlockHeader]` – the latest block header.
- `mmr_peaks`: `[Digest]` – peaks of the above block's mmr, The `forest` value is equal to the block number.
- `account_states`: `[AccountBlockInputRecord]` – the hashes of the requested accounts and their authentication paths.
- `nullifiers`: `[NullifierBlockInputRecord]` – the requested nullifiers and their authentication paths.

### GetTransactionInputs

Returns the data needed by the block producer to check validity of an incoming transaction.

**Parameters**

- `account_id`: `AccountId` – ID of the account against which a transaction is executed.
- `nullifiers`: `[Digest]` – array of nullifiers for all notes consumed by a transaction.

**Returns**

- `account_state`: `AccountTransactionInputRecord` – account's descriptors.
- `nullifiers`: `[NullifierTransactionInputRecord]` – the block numbers at which corresponding nullifiers have been consumed, zero if not consumed.

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

## Methods for testing purposes

### ListNullifiers

Lists all nullifiers of the current chain.

**Parameters**

This request doesn't have any parameters.

**Returns**

- `nullifiers`: `[NullifierLeaf]` – lists of all nullifiers of the current chain.

### ListAccounts

Lists all accounts of the current chain.

**Parameters**

This request doesn't have any parameters.

**Returns**

- `accounts`: `[AccountInfo]` – list of all accounts of the current chain.

### ListNotes

Lists all notes of the current chain.

**Parameters**

This request doesn't have any parameters.

**Returns**

- `notes`: `[Note]` – list of all notes of the current chain.

## License

This project is [MIT licensed](../../LICENSE).
