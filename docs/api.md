# Miden gRPC API Reference
## Table of Contents
- [Endpoints](#endpoints)
  - [`block_producer` methods](#block_producer-methods)
    - [SubmitProvenTransaction](#rpc-submitproventransaction)
  - [`rpc` methods](#rpc-methods)
    - [CheckNullifiers](#rpc-checknullifiers)
    - [CheckNullifiersByPrefix](#rpc-checknullifiersbyprefix)
    - [GetAccountDetails](#rpc-getaccountdetails)
    - [GetAccountProofs](#rpc-getaccountproofs)
    - [GetAccountStateDelta](#rpc-getaccountstatedelta)
    - [GetBlockByNumber](#rpc-getblockbynumber)
    - [GetBlockHeaderByNumber](#rpc-getblockheaderbynumber)
    - [GetNotesById](#rpc-getnotesbyid)
    - [SubmitProvenTransaction](#rpc-submitproventransaction)
    - [SyncNotes](#rpc-syncnotes)
    - [SyncState](#rpc-syncstate)
  - [`store` methods](#store-methods)
    - [ApplyBlock](#rpc-applyblock)
    - [CheckNullifiers](#rpc-checknullifiers)
    - [CheckNullifiersByPrefix](#rpc-checknullifiersbyprefix)
    - [GetAccountDetails](#rpc-getaccountdetails)
    - [GetAccountProofs](#rpc-getaccountproofs)
    - [GetAccountStateDelta](#rpc-getaccountstatedelta)
    - [GetBlockByNumber](#rpc-getblockbynumber)
    - [GetBlockHeaderByNumber](#rpc-getblockheaderbynumber)
    - [GetBlockInputs](#rpc-getblockinputs)
    - [GetNoteAuthenticationInfo](#rpc-getnoteauthenticationinfo)
    - [GetNotesById](#rpc-getnotesbyid)
    - [GetTransactionInputs](#rpc-gettransactioninputs)
    - [ListAccounts](#rpc-listaccounts)
    - [ListNotes](#rpc-listnotes)
    - [ListNullifiers](#rpc-listnullifiers)
    - [SyncNotes](#rpc-syncnotes)
    - [SyncState](#rpc-syncstate)
- [Messages](#messages)
  - [account.proto](#account-proto)
    - [AccountHeader](#account-accountheader)
    - [AccountId](#account-accountid)
    - [AccountInfo](#account-accountinfo)
    - [AccountSummary](#account-accountsummary)
  - [block.proto](#block-proto)
    - [BlockHeader](#block-blockheader)
    - [BlockInclusionProof](#block-blockinclusionproof)
  - [digest.proto](#digest-proto)
    - [Digest](#digest-digest)
  - [merkle.proto](#merkle-proto)
    - [MerklePath](#merkle-merklepath)
  - [mmr.proto](#mmr-proto)
    - [MmrDelta](#mmr-mmrdelta)
  - [note.proto](#note-proto)
    - [Note](#note-note)
    - [NoteAuthenticationInfo](#note-noteauthenticationinfo)
    - [NoteInclusionInBlockProof](#note-noteinclusioninblockproof)
    - [NoteMetadata](#note-notemetadata)
    - [NoteSyncRecord](#note-notesyncrecord)
  - [requests.proto](#requests-proto)
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
  - [responses.proto](#responses-proto)
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
  - [smt.proto](#smt-proto)
    - [SmtLeaf](#smt-smtleaf)
    - [SmtLeafEntries](#smt-smtleafentries)
    - [SmtLeafEntry](#smt-smtleafentry)
    - [SmtOpening](#smt-smtopening)
  - [transaction.proto](#transaction-proto)
    - [TransactionId](#transaction-transactionid)
    - [TransactionSummary](#transaction-transactionsummary)
- [Scalar Value Types](#scalar-value-types)

## Endpoints

### `block_producer` methods

#### <a name="rpc-submitproventransaction" />SubmitProvenTransaction
Submits proven transaction to the Miden network
> **rpc** SubmitProvenTransaction([SubmitProvenTransactionRequest](#requests-submitproventransactionrequest)) returns [SubmitProvenTransactionResponse](#responses-submitproventransactionresponse)

### `rpc` methods

#### <a name="rpc-checknullifiers" />CheckNullifiers
Gets a list of proofs for given nullifier hashes, each proof as a sparse Merkle Tree.
> **rpc** CheckNullifiers([CheckNullifiersRequest](#requests-checknullifiersrequest)) returns [CheckNullifiersResponse](#responses-checknullifiersresponse)

#### <a name="rpc-checknullifiersbyprefix" />CheckNullifiersByPrefix
Returns a list of nullifiers that match the specified prefixes and are recorded in the node.
> **rpc** CheckNullifiersByPrefix([CheckNullifiersByPrefixRequest](#requests-checknullifiersbyprefixrequest)) returns [CheckNullifiersByPrefixResponse](#responses-checknullifiersbyprefixresponse)

#### <a name="rpc-getaccountdetails" />GetAccountDetails
Returns the latest state of an account with the specified ID.
> **rpc** GetAccountDetails([GetAccountDetailsRequest](#requests-getaccountdetailsrequest)) returns [GetAccountDetailsResponse](#responses-getaccountdetailsresponse)

#### <a name="rpc-getaccountproofs" />GetAccountProofs
Returns the latest state proofs of accounts with the specified IDs.
> **rpc** GetAccountProofs([GetAccountProofsRequest](#requests-getaccountproofsrequest)) returns [GetAccountProofsResponse](#responses-getaccountproofsresponse)

#### <a name="rpc-getaccountstatedelta" />GetAccountStateDelta
Returns delta of the account states in the range from `from_block_num` (exclusive) to
`to_block_num` (inclusive).
> **rpc** GetAccountStateDelta([GetAccountStateDeltaRequest](#requests-getaccountstatedeltarequest)) returns [GetAccountStateDeltaResponse](#responses-getaccountstatedeltaresponse)

#### <a name="rpc-getblockbynumber" />GetBlockByNumber
Retrieves block data by given block number.
> **rpc** GetBlockByNumber([GetBlockByNumberRequest](#requests-getblockbynumberrequest)) returns [GetBlockByNumberResponse](#responses-getblockbynumberresponse)

#### <a name="rpc-getblockheaderbynumber" />GetBlockHeaderByNumber
Retrieves block header by given block number. Optionally, it also returns the MMR path
and current chain length to authenticate the block's inclusion.
> **rpc** GetBlockHeaderByNumber([GetBlockHeaderByNumberRequest](#requests-getblockheaderbynumberrequest)) returns [GetBlockHeaderByNumberResponse](#responses-getblockheaderbynumberresponse)

#### <a name="rpc-getnotesbyid" />GetNotesById
Returns a list of notes matching the provided note IDs.
> **rpc** GetNotesById([GetNotesByIdRequest](#requests-getnotesbyidrequest)) returns [GetNotesByIdResponse](#responses-getnotesbyidresponse)

#### <a name="rpc-submitproventransaction" />SubmitProvenTransaction
Submits proven transaction to the Miden network.
> **rpc** SubmitProvenTransaction([SubmitProvenTransactionRequest](#requests-submitproventransactionrequest)) returns [SubmitProvenTransactionResponse](#responses-submitproventransactionresponse)

#### <a name="rpc-syncnotes" />SyncNotes
Note synchronization request.

Specifies note tags that client is interested in. The server will return the first block which
contains a note matching `note_tags` or the chain tip.
> **rpc** SyncNotes([SyncNoteRequest](#requests-syncnoterequest)) returns [SyncNoteResponse](#responses-syncnoteresponse)

#### <a name="rpc-syncstate" />SyncState
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
> **rpc** SyncState([SyncStateRequest](#requests-syncstaterequest)) returns [SyncStateResponse](#responses-syncstateresponse)

### `store` methods

#### <a name="rpc-applyblock" />ApplyBlock
Applies changes of a new block to the DB and in-memory data structures.
> **rpc** ApplyBlock([ApplyBlockRequest](#requests-applyblockrequest)) returns [ApplyBlockResponse](#responses-applyblockresponse)

#### <a name="rpc-checknullifiers" />CheckNullifiers
Gets a list of proofs for given nullifier hashes, each proof as a sparse Merkle Tree.
> **rpc** CheckNullifiers([CheckNullifiersRequest](#requests-checknullifiersrequest)) returns [CheckNullifiersResponse](#responses-checknullifiersresponse)

#### <a name="rpc-checknullifiersbyprefix" />CheckNullifiersByPrefix
Returns a list of nullifiers that match the specified prefixes and are recorded in the node.
> **rpc** CheckNullifiersByPrefix([CheckNullifiersByPrefixRequest](#requests-checknullifiersbyprefixrequest)) returns [CheckNullifiersByPrefixResponse](#responses-checknullifiersbyprefixresponse)

#### <a name="rpc-getaccountdetails" />GetAccountDetails
Returns the latest state of an account with the specified ID.
> **rpc** GetAccountDetails([GetAccountDetailsRequest](#requests-getaccountdetailsrequest)) returns [GetAccountDetailsResponse](#responses-getaccountdetailsresponse)

#### <a name="rpc-getaccountproofs" />GetAccountProofs
Returns the latest state proofs of accounts with the specified IDs.
> **rpc** GetAccountProofs([GetAccountProofsRequest](#requests-getaccountproofsrequest)) returns [GetAccountProofsResponse](#responses-getaccountproofsresponse)

#### <a name="rpc-getaccountstatedelta" />GetAccountStateDelta
Returns delta of the account states in the range from `from_block_num` (exclusive) to
`to_block_num` (inclusive).
> **rpc** GetAccountStateDelta([GetAccountStateDeltaRequest](#requests-getaccountstatedeltarequest)) returns [GetAccountStateDeltaResponse](#responses-getaccountstatedeltaresponse)

#### <a name="rpc-getblockbynumber" />GetBlockByNumber
Retrieves block data by given block number.
> **rpc** GetBlockByNumber([GetBlockByNumberRequest](#requests-getblockbynumberrequest)) returns [GetBlockByNumberResponse](#responses-getblockbynumberresponse)

#### <a name="rpc-getblockheaderbynumber" />GetBlockHeaderByNumber
Retrieves block header by given block number. Optionally, it also returns the MMR path
and current chain length to authenticate the block's inclusion.
> **rpc** GetBlockHeaderByNumber([GetBlockHeaderByNumberRequest](#requests-getblockheaderbynumberrequest)) returns [GetBlockHeaderByNumberResponse](#responses-getblockheaderbynumberresponse)

#### <a name="rpc-getblockinputs" />GetBlockInputs
Returns data needed by the block producer to construct and prove the next block, including
account states, nullifiers, and unauthenticated notes.
> **rpc** GetBlockInputs([GetBlockInputsRequest](#requests-getblockinputsrequest)) returns [GetBlockInputsResponse](#responses-getblockinputsresponse)

#### <a name="rpc-getnoteauthenticationinfo" />GetNoteAuthenticationInfo
Returns a list of Note inclusion proofs for the specified Note IDs.
> **rpc** GetNoteAuthenticationInfo([GetNoteAuthenticationInfoRequest](#requests-getnoteauthenticationinforequest)) returns [GetNoteAuthenticationInfoResponse](#responses-getnoteauthenticationinforesponse)

#### <a name="rpc-getnotesbyid" />GetNotesById
Returns a list of notes matching the provided note IDs.
> **rpc** GetNotesById([GetNotesByIdRequest](#requests-getnotesbyidrequest)) returns [GetNotesByIdResponse](#responses-getnotesbyidresponse)

#### <a name="rpc-gettransactioninputs" />GetTransactionInputs
Returns the data needed by the block producer to check validity of an incoming transaction.
> **rpc** GetTransactionInputs([GetTransactionInputsRequest](#requests-gettransactioninputsrequest)) returns [GetTransactionInputsResponse](#responses-gettransactioninputsresponse)

#### <a name="rpc-listaccounts" />ListAccounts
Lists all accounts of the current chain.
> **rpc** ListAccounts([ListAccountsRequest](#requests-listaccountsrequest)) returns [ListAccountsResponse](#responses-listaccountsresponse)

#### <a name="rpc-listnotes" />ListNotes
Lists all notes of the current chain.
> **rpc** ListNotes([ListNotesRequest](#requests-listnotesrequest)) returns [ListNotesResponse](#responses-listnotesresponse)

#### <a name="rpc-listnullifiers" />ListNullifiers
Lists all nullifiers of the current chain.
> **rpc** ListNullifiers([ListNullifiersRequest](#requests-listnullifiersrequest)) returns [ListNullifiersResponse](#responses-listnullifiersresponse)

#### <a name="rpc-syncnotes" />SyncNotes
Note synchronization request.

Specifies note tags that client is interested in. The server will return the first block which
contains a note matching `note_tags` or the chain tip.
> **rpc** SyncNotes([SyncNoteRequest](#requests-syncnoterequest)) returns [SyncNoteResponse](#responses-syncnoteresponse)

#### <a name="rpc-syncstate" />SyncState
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
> **rpc** SyncState([SyncStateRequest](#requests-syncstaterequest)) returns [SyncStateResponse](#responses-syncstateresponse)


## Messages

### <a name="account-proto" />account.proto

#### <a name="account-accountheader" />AccountHeader
An account header.

##### Fields
- `vault_root`: [`digest.Digest`](#digest-digest) — Vault root hash.
- `storage_commitment`: [`digest.Digest`](#digest-digest) — Storage root hash.
- `code_commitment`: [`digest.Digest`](#digest-digest) — Code root hash.
- `nonce`: [`uint64`](#uint64) — Account nonce.


#### <a name="account-accountid" />AccountId
An account ID.

##### Fields
- `id`: [`fixed64`](#fixed64) — A miden account is defined with a little bit of proof-of-work, the id itself is defined as the first word of a hash digest. For this reason account ids can be considered as random values, because of that the encoding below uses fixed 64 bits, instead of zig-zag encoding.


#### <a name="account-accountinfo" />AccountInfo
An account info.

##### Fields
- `summary`: [`account.AccountSummary`](#account-accountsummary) — Account summary.
- `details`: [optional] [`bytes`](#bytes) — Account details encoded using Miden native format.


#### <a name="account-accountsummary" />AccountSummary
A summary of an account.

##### Fields
- `account_id`: [`account.AccountId`](#account-accountid) — The account ID.
- `account_hash`: [`digest.Digest`](#digest-digest) — The latest account hash, zero hash if the account doesn't exist.
- `block_num`: [`uint32`](#uint32) — Merkle path to verify the account's inclusion in the MMR.


### <a name="block-proto" />block.proto

#### <a name="block-blockheader" />BlockHeader
Represents a block header.

##### Fields
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


#### <a name="block-blockinclusionproof" />BlockInclusionProof
Represents a block inclusion proof.

##### Fields
- `block_header`: [`block.BlockHeader`](#block-blockheader) — Block header associated with the inclusion proof.
- `mmr_path`: [`merkle.MerklePath`](#merkle-merklepath) — Merkle path associated with the inclusion proof.
- `chain_length`: [`fixed32`](#fixed32) — The chain length associated with `mmr_path`.


### <a name="digest-proto" />digest.proto

#### <a name="digest-digest" />Digest
A hash digest, the result of a hash function.

##### Fields
- `d0`: [`fixed64`](#fixed64) — none
- `d1`: [`fixed64`](#fixed64) — none
- `d2`: [`fixed64`](#fixed64) — none
- `d3`: [`fixed64`](#fixed64) — none


### <a name="merkle-proto" />merkle.proto

#### <a name="merkle-merklepath" />MerklePath
Represents a Merkle path.

##### Fields
- `siblings`: [repeated] [`digest.Digest`](#digest-digest) — List of sibling node hashes, in order from the root to the leaf.


### <a name="mmr-proto" />mmr.proto

#### <a name="mmr-mmrdelta" />MmrDelta
Represents an MMR delta.

##### Fields
- `forest`: [`uint64`](#uint64) — The number of trees in the forest (latest block number + 1).
- `data`: [repeated] [`digest.Digest`](#digest-digest) — New and changed MMR peaks.


### <a name="note-proto" />note.proto

#### <a name="note-note" />Note
Represents a note.

##### Fields
- `block_num`: [`fixed32`](#fixed32) — The block number in which the note was created.
- `note_index`: [`uint32`](#uint32) — The index of the note in the block.
- `note_id`: [`digest.Digest`](#digest-digest) — The ID of the note.
- `metadata`: [`note.NoteMetadata`](#note-notemetadata) — The note metadata.
- `merkle_path`: [`merkle.MerklePath`](#merkle-merklepath) — The note inclusion proof in the block.
- `details`: [optional] [`bytes`](#bytes) — This field will be present when the note is public. details contain the `Note` in a serialized format.


#### <a name="note-noteauthenticationinfo" />NoteAuthenticationInfo
Represents proof of notes inclusion in the block(s) and block(s) inclusion in the chain.

##### Fields
- `note_proofs`: [repeated] [`note.NoteInclusionInBlockProof`](#note-noteinclusioninblockproof) — Proof of each note's inclusion in a block.
- `block_proofs`: [repeated] [`block.BlockInclusionProof`](#block-blockinclusionproof) — Proof of each block's inclusion in the chain.


#### <a name="note-noteinclusioninblockproof" />NoteInclusionInBlockProof
Represents proof of a note's inclusion in a block.

##### Fields
- `note_id`: [`digest.Digest`](#digest-digest) — The ID of the note.
- `block_num`: [`fixed32`](#fixed32) — The block number in which the note was created.
- `note_index_in_block`: [`uint32`](#uint32) — The index of the note in the block.
- `merkle_path`: [`merkle.MerklePath`](#merkle-merklepath) — The note inclusion proof in the block.


#### <a name="note-notemetadata" />NoteMetadata
Represents a note metadata.

##### Fields
- `sender`: [`account.AccountId`](#account-accountid) — The sender of the note.
- `note_type`: [`uint32`](#uint32) — The type of the note (0b01 = public, 0b10 = private, 0b11 = encrypted).
- `tag`: [`fixed32`](#fixed32) — A value which can be used by the recipient(s) to identify notes intended for them.
- `execution_hint`: [`fixed64`](#fixed64) — Specifies when a note is ready to be consumed: (6 least significant bits - hint identifier (tag), bits 6 to 38 - Hint payload). See `miden_objects::notes::execution_hint` for more info.
- `aux`: [`fixed64`](#fixed64) — An arbitrary user-defined value.


#### <a name="note-notesyncrecord" />NoteSyncRecord
Represents proof of a note inclusion in the block.

##### Fields
- `note_index`: [`uint32`](#uint32) — The index of the note.
- `note_id`: [`digest.Digest`](#digest-digest) — The ID of the note.
- `metadata`: [`note.NoteMetadata`](#note-notemetadata) — The note metadata.
- `merkle_path`: [`merkle.MerklePath`](#merkle-merklepath) — The note inclusion proof in the block.


### <a name="requests-proto" />requests.proto

#### <a name="requests-applyblockrequest" />ApplyBlockRequest
Applies changes of a new block to the DB and in-memory data structures.

##### Fields
- `block`: [`bytes`](#bytes) — Block data encoded using Miden's native format.


#### <a name="requests-checknullifiersbyprefixrequest" />CheckNullifiersByPrefixRequest
Returns a list of nullifiers that match the specified prefixes and are recorded in the node.

##### Fields
- `prefix_len`: [`uint32`](#uint32) — Number of bits used for nullifier prefix. Currently the only supported value is 16.
- `nullifiers`: [repeated] [`uint32`](#uint32) — List of nullifiers to check. Each nullifier is specified by its prefix with length equal to `prefix_len`.


#### <a name="requests-checknullifiersrequest" />CheckNullifiersRequest
Get a list of proofs for given nullifier hashes, each proof as a sparse Merkle Tree.

##### Fields
- `nullifiers`: [repeated] [`digest.Digest`](#digest-digest) — List of nullifiers to return proofs for.


#### <a name="requests-getaccountdetailsrequest" />GetAccountDetailsRequest
Returns the latest state of an account with the specified ID.

##### Fields
- `account_id`: [`account.AccountId`](#account-accountid) — Account ID to get details.


#### <a name="requests-getaccountproofsrequest" />GetAccountProofsRequest
Returns the latest state proofs of accounts with the specified IDs.

##### Fields
- `account_ids`: [repeated] [`account.AccountId`](#account-accountid) — List of account IDs to get states.
- `include_headers`: [optional] [`bool`](#bool) — Optional flag to include header and account code in the response. `false` by default.
- `code_commitments`: [repeated] [`digest.Digest`](#digest-digest) — Account code commitments corresponding to the last-known `AccountCode` for requested accounts. Responses will include only the ones that are not known to the caller. These are not associated with a specific account but rather, they will be matched against all requested accounts.


#### <a name="requests-getaccountstatedeltarequest" />GetAccountStateDeltaRequest
Returns delta of the account states in the range from `from_block_num` (exclusive) to
`to_block_num` (inclusive).

##### Fields
- `account_id`: [`account.AccountId`](#account-accountid) — ID of the account for which the delta is requested.
- `from_block_num`: [`fixed32`](#fixed32) — Block number from which the delta is requested (exclusive).
- `to_block_num`: [`fixed32`](#fixed32) — Block number up to which the delta is requested (inclusive).


#### <a name="requests-getblockbynumberrequest" />GetBlockByNumberRequest
Retrieves block data by given block number.

##### Fields
- `block_num`: [`fixed32`](#fixed32) — The block number of the target block.


#### <a name="requests-getblockheaderbynumberrequest" />GetBlockHeaderByNumberRequest
Returns the block header corresponding to the requested block number, as well as the merkle
path and current forest which validate the block's inclusion in the chain.

The Merkle path is an MMR proof for the block's leaf, based on the current chain length.

##### Fields
- `block_num`: [optional] [`uint32`](#uint32) — The block number of the target block. If not provided, means latest known block.
- `include_mmr_proof`: [optional] [`bool`](#bool) — Whether or not to return authentication data for the block header.


#### <a name="requests-getblockinputsrequest" />GetBlockInputsRequest
Returns data needed by the block producer to construct and prove the next block, including
account states, nullifiers, and unauthenticated notes.

##### Fields
- `account_ids`: [repeated] [`account.AccountId`](#account-accountid) — ID of the account against which a transaction is executed.
- `nullifiers`: [repeated] [`digest.Digest`](#digest-digest) — Array of nullifiers for all notes consumed by a transaction.
- `unauthenticated_notes`: [repeated] [`digest.Digest`](#digest-digest) — Array of note IDs to be checked for existence in the database.


#### <a name="requests-getnoteauthenticationinforequest" />GetNoteAuthenticationInfoRequest
Returns a list of Note inclusion proofs for the specified Note IDs.

##### Fields
- `note_ids`: [repeated] [`digest.Digest`](#digest-digest) — List of NoteId's to be queried from the database.


#### <a name="requests-getnotesbyidrequest" />GetNotesByIdRequest
Returns a list of notes matching the provided note IDs.

##### Fields
- `note_ids`: [repeated] [`digest.Digest`](#digest-digest) — List of NoteId's to be queried from the database.


#### <a name="requests-gettransactioninputsrequest" />GetTransactionInputsRequest
Returns the data needed by the block producer to check validity of an incoming transaction.

##### Fields
- `account_id`: [`account.AccountId`](#account-accountid) — ID of the account against which a transaction is executed.
- `nullifiers`: [repeated] [`digest.Digest`](#digest-digest) — Array of nullifiers for all notes consumed by a transaction.
- `unauthenticated_notes`: [repeated] [`digest.Digest`](#digest-digest) — Array of unauthenticated note IDs to be checked for existence in the database.


#### <a name="requests-listaccountsrequest" />ListAccountsRequest
Lists all accounts of the current chain.

##### Fields
No fields

#### <a name="requests-listnotesrequest" />ListNotesRequest
Lists all notes of the current chain.

##### Fields
No fields

#### <a name="requests-listnullifiersrequest" />ListNullifiersRequest
Lists all nullifiers of the current chain.

##### Fields
No fields

#### <a name="requests-submitproventransactionrequest" />SubmitProvenTransactionRequest
Submits proven transaction to the Miden network.

##### Fields
- `transaction`: [`bytes`](#bytes) — Transaction encoded using Miden's native format.


#### <a name="requests-syncnoterequest" />SyncNoteRequest
Note synchronization request.

Specifies note tags that client is interested in. The server will return the first block which
contains a note matching `note_tags` or the chain tip.

##### Fields
- `block_num`: [`fixed32`](#fixed32) — Last block known by the client. The response will contain data starting from the next block, until the first block which contains a note of matching the requested tag.
- `note_tags`: [repeated] [`fixed32`](#fixed32) — Specifies the tags which the client is interested in.


#### <a name="requests-syncstaterequest" />SyncStateRequest
State synchronization request.

Specifies state updates the client is interested in. The server will return the first block which
contains a note matching `note_tags` or the chain tip. And the corresponding updates to
`nullifiers` and `account_ids` for that block range.

##### Fields
- `block_num`: [`fixed32`](#fixed32) — Last block known by the client. The response will contain data starting from the next block, until the first block which contains a note of matching the requested tag, or the chain tip if there are no notes.
- `account_ids`: [repeated] [`account.AccountId`](#account-accountid) — Accounts' hash to include in the response. An account hash will be included if-and-only-if it is the latest update. Meaning it is possible there was an update to the account for the given range, but if it is not the latest, it won't be included in the response.
- `note_tags`: [repeated] [`fixed32`](#fixed32) — Specifies the tags which the client is interested in.
- `nullifiers`: [repeated] [`uint32`](#uint32) — Determines the nullifiers the client is interested in by specifying the 16high bits of the target nullifier.


### <a name="responses-proto" />responses.proto

#### <a name="responses-accountblockinputrecord" />AccountBlockInputRecord
An account returned as a response to the `GetBlockInputs`.

##### Fields
- `account_id`: [`account.AccountId`](#account-accountid) — The account ID.
- `account_hash`: [`digest.Digest`](#digest-digest) — The latest account hash, zero hash if the account doesn't exist.
- `proof`: [`merkle.MerklePath`](#merkle-merklepath) — Merkle path to verify the account's inclusion in the MMR.


#### <a name="responses-accountproofsresponse" />AccountProofsResponse
A single account proof returned as a response to the `GetAccountProofs`.

##### Fields
- `account_id`: [`account.AccountId`](#account-accountid) — Account ID.
- `account_hash`: [`digest.Digest`](#digest-digest) — Account hash.
- `account_proof`: [`merkle.MerklePath`](#merkle-merklepath) — Authentication path from the `account_root` of the block header to the account.
- `state_header`: [optional] [`responses.AccountStateHeader`](#responses-accountstateheader) — State header for public accounts. Filled only if `include_headers` flag is set to `true`.


#### <a name="responses-accountstateheader" />AccountStateHeader
State header for public accounts.

##### Fields
- `header`: [`account.AccountHeader`](#account-accountheader) — Account header.
- `storage_header`: [`bytes`](#bytes) — Values of all account storage slots (max 255).
- `account_code`: [optional] [`bytes`](#bytes) — Account code, returned only when none of the request's code commitments match with the current one.


#### <a name="responses-accounttransactioninputrecord" />AccountTransactionInputRecord
An account returned as a response to the `GetTransactionInputs`.

##### Fields
- `account_id`: [`account.AccountId`](#account-accountid) — The account ID.
- `account_hash`: [`digest.Digest`](#digest-digest) — The latest account hash, zero hash if the account doesn't exist.


#### <a name="responses-applyblockresponse" />ApplyBlockResponse
Represents the result of applying a block.

##### Fields
No fields

#### <a name="responses-checknullifiersbyprefixresponse" />CheckNullifiersByPrefixResponse
Represents the result of checking nullifiers by prefix.

##### Fields
- `nullifiers`: [repeated] [`responses.NullifierUpdate`](#responses-nullifierupdate) — List of nullifiers matching the prefixes specified in the request.


#### <a name="responses-checknullifiersresponse" />CheckNullifiersResponse
Represents the result of checking nullifiers.

##### Fields
- `proofs`: [repeated] [`smt.SmtOpening`](#smt-smtopening) — Each requested nullifier has its corresponding nullifier proof at the same position.


#### <a name="responses-getaccountdetailsresponse" />GetAccountDetailsResponse
Represents the result of getting account details.

##### Fields
- `details`: [`account.AccountInfo`](#account-accountinfo) — Account info (with details for public accounts).


#### <a name="responses-getaccountproofsresponse" />GetAccountProofsResponse
Represents the result of getting account proofs.

##### Fields
- `block_num`: [`fixed32`](#fixed32) — Block number at which the state of the account was returned.
- `account_proofs`: [repeated] [`responses.AccountProofsResponse`](#responses-accountproofsresponse) — List of account state infos for the requested account keys.


#### <a name="responses-getaccountstatedeltaresponse" />GetAccountStateDeltaResponse
Represents the result of getting account state delta.

##### Fields
- `delta`: [optional] [`bytes`](#bytes) — The calculated `AccountStateDelta` encoded using Miden native format.


#### <a name="responses-getblockbynumberresponse" />GetBlockByNumberResponse
Represents the result of getting block by number.

##### Fields
- `block`: [optional] [`bytes`](#bytes) — The requested `Block` data encoded using Miden native format.


#### <a name="responses-getblockheaderbynumberresponse" />GetBlockHeaderByNumberResponse
Represents the result of getting a block header by block number.

##### Fields
- `block_header`: [`block.BlockHeader`](#block-blockheader) — The requested block header.
- `mmr_path`: [optional] [`merkle.MerklePath`](#merkle-merklepath) — Merkle path to verify the block's inclusion in the MMR at the returned `chain_length`.
- `chain_length`: [optional] [`fixed32`](#fixed32) — Current chain length.


#### <a name="responses-getblockinputsresponse" />GetBlockInputsResponse
Represents the result of getting block inputs.

##### Fields
- `block_header`: [`block.BlockHeader`](#block-blockheader) — The latest block header.
- `mmr_peaks`: [repeated] [`digest.Digest`](#digest-digest) — Peaks of the above block's mmr, The `forest` value is equal to the block number.
- `account_states`: [repeated] [`responses.AccountBlockInputRecord`](#responses-accountblockinputrecord) — The hashes of the requested accounts and their authentication paths.
- `nullifiers`: [repeated] [`responses.NullifierBlockInputRecord`](#responses-nullifierblockinputrecord) — The requested nullifiers and their authentication paths.
- `found_unauthenticated_notes`: [`note.NoteAuthenticationInfo`](#note-noteauthenticationinfo) — The list of requested notes which were found in the database.


#### <a name="responses-getnoteauthenticationinforesponse" />GetNoteAuthenticationInfoResponse
Represents the result of getting note authentication info.

##### Fields
- `proofs`: [`note.NoteAuthenticationInfo`](#note-noteauthenticationinfo) — Proofs of note inclusions in blocks and block inclusions in chain.


#### <a name="responses-getnotesbyidresponse" />GetNotesByIdResponse
Represents the result of getting notes by IDs.

##### Fields
- `notes`: [repeated] [`note.Note`](#note-note) — Lists Note's returned by the database.


#### <a name="responses-gettransactioninputsresponse" />GetTransactionInputsResponse
Represents the result of getting transaction inputs.

##### Fields
- `account_state`: [`responses.AccountTransactionInputRecord`](#responses-accounttransactioninputrecord) — Account state proof.
- `nullifiers`: [repeated] [`responses.NullifierTransactionInputRecord`](#responses-nullifiertransactioninputrecord) — List of nullifiers that have been consumed.
- `missing_unauthenticated_notes`: [repeated] [`digest.Digest`](#digest-digest) — List of unauthenticated notes that were not found in the database.
- `block_height`: [`fixed32`](#fixed32) — The node's current block height.


#### <a name="responses-listaccountsresponse" />ListAccountsResponse
Represents the result of getting accounts list.

##### Fields
- `accounts`: [repeated] [`account.AccountInfo`](#account-accountinfo) — Lists all accounts of the current chain.


#### <a name="responses-listnotesresponse" />ListNotesResponse
Represents the result of getting notes list.

##### Fields
- `notes`: [repeated] [`note.Note`](#note-note) — Lists all notes of the current chain.


#### <a name="responses-listnullifiersresponse" />ListNullifiersResponse
Represents the result of getting nullifiers list.

##### Fields
- `nullifiers`: [map] [`smt.SmtLeafEntry`](#smt-smtleafentry) — Lists all nullifiers of the current chain.


#### <a name="responses-nullifierblockinputrecord" />NullifierBlockInputRecord
A nullifier returned as a response to the `GetBlockInputs`.

##### Fields
- `nullifier`: [`digest.Digest`](#digest-digest) — The nullifier ID.
- `opening`: [`smt.SmtOpening`](#smt-smtopening) — Merkle path to verify the nullifier's inclusion in the MMR.


#### <a name="responses-nullifiertransactioninputrecord" />NullifierTransactionInputRecord
A nullifier returned as a response to the `GetTransactionInputs`.

##### Fields
- `nullifier`: [`digest.Digest`](#digest-digest) — The nullifier ID.
- `block_num`: [`fixed32`](#fixed32) — The block at which the nullifier has been consumed, zero if not consumed.


#### <a name="responses-nullifierupdate" />NullifierUpdate
Represents a single nullifier update.

##### Fields
- `nullifier`: [`digest.Digest`](#digest-digest) — Nullifier ID.
- `block_num`: [`fixed32`](#fixed32) — Block number.


#### <a name="responses-submitproventransactionresponse" />SubmitProvenTransactionResponse
Represents the result of submitting proven transaction.

##### Fields
- `block_height`: [`fixed32`](#fixed32) — The node's current block height.


#### <a name="responses-syncnoteresponse" />SyncNoteResponse
Represents the result of syncing notes request.

##### Fields
- `chain_tip`: [`fixed32`](#fixed32) — Number of the latest block in the chain.
- `block_header`: [`block.BlockHeader`](#block-blockheader) — Block header of the block with the first note matching the specified criteria.
- `mmr_path`: [`merkle.MerklePath`](#merkle-merklepath) — Merkle path to verify the block's inclusion in the MMR at the returned `chain_tip`.

An MMR proof can be constructed for the leaf of index `block_header.block_num` of an MMR of forest `chain_tip` with this path.
- `notes`: [repeated] [`note.NoteSyncRecord`](#note-notesyncrecord) — List of all notes together with the Merkle paths from `response.block_header.note_root`.


#### <a name="responses-syncstateresponse" />SyncStateResponse
Represents the result of syncing state request.

##### Fields
- `chain_tip`: [`fixed32`](#fixed32) — Number of the latest block in the chain.
- `block_header`: [`block.BlockHeader`](#block-blockheader) — Block header of the block with the first note matching the specified criteria.
- `mmr_delta`: [`mmr.MmrDelta`](#mmr-mmrdelta) — Data needed to update the partial MMR from `request.block_num + 1` to `response.block_header.block_num`.
- `accounts`: [repeated] [`account.AccountSummary`](#account-accountsummary) — List of account hashes updated after `request.block_num + 1` but not after `response.block_header.block_num`.
- `transactions`: [repeated] [`transaction.TransactionSummary`](#transaction-transactionsummary) — List of transactions executed against requested accounts between `request.block_num + 1` and `response.block_header.block_num`.
- `notes`: [repeated] [`note.NoteSyncRecord`](#note-notesyncrecord) — List of all notes together with the Merkle paths from `response.block_header.note_root`.
- `nullifiers`: [repeated] [`responses.NullifierUpdate`](#responses-nullifierupdate) — List of nullifiers created between `request.block_num + 1` and `response.block_header.block_num`.


### <a name="smt-proto" />smt.proto

#### <a name="smt-smtleaf" />SmtLeaf
A leaf in an SMT, sitting at depth 64. A leaf can contain 0, 1 or multiple leaf entries.

##### Fields
- `empty`: [`uint64`](#uint64) — An empty leaf.
- `single`: [`smt.SmtLeafEntry`](#smt-smtleafentry) — A single leaf entry.
- `multiple`: [`smt.SmtLeafEntries`](#smt-smtleafentries) — Multiple leaf entries.


#### <a name="smt-smtleafentries" />SmtLeafEntries
Represents multiple leaf entries in an SMT.

##### Fields
- `entries`: [repeated] [`smt.SmtLeafEntry`](#smt-smtleafentry) — The entries list.


#### <a name="smt-smtleafentry" />SmtLeafEntry
Represents a single SMT leaf entry.

##### Fields
- `key`: [`digest.Digest`](#digest-digest) — The key of the entry.
- `value`: [`digest.Digest`](#digest-digest) — The value of the entry.


#### <a name="smt-smtopening" />SmtOpening
The opening of a leaf in an SMT.

##### Fields
- `path`: [`merkle.MerklePath`](#merkle-merklepath) — The merkle path to the leaf.
- `leaf`: [`smt.SmtLeaf`](#smt-smtleaf) — The leaf itself.


### <a name="transaction-proto" />transaction.proto

#### <a name="transaction-transactionid" />TransactionId
Represents a transaction ID.

##### Fields
- `id`: [`digest.Digest`](#digest-digest) — The transaction ID.


#### <a name="transaction-transactionsummary" />TransactionSummary
Represents a transaction summary.

##### Fields
- `transaction_id`: [`transaction.TransactionId`](#transaction-transactionid) — The transaction ID.
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
