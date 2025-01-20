# Miden node store

Contains the code defining the [Miden node's store component](/README.md#architecture). This component stores the
network's state and acts as the networks source of truth. It serves a [gRPC](https://grpc.io) API which allow the other
node components to interact with the store. This API is **internal** only and is considered trusted i.e. the node
operator must take care that the store's API endpoint is **only** exposed to the other node components.

For more information on the installation and operation of this component, please see the [node's readme](/README.md).

## API overview

The full gRPC API can be found [here](../../proto/store.proto).

<!--toc:start-->
- [ApplyBlock](#applyblock)
- [CheckNullifiers](#checknullifiers)
- [CheckNullifiersByPrefix](#checknullifiersbyprefix)
- [GetAccountDetails](#getaccountdetails)
- [GetAccountProofs](#getaccountproofs)
- [GetAccountStateDelta](#getaccountstatedelta)
- [GetBlockByNumber](#getblockbynumber)
- [GetBlockHeaderByNumber](#getblockheaderbynumber)
- [GetBlockInputs](#getblockinputs)
- [GetNoteAuthenticationInfo](#getnoteauthenticationinfo)
- [GetNotesById](#getnotesbyid)
- [GetTransactionInputs](#gettransactioninputs)
- [SyncNotes](#syncnotes)
- [SyncState](#syncstate)
<!--toc:end-->

---

### ApplyBlock

Applies changes of a new block to the DB and in-memory data structures.

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

### GetBlockInputs

Returns data required to prove the next block.

---

### GetNoteAuthenticationInfo

Returns a list of Note inclusion proofs for the specified Note IDs.

---

### GetNotesById

Returns a list of notes matching the provided note IDs.

---

### GetTransactionInputs

Returns data required to validate a new transaction.

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
