# API Reference

# Table of Contents
- [Endpoints](#endpoints)
  - [`block_producer` methods](#block_producer-methods)
  - [`rpc` methods](#rpc-methods)
  - [`store` methods](#store-methods)

- [Messages](#messages)
  - [account.proto](#account.proto)
    - [AccountHeader](#account-accountheader)
    - [AccountId](#account-accountid)
    - [AccountInfo](#account-accountinfo)
    - [AccountSummary](#account-accountsummary)
  - [block.proto](#block.proto)
    - [BlockHeader](#block-blockheader)
    - [BlockInclusionProof](#block-blockinclusionproof)
  - [digest.proto](#digest.proto)
    - [Digest](#digest-digest)
  - [merkle.proto](#merkle.proto)
    - [MerklePath](#merkle-merklepath)
  - [mmr.proto](#mmr.proto)
    - [MmrDelta](#mmr-mmrdelta)
  - [note.proto](#note.proto)
    - [Note](#note-note)
    - [NoteAuthenticationInfo](#note-noteauthenticationinfo)
    - [NoteInclusionInBlockProof](#note-noteinclusioninblockproof)
    - [NoteMetadata](#note-notemetadata)
    - [NoteSyncRecord](#note-notesyncrecord)
  - [requests.proto](#requests.proto)
    - [ApplyBlockRequest](#requests-applyblockrequest)
    - [CheckNullifiersByPrefixRequest](#requests-checknullifiersbyprefixrequest)
    - [CheckNullifiersRequest](#requests-checknullifiersrequest)
    - [GetAccountDetailsRequest](#requests-getaccountdetailsrequest)
    - [GetAccountProofsRequest](#requests-getaccountproofsrequest)
    - [GetAccountStateDeltaRequest](#requests-getaccountstatedeltarequest)
    - [GetBlockByNumberRequest](#requests-getblockbynumberrequest)
    - [GetBlockHeaderByNumberRequest](#requests-getblockheaderbynumberrequest)
    - [GetBlockInputsRequest](#requests-getblockinputsrequest)
    - [GetNoteAuthenticationInfoRequest](#requests-getnoteauthenticationinforequest)
    - [GetNotesByIdRequest](#requests-getnotesbyidrequest)
    - [GetTransactionInputsRequest](#requests-gettransactioninputsrequest)
    - [ListAccountsRequest](#requests-listaccountsrequest)
    - [ListNotesRequest](#requests-listnotesrequest)
    - [ListNullifiersRequest](#requests-listnullifiersrequest)
    - [SubmitProvenTransactionRequest](#requests-submitproventransactionrequest)
    - [SyncNoteRequest](#requests-syncnoterequest)
    - [SyncStateRequest](#requests-syncstaterequest)
  - [responses.proto](#responses.proto)
    - [AccountBlockInputRecord](#responses-accountblockinputrecord)
    - [AccountProofsResponse](#responses-accountproofsresponse)
    - [AccountStateHeader](#responses-accountstateheader)
    - [AccountTransactionInputRecord](#responses-accounttransactioninputrecord)
    - [ApplyBlockResponse](#responses-applyblockresponse)
    - [CheckNullifiersByPrefixResponse](#responses-checknullifiersbyprefixresponse)
    - [CheckNullifiersResponse](#responses-checknullifiersresponse)
    - [GetAccountDetailsResponse](#responses-getaccountdetailsresponse)
    - [GetAccountProofsResponse](#responses-getaccountproofsresponse)
    - [GetAccountStateDeltaResponse](#responses-getaccountstatedeltaresponse)
    - [GetBlockByNumberResponse](#responses-getblockbynumberresponse)
    - [GetBlockHeaderByNumberResponse](#responses-getblockheaderbynumberresponse)
    - [GetBlockInputsResponse](#responses-getblockinputsresponse)
    - [GetNoteAuthenticationInfoResponse](#responses-getnoteauthenticationinforesponse)
    - [GetNotesByIdResponse](#responses-getnotesbyidresponse)
    - [GetTransactionInputsResponse](#responses-gettransactioninputsresponse)
    - [ListAccountsResponse](#responses-listaccountsresponse)
    - [ListNotesResponse](#responses-listnotesresponse)
    - [ListNullifiersResponse](#responses-listnullifiersresponse)
    - [NullifierBlockInputRecord](#responses-nullifierblockinputrecord)
    - [NullifierTransactionInputRecord](#responses-nullifiertransactioninputrecord)
    - [NullifierUpdate](#responses-nullifierupdate)
    - [SubmitProvenTransactionResponse](#responses-submitproventransactionresponse)
    - [SyncNoteResponse](#responses-syncnoteresponse)
    - [SyncStateResponse](#responses-syncstateresponse)
  - [smt.proto](#smt.proto)
    - [SmtLeaf](#smt-smtleaf)
    - [SmtLeafEntries](#smt-smtleafentries)
    - [SmtLeafEntry](#smt-smtleafentry)
    - [SmtOpening](#smt-smtopening)
  - [transaction.proto](#transaction.proto)
    - [TransactionId](#transaction-transactionid)
    - [TransactionSummary](#transaction-transactionsummary)

- [Scalar Value Types](#scalar-value-types)

# Endpoints

## `block_producer` methods

### SubmitProvenTransaction
Submits proven transaction to the Miden network
> **rpc** SubmitProvenTransaction([SubmitProvenTransactionRequest](#.requests.SubmitProvenTransactionRequest)) returns [SubmitProvenTransactionResponse](#.responses.SubmitProvenTransactionResponse)

## `rpc` methods

### CheckNullifiers
Gets a list of proofs for given nullifier hashes, each proof as a sparse Merkle Tree.
> **rpc** CheckNullifiers([CheckNullifiersRequest](#.requests.CheckNullifiersRequest)) returns [CheckNullifiersResponse](#.responses.CheckNullifiersResponse)

### CheckNullifiersByPrefix
Returns a list of nullifiers that match the specified prefixes and are recorded in the node.
> **rpc** CheckNullifiersByPrefix([CheckNullifiersByPrefixRequest](#.requests.CheckNullifiersByPrefixRequest)) returns [CheckNullifiersByPrefixResponse](#.responses.CheckNullifiersByPrefixResponse)

### GetAccountDetails
Returns the latest state of an account with the specified ID.
> **rpc** GetAccountDetails([GetAccountDetailsRequest](#.requests.GetAccountDetailsRequest)) returns [GetAccountDetailsResponse](#.responses.GetAccountDetailsResponse)

### GetAccountProofs
Returns the latest state proofs of accounts with the specified IDs.
> **rpc** GetAccountProofs([GetAccountProofsRequest](#.requests.GetAccountProofsRequest)) returns [GetAccountProofsResponse](#.responses.GetAccountProofsResponse)

### GetAccountStateDelta
Returns delta of the account states in the range from `from_block_num` (exclusive) to
`to_block_num` (inclusive).
> **rpc** GetAccountStateDelta([GetAccountStateDeltaRequest](#.requests.GetAccountStateDeltaRequest)) returns [GetAccountStateDeltaResponse](#.responses.GetAccountStateDeltaResponse)

### GetBlockByNumber
Retrieves block data by given block number.
> **rpc** GetBlockByNumber([GetBlockByNumberRequest](#.requests.GetBlockByNumberRequest)) returns [GetBlockByNumberResponse](#.responses.GetBlockByNumberResponse)

### GetBlockHeaderByNumber
Retrieves block header by given block number. Optionally, it also returns the MMR path
and current chain length to authenticate the block's inclusion.
> **rpc** GetBlockHeaderByNumber([GetBlockHeaderByNumberRequest](#.requests.GetBlockHeaderByNumberRequest)) returns [GetBlockHeaderByNumberResponse](#.responses.GetBlockHeaderByNumberResponse)

### GetNotesById
Returns a list of notes matching the provided note IDs.
> **rpc** GetNotesById([GetNotesByIdRequest](#.requests.GetNotesByIdRequest)) returns [GetNotesByIdResponse](#.responses.GetNotesByIdResponse)

### SubmitProvenTransaction
Submits proven transaction to the Miden network.
> **rpc** SubmitProvenTransaction([SubmitProvenTransactionRequest](#.requests.SubmitProvenTransactionRequest)) returns [SubmitProvenTransactionResponse](#.responses.SubmitProvenTransactionResponse)

### SyncNotes
Note synchronization request.

Specifies note tags that client is interested in. The server will return the first block which
contains a note matching `note_tags` or the chain tip.
> **rpc** SyncNotes([SyncNoteRequest](#.requests.SyncNoteRequest)) returns [SyncNoteResponse](#.responses.SyncNoteResponse)

### SyncState
Returns info which can be used by the client to sync up to the latest state of the chain
for the objects (accounts, notes, nullifiers) the client is interested in.

This request returns the next block containing requested data. It also returns `chain_tip`
which is the latest block number in the chain. Client is expected to repeat these requests
in a loop until `response.block_header.block_num == response.chain_tip`, at which point
the client is fully synchronized with the chain.

Each request also returns info about new notes, nullifiers etc. created. It also returns
Chain MMR delta that can be used to update the state of Chain MMR. This includes both chain
MMR peaks and chain MMR nodes.

For preserving some degree of privacy, note tags and nullifiers filters contain only high
part of hashes. Thus, returned data contains excessive notes and nullifiers, client can make
additional filtering of that data on its side.
> **rpc** SyncState([SyncStateRequest](#.requests.SyncStateRequest)) returns [SyncStateResponse](#.responses.SyncStateResponse)

## `store` methods

### ApplyBlock
Applies changes of a new block to the DB and in-memory data structures.
> **rpc** ApplyBlock([ApplyBlockRequest](#.requests.ApplyBlockRequest)) returns [ApplyBlockResponse](#.responses.ApplyBlockResponse)

### CheckNullifiers
Gets a list of proofs for given nullifier hashes, each proof as a sparse Merkle Tree.
> **rpc** CheckNullifiers([CheckNullifiersRequest](#.requests.CheckNullifiersRequest)) returns [CheckNullifiersResponse](#.responses.CheckNullifiersResponse)

### CheckNullifiersByPrefix
Returns a list of nullifiers that match the specified prefixes and are recorded in the node.
> **rpc** CheckNullifiersByPrefix([CheckNullifiersByPrefixRequest](#.requests.CheckNullifiersByPrefixRequest)) returns [CheckNullifiersByPrefixResponse](#.responses.CheckNullifiersByPrefixResponse)

### GetAccountDetails
Returns the latest state of an account with the specified ID.
> **rpc** GetAccountDetails([GetAccountDetailsRequest](#.requests.GetAccountDetailsRequest)) returns [GetAccountDetailsResponse](#.responses.GetAccountDetailsResponse)

### GetAccountProofs
Returns the latest state proofs of accounts with the specified IDs.
> **rpc** GetAccountProofs([GetAccountProofsRequest](#.requests.GetAccountProofsRequest)) returns [GetAccountProofsResponse](#.responses.GetAccountProofsResponse)

### GetAccountStateDelta
Returns delta of the account states in the range from `from_block_num` (exclusive) to
`to_block_num` (inclusive).
> **rpc** GetAccountStateDelta([GetAccountStateDeltaRequest](#.requests.GetAccountStateDeltaRequest)) returns [GetAccountStateDeltaResponse](#.responses.GetAccountStateDeltaResponse)

### GetBlockByNumber
Retrieves block data by given block number.
> **rpc** GetBlockByNumber([GetBlockByNumberRequest](#.requests.GetBlockByNumberRequest)) returns [GetBlockByNumberResponse](#.responses.GetBlockByNumberResponse)

### GetBlockHeaderByNumber
Retrieves block header by given block number. Optionally, it also returns the MMR path
and current chain length to authenticate the block's inclusion.
> **rpc** GetBlockHeaderByNumber([GetBlockHeaderByNumberRequest](#.requests.GetBlockHeaderByNumberRequest)) returns [GetBlockHeaderByNumberResponse](#.responses.GetBlockHeaderByNumberResponse)

### GetBlockInputs
Returns data needed by the block producer to construct and prove the next block, including
account states, nullifiers, and unauthenticated notes.
> **rpc** GetBlockInputs([GetBlockInputsRequest](#.requests.GetBlockInputsRequest)) returns [GetBlockInputsResponse](#.responses.GetBlockInputsResponse)

### GetNoteAuthenticationInfo
Returns a list of Note inclusion proofs for the specified Note IDs.
> **rpc** GetNoteAuthenticationInfo([GetNoteAuthenticationInfoRequest](#.requests.GetNoteAuthenticationInfoRequest)) returns [GetNoteAuthenticationInfoResponse](#.responses.GetNoteAuthenticationInfoResponse)

### GetNotesById
Returns a list of notes matching the provided note IDs.
> **rpc** GetNotesById([GetNotesByIdRequest](#.requests.GetNotesByIdRequest)) returns [GetNotesByIdResponse](#.responses.GetNotesByIdResponse)

### GetTransactionInputs
Returns the data needed by the block producer to check validity of an incoming transaction.
> **rpc** GetTransactionInputs([GetTransactionInputsRequest](#.requests.GetTransactionInputsRequest)) returns [GetTransactionInputsResponse](#.responses.GetTransactionInputsResponse)

### ListAccounts
Lists all accounts of the current chain.
> **rpc** ListAccounts([ListAccountsRequest](#.requests.ListAccountsRequest)) returns [ListAccountsResponse](#.responses.ListAccountsResponse)

### ListNotes
Lists all notes of the current chain.
> **rpc** ListNotes([ListNotesRequest](#.requests.ListNotesRequest)) returns [ListNotesResponse](#.responses.ListNotesResponse)

### ListNullifiers
Lists all nullifiers of the current chain.
> **rpc** ListNullifiers([ListNullifiersRequest](#.requests.ListNullifiersRequest)) returns [ListNullifiersResponse](#.responses.ListNullifiersResponse)

### SyncNotes
Note synchronization request.

Specifies note tags that client is interested in. The server will return the first block which
contains a note matching `note_tags` or the chain tip.
> **rpc** SyncNotes([SyncNoteRequest](#.requests.SyncNoteRequest)) returns [SyncNoteResponse](#.responses.SyncNoteResponse)

### SyncState
Returns info which can be used by the client to sync up to the latest state of the chain
for the objects (accounts, notes, nullifiers) the client is interested in.

This request returns the next block containing requested data. It also returns `chain_tip`
which is the latest block number in the chain. Client is expected to repeat these requests
in a loop until `response.block_header.block_num == response.chain_tip`, at which point
the client is fully synchronized with the chain.

Each request also returns info about new notes, nullifiers etc. created. It also returns
Chain MMR delta that can be used to update the state of Chain MMR. This includes both chain
MMR peaks and chain MMR nodes.

For preserving some degree of privacy, note tags and nullifiers filters contain only high
part of hashes. Thus, returned data contains excessive notes and nullifiers, client can make
additional filtering of that data on its side.
> **rpc** SyncState([SyncStateRequest](#.requests.SyncStateRequest)) returns [SyncStateResponse](#.responses.SyncStateResponse)


# Messages

## AccountHeader {#account-accountheader}
An account header.

### Fields
- `vault_root`: [`digest.Digest`](#digest-digest) — Vault root hash.
- `storage_commitment`: [`digest.Digest`](#digest-digest) — Storage root hash.
- `code_commitment`: [`digest.Digest`](#digest-digest) — Code root hash.
- `nonce`: [`uint64`](#uint64) — Account nonce.


## AccountId {#account-accountid}
An account ID.

### Fields
- `id`: [`fixed64`](#fixed64) — A miden account is defined with a little bit of proof-of-work, the id itself is defined as the first word of a hash digest. For this reason account ids can be considered as random values, because of that the encoding below uses fixed 64 bits, instead of zig-zag encoding.


## AccountInfo {#account-accountinfo}
An account info.

### Fields
- `summary`: [`AccountSummary`](#accountsummary) — Account summary.
- `details`: _optional_ [`bytes`](#bytes) — Account details encoded using Miden native format.


## AccountSummary {#account-accountsummary}
A summary of an account.

### Fields
- `account_id`: [`AccountId`](#accountid) — The account ID.
- `account_hash`: [`digest.Digest`](#digest-digest) — The latest account hash, zero hash if the account doesn't exist.
- `block_num`: [`uint32`](#uint32) — Merkle path to verify the account's inclusion in the MMR.


## BlockHeader {#block-blockheader}
Represents a block header.

### Fields
- `version`: [`uint32`](#uint32) — Specifies the version of the protocol.
- `prev_hash`: [`digest.Digest`](#digest-digest) — The hash of the previous blocks header.
- `block_num`: [`fixed32`](#fixed32) — A unique sequential number of the current block.
- `chain_root`: [`digest.Digest`](#digest-digest) — A commitment to an MMR of the entire chain where each block is a leaf.
- `account_root`: [`digest.Digest`](#digest-digest) — A commitment to account database.
- `nullifier_root`: [`digest.Digest`](#digest-digest) — A commitment to the nullifier database.
- `note_root`: [`digest.Digest`](#digest-digest) — A commitment to all notes created in the current block.
- `tx_hash`: [`digest.Digest`](#digest-digest) — A commitment to a set of IDs of transactions which affected accounts in this block.
- `proof_hash`: [`digest.Digest`](#digest-digest) — A hash of a STARK proof attesting to the correct state transition.
- `kernel_root`: [`digest.Digest`](#digest-digest) — A commitment to all transaction kernels supported by this block.
- `timestamp`: [`fixed32`](#fixed32) — The time when the block was created.


## BlockInclusionProof {#block-blockinclusionproof}
Represents a block inclusion proof.

### Fields
- `block_header`: [`BlockHeader`](#blockheader) — Block header associated with the inclusion proof.
- `mmr_path`: [`merkle.MerklePath`](#merkle-merklepath) — Merkle path associated with the inclusion proof.
- `chain_length`: [`fixed32`](#fixed32) — The chain length associated with `mmr_path`.


## Digest {#digest-digest}
A hash digest, the result of a hash function.

### Fields
- `d0`: [`fixed64`](#fixed64) — none
- `d1`: [`fixed64`](#fixed64) — none
- `d2`: [`fixed64`](#fixed64) — none
- `d3`: [`fixed64`](#fixed64) — none


## MerklePath {#merkle-merklepath}
Represents a Merkle path.

### Fields
- `siblings`: _repeated_ [`digest.Digest`](#digest-digest) — List of sibling node hashes, in order from the root to the leaf.


## MmrDelta {#mmr-mmrdelta}
Represents an MMR delta.

### Fields
- `forest`: [`uint64`](#uint64) — The number of trees in the forest (latest block number + 1).
- `data`: _repeated_ [`digest.Digest`](#digest-digest) — New and changed MMR peaks.


## Note {#note-note}
Represents a note.

### Fields
- `block_num`: [`fixed32`](#fixed32) — The block number in which the note was created.
- `note_index`: [`uint32`](#uint32) — The index of the note in the block.
- `note_id`: [`digest.Digest`](#digest-digest) — The ID of the note.
- `metadata`: [`NoteMetadata`](#notemetadata) — The note metadata.
- `merkle_path`: [`merkle.MerklePath`](#merkle-merklepath) — The note inclusion proof in the block.
- `details`: _optional_ [`bytes`](#bytes) — This field will be present when the note is public. details contain the `Note` in a serialized format.


## NoteAuthenticationInfo {#note-noteauthenticationinfo}
Represents proof of notes inclusion in the block(s) and block(s) inclusion in the chain.

### Fields
- `note_proofs`: _repeated_ [`NoteInclusionInBlockProof`](#noteinclusioninblockproof) — Proof of each note's inclusion in a block.
- `block_proofs`: _repeated_ [`block.BlockInclusionProof`](#block-blockinclusionproof) — Proof of each block's inclusion in the chain.


## NoteInclusionInBlockProof {#note-noteinclusioninblockproof}
Represents proof of a note's inclusion in a block.

### Fields
- `note_id`: [`digest.Digest`](#digest-digest) — The ID of the note.
- `block_num`: [`fixed32`](#fixed32) — The block number in which the note was created.
- `note_index_in_block`: [`uint32`](#uint32) — The index of the note in the block.
- `merkle_path`: [`merkle.MerklePath`](#merkle-merklepath) — The note inclusion proof in the block.


## NoteMetadata {#note-notemetadata}
Represents a note metadata.

### Fields
- `sender`: [`account.AccountId`](#account-accountid) — The sender of the note.
- `note_type`: [`uint32`](#uint32) — The type of the note (0b01 = public, 0b10 = private, 0b11 = encrypted).
- `tag`: [`fixed32`](#fixed32) — A value which can be used by the recipient(s) to identify notes intended for them.
- `execution_hint`: [`fixed64`](#fixed64) — Specifies when a note is ready to be consumed: (6 least significant bits - hint identifier (tag), bits 6 to 38 - Hint payload). See `miden_objects::notes::execution_hint` for more info.
- `aux`: [`fixed64`](#fixed64) — An arbitrary user-defined value.


## NoteSyncRecord {#note-notesyncrecord}
Represents proof of a note inclusion in the block.

### Fields
- `note_index`: [`uint32`](#uint32) — The index of the note.
- `note_id`: [`digest.Digest`](#digest-digest) — The ID of the note.
- `metadata`: [`NoteMetadata`](#notemetadata) — The note metadata.
- `merkle_path`: [`merkle.MerklePath`](#merkle-merklepath) — The note inclusion proof in the block.


## ApplyBlockRequest {#requests-applyblockrequest}
Applies changes of a new block to the DB and in-memory data structures.

### Fields
- `block`: [`bytes`](#bytes) — Block data encoded using Miden's native format.


## CheckNullifiersByPrefixRequest {#requests-checknullifiersbyprefixrequest}
Returns a list of nullifiers that match the specified prefixes and are recorded in the node.

### Fields
- `prefix_len`: [`uint32`](#uint32) — Number of bits used for nullifier prefix. Currently the only supported value is 16.
- `nullifiers`: _repeated_ [`uint32`](#uint32) — List of nullifiers to check. Each nullifier is specified by its prefix with length equal to `prefix_len`.


## CheckNullifiersRequest {#requests-checknullifiersrequest}
Get a list of proofs for given nullifier hashes, each proof as a sparse Merkle Tree.

### Fields
- `nullifiers`: _repeated_ [`digest.Digest`](#digest-digest) — List of nullifiers to return proofs for.


## GetAccountDetailsRequest {#requests-getaccountdetailsrequest}
Returns the latest state of an account with the specified ID.

### Fields
- `account_id`: [`account.AccountId`](#account-accountid) — Account ID to get details.


## GetAccountProofsRequest {#requests-getaccountproofsrequest}
Returns the latest state proofs of accounts with the specified IDs.

### Fields
- `account_ids`: _repeated_ [`account.AccountId`](#account-accountid) — List of account IDs to get states.
- `include_headers`: _optional_ [`bool`](#bool) — Optional flag to include header and account code in the response. `false` by default.
- `code_commitments`: _repeated_ [`digest.Digest`](#digest-digest) — Account code commitments corresponding to the last-known `AccountCode` for requested accounts. Responses will include only the ones that are not known to the caller. These are not associated with a specific account but rather, they will be matched against all requested accounts.


## GetAccountStateDeltaRequest {#requests-getaccountstatedeltarequest}
Returns delta of the account states in the range from `from_block_num` (exclusive) to
`to_block_num` (inclusive).

### Fields
- `account_id`: [`account.AccountId`](#account-accountid) — ID of the account for which the delta is requested.
- `from_block_num`: [`fixed32`](#fixed32) — Block number from which the delta is requested (exclusive).
- `to_block_num`: [`fixed32`](#fixed32) — Block number up to which the delta is requested (inclusive).


## GetBlockByNumberRequest {#requests-getblockbynumberrequest}
Retrieves block data by given block number.

### Fields
- `block_num`: [`fixed32`](#fixed32) — The block number of the target block.


## GetBlockHeaderByNumberRequest {#requests-getblockheaderbynumberrequest}
Returns the block header corresponding to the requested block number, as well as the merkle
path and current forest which validate the block's inclusion in the chain.

The Merkle path is an MMR proof for the block's leaf, based on the current chain length.

### Fields
- `block_num`: _optional_ [`uint32`](#uint32) — The block number of the target block. If not provided, means latest known block.
- `include_mmr_proof`: _optional_ [`bool`](#bool) — Whether or not to return authentication data for the block header.


## GetBlockInputsRequest {#requests-getblockinputsrequest}
Returns data needed by the block producer to construct and prove the next block, including
account states, nullifiers, and unauthenticated notes.

### Fields
- `account_ids`: _repeated_ [`account.AccountId`](#account-accountid) — ID of the account against which a transaction is executed.
- `nullifiers`: _repeated_ [`digest.Digest`](#digest-digest) — Array of nullifiers for all notes consumed by a transaction.
- `unauthenticated_notes`: _repeated_ [`digest.Digest`](#digest-digest) — Array of note IDs to be checked for existence in the database.


## GetNoteAuthenticationInfoRequest {#requests-getnoteauthenticationinforequest}
Returns a list of Note inclusion proofs for the specified Note IDs.

### Fields
- `note_ids`: _repeated_ [`digest.Digest`](#digest-digest) — List of NoteId's to be queried from the database.


## GetNotesByIdRequest {#requests-getnotesbyidrequest}
Returns a list of notes matching the provided note IDs.

### Fields
- `note_ids`: _repeated_ [`digest.Digest`](#digest-digest) — List of NoteId's to be queried from the database.


## GetTransactionInputsRequest {#requests-gettransactioninputsrequest}
Returns the data needed by the block producer to check validity of an incoming transaction.

### Fields
- `account_id`: [`account.AccountId`](#account-accountid) — ID of the account against which a transaction is executed.
- `nullifiers`: _repeated_ [`digest.Digest`](#digest-digest) — Array of nullifiers for all notes consumed by a transaction.
- `unauthenticated_notes`: _repeated_ [`digest.Digest`](#digest-digest) — Array of unauthenticated note IDs to be checked for existence in the database.


## ListAccountsRequest {#requests-listaccountsrequest}
Lists all accounts of the current chain.

### Fields
No fields

## ListNotesRequest {#requests-listnotesrequest}
Lists all notes of the current chain.

### Fields
No fields

## ListNullifiersRequest {#requests-listnullifiersrequest}
Lists all nullifiers of the current chain.

### Fields
No fields

## SubmitProvenTransactionRequest {#requests-submitproventransactionrequest}
Submits proven transaction to the Miden network.

### Fields
- `transaction`: [`bytes`](#bytes) — Transaction encoded using Miden's native format.


## SyncNoteRequest {#requests-syncnoterequest}
Note synchronization request.

Specifies note tags that client is interested in. The server will return the first block which
contains a note matching `note_tags` or the chain tip.

### Fields
- `block_num`: [`fixed32`](#fixed32) — Last block known by the client. The response will contain data starting from the next block, until the first block which contains a note of matching the requested tag.
- `note_tags`: _repeated_ [`fixed32`](#fixed32) — Specifies the tags which the client is interested in.


## SyncStateRequest {#requests-syncstaterequest}
State synchronization request.

Specifies state updates the client is interested in. The server will return the first block which
contains a note matching `note_tags` or the chain tip. And the corresponding updates to
`nullifiers` and `account_ids` for that block range.

### Fields
- `block_num`: [`fixed32`](#fixed32) — Last block known by the client. The response will contain data starting from the next block, until the first block which contains a note of matching the requested tag, or the chain tip if there are no notes.
- `account_ids`: _repeated_ [`account.AccountId`](#account-accountid) — Accounts' hash to include in the response. An account hash will be included if-and-only-if it is the latest update. Meaning it is possible there was an update to the account for the given range, but if it is not the latest, it won't be included in the response.
- `note_tags`: _repeated_ [`fixed32`](#fixed32) — Specifies the tags which the client is interested in.
- `nullifiers`: _repeated_ [`uint32`](#uint32) — Determines the nullifiers the client is interested in by specifying the 16high bits of the target nullifier.


## AccountBlockInputRecord {#responses-accountblockinputrecord}
An account returned as a response to the `GetBlockInputs`.

### Fields
- `account_id`: [`account.AccountId`](#account-accountid) — The account ID.
- `account_hash`: [`digest.Digest`](#digest-digest) — The latest account hash, zero hash if the account doesn't exist.
- `proof`: [`merkle.MerklePath`](#merkle-merklepath) — Merkle path to verify the account's inclusion in the MMR.


## AccountProofsResponse {#responses-accountproofsresponse}
A single account proof returned as a response to the `GetAccountProofs`.

### Fields
- `account_id`: [`account.AccountId`](#account-accountid) — Account ID.
- `account_hash`: [`digest.Digest`](#digest-digest) — Account hash.
- `account_proof`: [`merkle.MerklePath`](#merkle-merklepath) — Authentication path from the `account_root` of the block header to the account.
- `state_header`: _optional_ [`AccountStateHeader`](#accountstateheader) — State header for public accounts. Filled only if `include_headers` flag is set to `true`.


## AccountStateHeader {#responses-accountstateheader}
State header for public accounts.

### Fields
- `header`: [`account.AccountHeader`](#account-accountheader) — Account header.
- `storage_header`: [`bytes`](#bytes) — Values of all account storage slots (max 255).
- `account_code`: _optional_ [`bytes`](#bytes) — Account code, returned only when none of the request's code commitments match with the current one.


## AccountTransactionInputRecord {#responses-accounttransactioninputrecord}
An account returned as a response to the `GetTransactionInputs`.

### Fields
- `account_id`: [`account.AccountId`](#account-accountid) — The account ID.
- `account_hash`: [`digest.Digest`](#digest-digest) — The latest account hash, zero hash if the account doesn't exist.


## ApplyBlockResponse {#responses-applyblockresponse}
Represents the result of applying a block.

### Fields
No fields

## CheckNullifiersByPrefixResponse {#responses-checknullifiersbyprefixresponse}
Represents the result of checking nullifiers by prefix.

### Fields
- `nullifiers`: _repeated_ [`NullifierUpdate`](#nullifierupdate) — List of nullifiers matching the prefixes specified in the request.


## CheckNullifiersResponse {#responses-checknullifiersresponse}
Represents the result of checking nullifiers.

### Fields
- `proofs`: _repeated_ [`smt.SmtOpening`](#smt-smtopening) — Each requested nullifier has its corresponding nullifier proof at the same position.


## GetAccountDetailsResponse {#responses-getaccountdetailsresponse}
Represents the result of getting account details.

### Fields
- `details`: [`account.AccountInfo`](#account-accountinfo) — Account info (with details for public accounts).


## GetAccountProofsResponse {#responses-getaccountproofsresponse}
Represents the result of getting account proofs.

### Fields
- `block_num`: [`fixed32`](#fixed32) — Block number at which the state of the account was returned.
- `account_proofs`: _repeated_ [`AccountProofsResponse`](#accountproofsresponse) — List of account state infos for the requested account keys.


## GetAccountStateDeltaResponse {#responses-getaccountstatedeltaresponse}
Represents the result of getting account state delta.

### Fields
- `delta`: _optional_ [`bytes`](#bytes) — The calculated `AccountStateDelta` encoded using Miden native format.


## GetBlockByNumberResponse {#responses-getblockbynumberresponse}
Represents the result of getting block by number.

### Fields
- `block`: _optional_ [`bytes`](#bytes) — The requested `Block` data encoded using Miden native format.


## GetBlockHeaderByNumberResponse {#responses-getblockheaderbynumberresponse}
Represents the result of getting a block header by block number.

### Fields
- `block_header`: [`block.BlockHeader`](#block-blockheader) — The requested block header.
- `mmr_path`: _optional_ [`merkle.MerklePath`](#merkle-merklepath) — Merkle path to verify the block's inclusion in the MMR at the returned `chain_length`.
- `chain_length`: _optional_ [`fixed32`](#fixed32) — Current chain length.


## GetBlockInputsResponse {#responses-getblockinputsresponse}
Represents the result of getting block inputs.

### Fields
- `block_header`: [`block.BlockHeader`](#block-blockheader) — The latest block header.
- `mmr_peaks`: _repeated_ [`digest.Digest`](#digest-digest) — Peaks of the above block's mmr, The `forest` value is equal to the block number.
- `account_states`: _repeated_ [`AccountBlockInputRecord`](#accountblockinputrecord) — The hashes of the requested accounts and their authentication paths.
- `nullifiers`: _repeated_ [`NullifierBlockInputRecord`](#nullifierblockinputrecord) — The requested nullifiers and their authentication paths.
- `found_unauthenticated_notes`: [`note.NoteAuthenticationInfo`](#note-noteauthenticationinfo) — The list of requested notes which were found in the database.


## GetNoteAuthenticationInfoResponse {#responses-getnoteauthenticationinforesponse}
Represents the result of getting note authentication info.

### Fields
- `proofs`: [`note.NoteAuthenticationInfo`](#note-noteauthenticationinfo) — Proofs of note inclusions in blocks and block inclusions in chain.


## GetNotesByIdResponse {#responses-getnotesbyidresponse}
Represents the result of getting notes by IDs.

### Fields
- `notes`: _repeated_ [`note.Note`](#note-note) — Lists Note's returned by the database.


## GetTransactionInputsResponse {#responses-gettransactioninputsresponse}
Represents the result of getting transaction inputs.

### Fields
- `account_state`: [`AccountTransactionInputRecord`](#accounttransactioninputrecord) — Account state proof.
- `nullifiers`: _repeated_ [`NullifierTransactionInputRecord`](#nullifiertransactioninputrecord) — List of nullifiers that have been consumed.
- `missing_unauthenticated_notes`: _repeated_ [`digest.Digest`](#digest-digest) — List of unauthenticated notes that were not found in the database.
- `block_height`: [`fixed32`](#fixed32) — The node's current block height.


## ListAccountsResponse {#responses-listaccountsresponse}
Represents the result of getting accounts list.

### Fields
- `accounts`: _repeated_ [`account.AccountInfo`](#account-accountinfo) — Lists all accounts of the current chain.


## ListNotesResponse {#responses-listnotesresponse}
Represents the result of getting notes list.

### Fields
- `notes`: _repeated_ [`note.Note`](#note-note) — Lists all notes of the current chain.


## ListNullifiersResponse {#responses-listnullifiersresponse}
Represents the result of getting nullifiers list.

### Fields
- `nullifiers`: _map_ [`smt.SmtLeafEntry`](#smt-smtleafentry) — Lists all nullifiers of the current chain.


## NullifierBlockInputRecord {#responses-nullifierblockinputrecord}
A nullifier returned as a response to the `GetBlockInputs`.

### Fields
- `nullifier`: [`digest.Digest`](#digest-digest) — The nullifier ID.
- `opening`: [`smt.SmtOpening`](#smt-smtopening) — Merkle path to verify the nullifier's inclusion in the MMR.


## NullifierTransactionInputRecord {#responses-nullifiertransactioninputrecord}
A nullifier returned as a response to the `GetTransactionInputs`.

### Fields
- `nullifier`: [`digest.Digest`](#digest-digest) — The nullifier ID.
- `block_num`: [`fixed32`](#fixed32) — The block at which the nullifier has been consumed, zero if not consumed.


## NullifierUpdate {#responses-nullifierupdate}
Represents a single nullifier update.

### Fields
- `nullifier`: [`digest.Digest`](#digest-digest) — Nullifier ID.
- `block_num`: [`fixed32`](#fixed32) — Block number.


## SubmitProvenTransactionResponse {#responses-submitproventransactionresponse}
Represents the result of submitting proven transaction.

### Fields
- `block_height`: [`fixed32`](#fixed32) — The node's current block height.


## SyncNoteResponse {#responses-syncnoteresponse}
Represents the result of syncing notes request.

### Fields
- `chain_tip`: [`fixed32`](#fixed32) — Number of the latest block in the chain.
- `block_header`: [`block.BlockHeader`](#block-blockheader) — Block header of the block with the first note matching the specified criteria.
- `mmr_path`: [`merkle.MerklePath`](#merkle-merklepath) — Merkle path to verify the block's inclusion in the MMR at the returned `chain_tip`.

An MMR proof can be constructed for the leaf of index `block_header.block_num` of an MMR of forest `chain_tip` with this path.
- `notes`: _repeated_ [`note.NoteSyncRecord`](#note-notesyncrecord) — List of all notes together with the Merkle paths from `response.block_header.note_root`.


## SyncStateResponse {#responses-syncstateresponse}
Represents the result of syncing state request.

### Fields
- `chain_tip`: [`fixed32`](#fixed32) — Number of the latest block in the chain.
- `block_header`: [`block.BlockHeader`](#block-blockheader) — Block header of the block with the first note matching the specified criteria.
- `mmr_delta`: [`mmr.MmrDelta`](#mmr-mmrdelta) — Data needed to update the partial MMR from `request.block_num + 1` to `response.block_header.block_num`.
- `accounts`: _repeated_ [`account.AccountSummary`](#account-accountsummary) — List of account hashes updated after `request.block_num + 1` but not after `response.block_header.block_num`.
- `transactions`: _repeated_ [`transaction.TransactionSummary`](#transaction-transactionsummary) — List of transactions executed against requested accounts between `request.block_num + 1` and `response.block_header.block_num`.
- `notes`: _repeated_ [`note.NoteSyncRecord`](#note-notesyncrecord) — List of all notes together with the Merkle paths from `response.block_header.note_root`.
- `nullifiers`: _repeated_ [`NullifierUpdate`](#nullifierupdate) — List of nullifiers created between `request.block_num + 1` and `response.block_header.block_num`.


## SmtLeaf {#smt-smtleaf}
A leaf in an SMT, sitting at depth 64. A leaf can contain 0, 1 or multiple leaf entries.

### Fields
- `empty`: [`uint64`](#uint64) — An empty leaf.
- `single`: [`SmtLeafEntry`](#smtleafentry) — A single leaf entry.
- `multiple`: [`SmtLeafEntries`](#smtleafentries) — Multiple leaf entries.


## SmtLeafEntries {#smt-smtleafentries}
Represents multiple leaf entries in an SMT.

### Fields
- `entries`: _repeated_ [`SmtLeafEntry`](#smtleafentry) — The entries list.


## SmtLeafEntry {#smt-smtleafentry}
Represents a single SMT leaf entry.

### Fields
- `key`: [`digest.Digest`](#digest-digest) — The key of the entry.
- `value`: [`digest.Digest`](#digest-digest) — The value of the entry.


## SmtOpening {#smt-smtopening}
The opening of a leaf in an SMT.

### Fields
- `path`: [`merkle.MerklePath`](#merkle-merklepath) — The merkle path to the leaf.
- `leaf`: [`SmtLeaf`](#smtleaf) — The leaf itself.


## TransactionId {#transaction-transactionid}
Represents a transaction ID.

### Fields
- `id`: [`digest.Digest`](#digest-digest) — The transaction ID.


## TransactionSummary {#transaction-transactionsummary}
Represents a transaction summary.

### Fields
- `transaction_id`: [`TransactionId`](#transactionid) — The transaction ID.
- `block_num`: [`fixed32`](#fixed32) — The block number.
- `account_id`: [`account.AccountId`](#account-accountid) — The account ID.



# Scalar Value Types

| .proto Type | Notes | C++ Type | Java Type | Python Type |
| ----------- | ----- | -------- | --------- | ----------- |
| <div><h4 id="double" /></div><a name="double" /> `double` |  | `double` | `double` | `float` |
| <div><h4 id="float" /></div><a name="float" /> `float` |  | `float` | `float` | `float` |
| <div><h4 id="int32" /></div><a name="int32" /> `int32` | Uses variable-length encoding. Inefficient for encoding negative numbers – if your field is likely to have negative values, use sint32 instead. | `int32` | `int` | `int` |
| <div><h4 id="int64" /></div><a name="int64" /> `int64` | Uses variable-length encoding. Inefficient for encoding negative numbers – if your field is likely to have negative values, use sint64 instead. | `int64` | `long` | `int/long` |
| <div><h4 id="uint32" /></div><a name="uint32" /> `uint32` | Uses variable-length encoding. | `uint32` | `int` | `int/long` |
| <div><h4 id="uint64" /></div><a name="uint64" /> `uint64` | Uses variable-length encoding. | `uint64` | `long` | `int/long` |
| <div><h4 id="sint32" /></div><a name="sint32" /> `sint32` | Uses variable-length encoding. Signed int value. These more efficiently encode negative numbers than regular int32s. | `int32` | `int` | `int` |
| <div><h4 id="sint64" /></div><a name="sint64" /> `sint64` | Uses variable-length encoding. Signed int value. These more efficiently encode negative numbers than regular int64s. | `int64` | `long` | `int/long` |
| <div><h4 id="fixed32" /></div><a name="fixed32" /> `fixed32` | Always four bytes. More efficient than uint32 if values are often greater than 2^28. | `uint32` | `int` | `int` |
| <div><h4 id="fixed64" /></div><a name="fixed64" /> `fixed64` | Always eight bytes. More efficient than uint64 if values are often greater than 2^56. | `uint64` | `long` | `int/long` |
| <div><h4 id="sfixed32" /></div><a name="sfixed32" /> `sfixed32` | Always four bytes. | `int32` | `int` | `int` |
| <div><h4 id="sfixed64" /></div><a name="sfixed64" /> `sfixed64` | Always eight bytes. | `int64` | `long` | `int/long` |
| <div><h4 id="bool" /></div><a name="bool" /> `bool` |  | `bool` | `boolean` | `boolean` |
| <div><h4 id="string" /></div><a name="string" /> `string` | A string must always contain UTF-8 encoded or 7-bit ASCII text. | `string` | `String` | `str/unicode` |
| <div><h4 id="bytes" /></div><a name="bytes" /> `bytes` | May contain any arbitrary sequence of bytes. | `string` | `ByteString` | `str` |
