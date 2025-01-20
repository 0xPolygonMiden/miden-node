# Miden node RPC

Contains the code defining the [Miden node's RPC component](/README.md#architecture). This component serves the
user-facing [gRPC](https://grpc.io) methods used to submit transactions and sync with the state of the network.

This is the **only** set of node RPC methods intended to be publicly available.

For more information on the installation and operation of this component, please see the [node's readme](/README.md).

## API overview

The full gRPC method definitions can be found in the [rpc-proto](../rpc-proto/README.md) crate.

<!--toc:start-->
- [CheckNullifiers](#checknullifiers)
- [CheckNullifiersByPrefix](#checknullifiersbyprefix)
- [GetAccountDetails](#getaccountdetails)
- [GetAccountProofs](#getaccountproofs)
- [GetAccountStateDelta](#getaccountstatedelta)
- [GetBlockByNumber](#getblockbynumber)
- [GetBlockHeaderByNumber](#getblockheaderbynumber)
- [GetNotesById](#getnotesbyid)
- [SubmitProvenTransaction](#submitproventransaction)
- [SyncNotes](#syncnotes)
- [SyncState](#syncstate)
<!--toc:end-->

---

### CheckNullifiers

Gets a list of proofs for given nullifier hashes, each proof as a sparse Merkle Tree.

---

### CheckNullifiersByPrefix

Returns a list of nullifiers that match the specified prefixes and are recorded in the node.

---

### GetAccountDetails

Returns the latest state of an account with the specified ID.

---

### GetAccountProofs

Returns the latest state proofs of the specified accounts.

---

### GetAccountStateDelta

Returns delta of the account states in the range from `from_block_num` (exclusive) to `to_block_num` (inclusive).

---

### GetBlockByNumber

Retrieves block data by given block number.

---

### GetBlockHeaderByNumber

Retrieves block header by given block number. Optionally, it also returns the MMR path and current chain length to
authenticate the block's inclusion.

---

### GetNotesById

Returns a list of notes matching the provided note IDs.

---

### SubmitProvenTransaction

Submits proven transaction to the Miden network.

---

### SyncNotes

Note synchronization request.

Specifies note tags that client is interested in. The server will return the first block which contains a note matching
`note_tags` or the chain tip.

---

### SyncState

Returns info which can be used by the client to sync up to the latest state of the chain for the objects (accounts,
notes, nullifiers) the client is interested in.

This request returns the next block containing requested data. It also returns `chain_tip` which is the latest block
number in the chain. Client is expected to repeat these requests in a loop until
`response.block_header.block_num == response.chain_tip`, at which point the client is fully synchronized with the chain.

Each request also returns info about new notes, nullifiers etc. created. It also returns Chain MMR delta that can be
used to update the state of Chain MMR. This includes both chain MMR peaks and chain MMR nodes.

For preserving some degree of privacy, note tags and nullifiers filters contain only high part of hashes. Thus, returned
data contains excessive notes and nullifiers, client can make additional filtering of that data on its side.

---

## License

This project is [MIT licensed](../../LICENSE).
