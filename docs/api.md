# Protocol Documentation
<a name="top"></a>

## Table of Contents

- [account.proto](#account-proto)
    - [AccountHeader](#account-AccountHeader)
    - [AccountId](#account-AccountId)
    - [AccountInfo](#account-AccountInfo)
    - [AccountSummary](#account-AccountSummary)
  
- [block.proto](#block-proto)
    - [BlockHeader](#block-BlockHeader)
    - [BlockInclusionProof](#block-BlockInclusionProof)
  
- [block_producer.proto](#block_producer-proto)
    - [Api](#block_producer-Api)
  
- [digest.proto](#digest-proto)
    - [Digest](#digest-Digest)
  
- [merkle.proto](#merkle-proto)
    - [MerklePath](#merkle-MerklePath)
  
- [mmr.proto](#mmr-proto)
    - [MmrDelta](#mmr-MmrDelta)
  
- [note.proto](#note-proto)
    - [Note](#note-Note)
    - [NoteAuthenticationInfo](#note-NoteAuthenticationInfo)
    - [NoteInclusionInBlockProof](#note-NoteInclusionInBlockProof)
    - [NoteMetadata](#note-NoteMetadata)
    - [NoteSyncRecord](#note-NoteSyncRecord)
  
- [requests.proto](#requests-proto)
    - [ApplyBlockRequest](#requests-ApplyBlockRequest)
    - [CheckNullifiersByPrefixRequest](#requests-CheckNullifiersByPrefixRequest)
    - [CheckNullifiersRequest](#requests-CheckNullifiersRequest)
    - [GetAccountDetailsRequest](#requests-GetAccountDetailsRequest)
    - [GetAccountProofsRequest](#requests-GetAccountProofsRequest)
    - [GetAccountStateDeltaRequest](#requests-GetAccountStateDeltaRequest)
    - [GetBlockByNumberRequest](#requests-GetBlockByNumberRequest)
    - [GetBlockHeaderByNumberRequest](#requests-GetBlockHeaderByNumberRequest)
    - [GetBlockInputsRequest](#requests-GetBlockInputsRequest)
    - [GetNoteAuthenticationInfoRequest](#requests-GetNoteAuthenticationInfoRequest)
    - [GetNotesByIdRequest](#requests-GetNotesByIdRequest)
    - [GetTransactionInputsRequest](#requests-GetTransactionInputsRequest)
    - [ListAccountsRequest](#requests-ListAccountsRequest)
    - [ListNotesRequest](#requests-ListNotesRequest)
    - [ListNullifiersRequest](#requests-ListNullifiersRequest)
    - [SubmitProvenTransactionRequest](#requests-SubmitProvenTransactionRequest)
    - [SyncNoteRequest](#requests-SyncNoteRequest)
    - [SyncStateRequest](#requests-SyncStateRequest)
  
- [responses.proto](#responses-proto)
    - [AccountBlockInputRecord](#responses-AccountBlockInputRecord)
    - [AccountProofsResponse](#responses-AccountProofsResponse)
    - [AccountStateHeader](#responses-AccountStateHeader)
    - [AccountTransactionInputRecord](#responses-AccountTransactionInputRecord)
    - [ApplyBlockResponse](#responses-ApplyBlockResponse)
    - [CheckNullifiersByPrefixResponse](#responses-CheckNullifiersByPrefixResponse)
    - [CheckNullifiersResponse](#responses-CheckNullifiersResponse)
    - [GetAccountDetailsResponse](#responses-GetAccountDetailsResponse)
    - [GetAccountProofsResponse](#responses-GetAccountProofsResponse)
    - [GetAccountStateDeltaResponse](#responses-GetAccountStateDeltaResponse)
    - [GetBlockByNumberResponse](#responses-GetBlockByNumberResponse)
    - [GetBlockHeaderByNumberResponse](#responses-GetBlockHeaderByNumberResponse)
    - [GetBlockInputsResponse](#responses-GetBlockInputsResponse)
    - [GetNoteAuthenticationInfoResponse](#responses-GetNoteAuthenticationInfoResponse)
    - [GetNotesByIdResponse](#responses-GetNotesByIdResponse)
    - [GetTransactionInputsResponse](#responses-GetTransactionInputsResponse)
    - [ListAccountsResponse](#responses-ListAccountsResponse)
    - [ListNotesResponse](#responses-ListNotesResponse)
    - [ListNullifiersResponse](#responses-ListNullifiersResponse)
    - [NullifierBlockInputRecord](#responses-NullifierBlockInputRecord)
    - [NullifierTransactionInputRecord](#responses-NullifierTransactionInputRecord)
    - [NullifierUpdate](#responses-NullifierUpdate)
    - [SubmitProvenTransactionResponse](#responses-SubmitProvenTransactionResponse)
    - [SyncNoteResponse](#responses-SyncNoteResponse)
    - [SyncStateResponse](#responses-SyncStateResponse)
  
- [rpc.proto](#rpc-proto)
    - [Api](#rpc-Api)
  
- [smt.proto](#smt-proto)
    - [SmtLeaf](#smt-SmtLeaf)
    - [SmtLeafEntries](#smt-SmtLeafEntries)
    - [SmtLeafEntry](#smt-SmtLeafEntry)
    - [SmtOpening](#smt-SmtOpening)
  
- [store.proto](#store-proto)
    - [Api](#store-Api)
  
- [transaction.proto](#transaction-proto)
    - [TransactionId](#transaction-TransactionId)
    - [TransactionSummary](#transaction-TransactionSummary)
  
- [Scalar Value Types](#scalar-value-types)



<a name="account-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## account.proto



<a name="account-AccountHeader"></a>

### AccountHeader
An account header


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| vault_root | [digest.Digest](#digest-Digest) |  | Vault root hash |
| storage_commitment | [digest.Digest](#digest-Digest) |  | Storage root hash |
| code_commitment | [digest.Digest](#digest-Digest) |  | Code root hash |
| nonce | [uint64](#uint64) |  | Account nonce |






<a name="account-AccountId"></a>

### AccountId
An account ID


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| id | [fixed64](#fixed64) |  | A miden account is defined with a little bit of proof-of-work, the id itself is defined as the first word of a hash digest. For this reason account ids can be considered as random values, because of that the encoding below uses fixed 64 bits, instead of zig-zag encoding. |






<a name="account-AccountInfo"></a>

### AccountInfo
An account info


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| summary | [AccountSummary](#account-AccountSummary) |  | Account summary |
| details | [bytes](#bytes) | optional | Account details encoded using Miden native format |






<a name="account-AccountSummary"></a>

### AccountSummary
A summary of an account


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| account_id | [AccountId](#account-AccountId) |  | The account ID |
| account_hash | [digest.Digest](#digest-Digest) |  | The latest account hash, zero hash if the account doesn&#39;t exist |
| block_num | [uint32](#uint32) |  | Merkle path to verify the account&#39;s inclusion in the MMR |





 

 

 

 



<a name="block-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## block.proto



<a name="block-BlockHeader"></a>

### BlockHeader
Represents a block header


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| version | [uint32](#uint32) |  | Specifies the version of the protocol |
| prev_hash | [digest.Digest](#digest-Digest) |  | The hash of the previous blocks header |
| block_num | [fixed32](#fixed32) |  | A unique sequential number of the current block |
| chain_root | [digest.Digest](#digest-Digest) |  | A commitment to an MMR of the entire chain where each block is a leaf |
| account_root | [digest.Digest](#digest-Digest) |  | A commitment to account database |
| nullifier_root | [digest.Digest](#digest-Digest) |  | A commitment to the nullifier database |
| note_root | [digest.Digest](#digest-Digest) |  | A commitment to all notes created in the current block |
| tx_hash | [digest.Digest](#digest-Digest) |  | A commitment to a set of IDs of transactions which affected accounts in this block |
| proof_hash | [digest.Digest](#digest-Digest) |  | A hash of a STARK proof attesting to the correct state transition |
| kernel_root | [digest.Digest](#digest-Digest) |  | A commitment to all transaction kernels supported by this block |
| timestamp | [fixed32](#fixed32) |  | The time when the block was created |






<a name="block-BlockInclusionProof"></a>

### BlockInclusionProof
Represents a block inclusion proof


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| block_header | [BlockHeader](#block-BlockHeader) |  | Block header associated with the inclusion proof |
| mmr_path | [merkle.MerklePath](#merkle-MerklePath) |  | Merkle path associated with the inclusion proof |
| chain_length | [fixed32](#fixed32) |  | The chain length associated with `mmr_path` |





 

 

 

 



<a name="block_producer-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## block_producer.proto
Specification of the user facing gRPC API.

 

 

 


<a name="block_producer-Api"></a>

### Api


| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| SubmitProvenTransaction | [.requests.SubmitProvenTransactionRequest](#requests-SubmitProvenTransactionRequest) | [.responses.SubmitProvenTransactionResponse](#responses-SubmitProvenTransactionResponse) | Submits proven transaction to the Miden network. |

 



<a name="digest-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## digest.proto



<a name="digest-Digest"></a>

### Digest
A hash digest, the result of a hash function.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| d0 | [fixed64](#fixed64) |  |  |
| d1 | [fixed64](#fixed64) |  |  |
| d2 | [fixed64](#fixed64) |  |  |
| d3 | [fixed64](#fixed64) |  |  |





 

 

 

 



<a name="merkle-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## merkle.proto



<a name="merkle-MerklePath"></a>

### MerklePath
Represents a Merkle path


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| siblings | [digest.Digest](#digest-Digest) | repeated | List of sibling node hashes, in order from the root to the leaf |





 

 

 

 



<a name="mmr-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## mmr.proto



<a name="mmr-MmrDelta"></a>

### MmrDelta
Represents an MMR delta


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| forest | [uint64](#uint64) |  | The number of trees in the forest (latest block number &#43; 1) |
| data | [digest.Digest](#digest-Digest) | repeated | New and changed MMR peaks |





 

 

 

 



<a name="note-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## note.proto



<a name="note-Note"></a>

### Note
Represents a note


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| block_num | [fixed32](#fixed32) |  | The block number in which the note was created |
| note_index | [uint32](#uint32) |  | The index of the note in the block |
| note_id | [digest.Digest](#digest-Digest) |  | The ID of the note |
| metadata | [NoteMetadata](#note-NoteMetadata) |  | The note metadata |
| merkle_path | [merkle.MerklePath](#merkle-MerklePath) |  | The note inclusion proof in the block |
| details | [bytes](#bytes) | optional | This field will be present when the note is public. details contain the `Note` in a serialized format. |






<a name="note-NoteAuthenticationInfo"></a>

### NoteAuthenticationInfo
Represents proof of notes inclusion in the block(s) and block(s) inclusion in the chain


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| note_proofs | [NoteInclusionInBlockProof](#note-NoteInclusionInBlockProof) | repeated | Proof of each note&#39;s inclusion in a block. |
| block_proofs | [block.BlockInclusionProof](#block-BlockInclusionProof) | repeated | Proof of each block&#39;s inclusion in the chain. |






<a name="note-NoteInclusionInBlockProof"></a>

### NoteInclusionInBlockProof
Represents proof of a note&#39;s inclusion in a block


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| note_id | [digest.Digest](#digest-Digest) |  | The ID of the note |
| block_num | [fixed32](#fixed32) |  | The block number in which the note was created |
| note_index_in_block | [uint32](#uint32) |  | The index of the note in the block |
| merkle_path | [merkle.MerklePath](#merkle-MerklePath) |  | The note inclusion proof in the block |






<a name="note-NoteMetadata"></a>

### NoteMetadata
Represents a note metadata


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| sender | [account.AccountId](#account-AccountId) |  | The sender of the note |
| note_type | [uint32](#uint32) |  | The type of the note (0b01 = public, 0b10 = private, 0b11 = encrypted) |
| tag | [fixed32](#fixed32) |  | A value which can be used by the recipient(s) to identify notes intended for them |
| execution_hint | [fixed64](#fixed64) |  | Specifies when a note is ready to be consumed: * 6 least significant bits: Hint identifier (tag). * Bits 6 to 38: Hint payload.

See `miden_objects::notes::execution_hint` for more info. |
| aux | [fixed64](#fixed64) |  | An arbitrary user-defined value |






<a name="note-NoteSyncRecord"></a>

### NoteSyncRecord
Represents proof of a note inclusion in the block


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| note_index | [uint32](#uint32) |  | The index of the note |
| note_id | [digest.Digest](#digest-Digest) |  | The ID of the note |
| metadata | [NoteMetadata](#note-NoteMetadata) |  | The note metadata |
| merkle_path | [merkle.MerklePath](#merkle-MerklePath) |  | The note inclusion proof in the block |





 

 

 

 



<a name="requests-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## requests.proto



<a name="requests-ApplyBlockRequest"></a>

### ApplyBlockRequest
Applies changes of a new block to the DB and in-memory data structures.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| block | [bytes](#bytes) |  | Block data encoded using Miden&#39;s native format |






<a name="requests-CheckNullifiersByPrefixRequest"></a>

### CheckNullifiersByPrefixRequest
Returns a list of nullifiers that match the specified prefixes and are recorded in the node.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| prefix_len | [uint32](#uint32) |  | Number of bits used for nullifier prefix. Currently the only supported value is 16. |
| nullifiers | [uint32](#uint32) | repeated | List of nullifiers to check. Each nullifier is specified by its prefix with length equal to prefix_len |






<a name="requests-CheckNullifiersRequest"></a>

### CheckNullifiersRequest
Get a list of proofs for given nullifier hashes, each proof as a sparse Merkle Tree


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| nullifiers | [digest.Digest](#digest-Digest) | repeated | List of nullifiers to return proofs for |






<a name="requests-GetAccountDetailsRequest"></a>

### GetAccountDetailsRequest
Returns the latest state of an account with the specified ID.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| account_id | [account.AccountId](#account-AccountId) |  | Account ID to get details. |






<a name="requests-GetAccountProofsRequest"></a>

### GetAccountProofsRequest
Returns the latest state proofs of accounts with the specified IDs.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| account_ids | [account.AccountId](#account-AccountId) | repeated | List of account IDs to get states. |
| include_headers | [bool](#bool) | optional | Optional flag to include header and account code in the response. `false` by default. |
| code_commitments | [digest.Digest](#digest-Digest) | repeated | Account code commitments corresponding to the last-known `AccountCode` for requested accounts. Responses will include only the ones that are not known to the caller. These are not associated with a specific account but rather, they will be matched against all requested accounts. |






<a name="requests-GetAccountStateDeltaRequest"></a>

### GetAccountStateDeltaRequest
Returns delta of the account states in the range from `from_block_num` (exclusive) to
`to_block_num` (inclusive).


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| account_id | [account.AccountId](#account-AccountId) |  | ID of the account for which the delta is requested. |
| from_block_num | [fixed32](#fixed32) |  | Block number from which the delta is requested (exclusive). |
| to_block_num | [fixed32](#fixed32) |  | Block number up to which the delta is requested (inclusive). |






<a name="requests-GetBlockByNumberRequest"></a>

### GetBlockByNumberRequest
Retrieves block data by given block number.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| block_num | [fixed32](#fixed32) |  | The block number of the target block. |






<a name="requests-GetBlockHeaderByNumberRequest"></a>

### GetBlockHeaderByNumberRequest
Returns the block header corresponding to the requested block number, as well as the merkle
path and current forest which validate the block&#39;s inclusion in the chain.

The Merkle path is an MMR proof for the block&#39;s leaf, based on the current chain length.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| block_num | [uint32](#uint32) | optional | The block number of the target block.

If not provided, means latest known block. |
| include_mmr_proof | [bool](#bool) | optional | Whether or not to return authentication data for the block header. |






<a name="requests-GetBlockInputsRequest"></a>

### GetBlockInputsRequest
Returns data needed by the block producer to construct and prove the next block, including
account states, nullifiers, and unauthenticated notes.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| account_ids | [account.AccountId](#account-AccountId) | repeated | ID of the account against which a transaction is executed. |
| nullifiers | [digest.Digest](#digest-Digest) | repeated | Array of nullifiers for all notes consumed by a transaction. |
| unauthenticated_notes | [digest.Digest](#digest-Digest) | repeated | Array of note IDs to be checked for existence in the database. |






<a name="requests-GetNoteAuthenticationInfoRequest"></a>

### GetNoteAuthenticationInfoRequest
Returns a list of Note inclusion proofs for the specified Note IDs.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| note_ids | [digest.Digest](#digest-Digest) | repeated | List of NoteId&#39;s to be queried from the database |






<a name="requests-GetNotesByIdRequest"></a>

### GetNotesByIdRequest
Returns a list of notes matching the provided note IDs.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| note_ids | [digest.Digest](#digest-Digest) | repeated | List of NoteId&#39;s to be queried from the database |






<a name="requests-GetTransactionInputsRequest"></a>

### GetTransactionInputsRequest
Returns the data needed by the block producer to check validity of an incoming transaction.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| account_id | [account.AccountId](#account-AccountId) |  | ID of the account against which a transaction is executed. |
| nullifiers | [digest.Digest](#digest-Digest) | repeated | Array of nullifiers for all notes consumed by a transaction. |
| unauthenticated_notes | [digest.Digest](#digest-Digest) | repeated | Array of unauthenticated note IDs to be checked for existence in the database. |






<a name="requests-ListAccountsRequest"></a>

### ListAccountsRequest
Lists all accounts of the current chain.






<a name="requests-ListNotesRequest"></a>

### ListNotesRequest
Lists all notes of the current chain.






<a name="requests-ListNullifiersRequest"></a>

### ListNullifiersRequest
Lists all nullifiers of the current chain.






<a name="requests-SubmitProvenTransactionRequest"></a>

### SubmitProvenTransactionRequest
Submits proven transaction to the Miden network.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| transaction | [bytes](#bytes) |  | Transaction encoded using Miden&#39;s native format |






<a name="requests-SyncNoteRequest"></a>

### SyncNoteRequest
Note synchronization request.

Specifies note tags that client is interested in. The server will return the first block which
contains a note matching `note_tags` or the chain tip.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| block_num | [fixed32](#fixed32) |  | Last block known by the client. The response will contain data starting from the next block, until the first block which contains a note of matching the requested tag. |
| note_tags | [fixed32](#fixed32) | repeated | Specifies the tags which the client is interested in. |






<a name="requests-SyncStateRequest"></a>

### SyncStateRequest
State synchronization request.

Specifies state updates the client is interested in. The server will return the first block which
contains a note matching `note_tags` or the chain tip. And the corresponding updates to
`nullifiers` and `account_ids` for that block range.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| block_num | [fixed32](#fixed32) |  | Last block known by the client. The response will contain data starting from the next block, until the first block which contains a note of matching the requested tag, or the chain tip if there are no notes. |
| account_ids | [account.AccountId](#account-AccountId) | repeated | Accounts&#39; hash to include in the response.

An account hash will be included if-and-only-if it is the latest update. Meaning it is possible there was an update to the account for the given range, but if it is not the latest, it won&#39;t be included in the response. |
| note_tags | [fixed32](#fixed32) | repeated | Specifies the tags which the client is interested in. |
| nullifiers | [uint32](#uint32) | repeated | Determines the nullifiers the client is interested in by specifying the 16high bits of the target nullifier. |





 

 

 

 



<a name="responses-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## responses.proto



<a name="responses-AccountBlockInputRecord"></a>

### AccountBlockInputRecord
An account returned as a response to the `GetBlockInputs`


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| account_id | [account.AccountId](#account-AccountId) |  | The account ID |
| account_hash | [digest.Digest](#digest-Digest) |  | The latest account hash, zero hash if the account doesn&#39;t exist |
| proof | [merkle.MerklePath](#merkle-MerklePath) |  | Merkle path to verify the account&#39;s inclusion in the MMR |






<a name="responses-AccountProofsResponse"></a>

### AccountProofsResponse
A single account proof returned as a response to the `GetAccountProofs`


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| account_id | [account.AccountId](#account-AccountId) |  | Account ID |
| account_hash | [digest.Digest](#digest-Digest) |  | Account hash |
| account_proof | [merkle.MerklePath](#merkle-MerklePath) |  | Authentication path from the `account_root` of the block header to the account |
| state_header | [AccountStateHeader](#responses-AccountStateHeader) | optional | State header for public accounts. Filled only if `include_headers` flag is set to `true`. |






<a name="responses-AccountStateHeader"></a>

### AccountStateHeader
State header for public accounts


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| header | [account.AccountHeader](#account-AccountHeader) |  | Account header |
| storage_header | [bytes](#bytes) |  | Values of all account storage slots (max 255) |
| account_code | [bytes](#bytes) | optional | Account code, returned only when none of the request&#39;s code commitments match with the current one |






<a name="responses-AccountTransactionInputRecord"></a>

### AccountTransactionInputRecord
An account returned as a response to the `GetTransactionInputs`


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| account_id | [account.AccountId](#account-AccountId) |  | The account ID |
| account_hash | [digest.Digest](#digest-Digest) |  | The latest account hash, zero hash if the account doesn&#39;t exist |






<a name="responses-ApplyBlockResponse"></a>

### ApplyBlockResponse
Represents the result of applying a block






<a name="responses-CheckNullifiersByPrefixResponse"></a>

### CheckNullifiersByPrefixResponse
Represents the result of checking nullifiers by prefix


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| nullifiers | [NullifierUpdate](#responses-NullifierUpdate) | repeated | List of nullifiers matching the prefixes specified in the request |






<a name="responses-CheckNullifiersResponse"></a>

### CheckNullifiersResponse
Represents the result of checking nullifiers


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| proofs | [smt.SmtOpening](#smt-SmtOpening) | repeated | Each requested nullifier has its corresponding nullifier proof at the same position |






<a name="responses-GetAccountDetailsResponse"></a>

### GetAccountDetailsResponse
Represents the result of getting account details


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| details | [account.AccountInfo](#account-AccountInfo) |  | Account info (with details for public accounts) |






<a name="responses-GetAccountProofsResponse"></a>

### GetAccountProofsResponse
Represents the result of getting account proofs


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| block_num | [fixed32](#fixed32) |  | Block number at which the state of the account was returned |
| account_proofs | [AccountProofsResponse](#responses-AccountProofsResponse) | repeated | List of account state infos for the requested account keys |






<a name="responses-GetAccountStateDeltaResponse"></a>

### GetAccountStateDeltaResponse
Represents the result of getting account state delta


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| delta | [bytes](#bytes) | optional | The calculated `AccountStateDelta` encoded using Miden native format |






<a name="responses-GetBlockByNumberResponse"></a>

### GetBlockByNumberResponse
Represents the result of getting block by number


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| block | [bytes](#bytes) | optional | The requested `Block` data encoded using Miden native format |






<a name="responses-GetBlockHeaderByNumberResponse"></a>

### GetBlockHeaderByNumberResponse
Represents the result of getting a block header by block number


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| block_header | [block.BlockHeader](#block-BlockHeader) |  | The requested block header |
| mmr_path | [merkle.MerklePath](#merkle-MerklePath) | optional | Merkle path to verify the block&#39;s inclusion in the MMR at the returned `chain_length` |
| chain_length | [fixed32](#fixed32) | optional | Current chain length |






<a name="responses-GetBlockInputsResponse"></a>

### GetBlockInputsResponse
Represents the result of getting block inputs


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| block_header | [block.BlockHeader](#block-BlockHeader) |  | The latest block header |
| mmr_peaks | [digest.Digest](#digest-Digest) | repeated | Peaks of the above block&#39;s mmr, The `forest` value is equal to the block number |
| account_states | [AccountBlockInputRecord](#responses-AccountBlockInputRecord) | repeated | The hashes of the requested accounts and their authentication paths |
| nullifiers | [NullifierBlockInputRecord](#responses-NullifierBlockInputRecord) | repeated | The requested nullifiers and their authentication paths |
| found_unauthenticated_notes | [note.NoteAuthenticationInfo](#note-NoteAuthenticationInfo) |  | The list of requested notes which were found in the database |






<a name="responses-GetNoteAuthenticationInfoResponse"></a>

### GetNoteAuthenticationInfoResponse
Represents the result of getting note authentication info


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| proofs | [note.NoteAuthenticationInfo](#note-NoteAuthenticationInfo) |  | Proofs of note inclusions in blocks and block inclusions in chain |






<a name="responses-GetNotesByIdResponse"></a>

### GetNotesByIdResponse
Represents the result of getting notes by IDs


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| notes | [note.Note](#note-Note) | repeated | Lists Note&#39;s returned by the database |






<a name="responses-GetTransactionInputsResponse"></a>

### GetTransactionInputsResponse
Represents the result of getting transaction inputs


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| account_state | [AccountTransactionInputRecord](#responses-AccountTransactionInputRecord) |  | Account state proof |
| nullifiers | [NullifierTransactionInputRecord](#responses-NullifierTransactionInputRecord) | repeated | List of nullifiers that have been consumed |
| missing_unauthenticated_notes | [digest.Digest](#digest-Digest) | repeated | List of unauthenticated notes that were not found in the database |
| block_height | [fixed32](#fixed32) |  | The node&#39;s current block height |






<a name="responses-ListAccountsResponse"></a>

### ListAccountsResponse
Represents the result of getting accounts list


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| accounts | [account.AccountInfo](#account-AccountInfo) | repeated | Lists all accounts of the current chain |






<a name="responses-ListNotesResponse"></a>

### ListNotesResponse
Represents the result of getting notes list


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| notes | [note.Note](#note-Note) | repeated | Lists all notes of the current chain |






<a name="responses-ListNullifiersResponse"></a>

### ListNullifiersResponse
Represents the result of getting nullifiers list


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| nullifiers | [smt.SmtLeafEntry](#smt-SmtLeafEntry) | repeated | Lists all nullifiers of the current chain |






<a name="responses-NullifierBlockInputRecord"></a>

### NullifierBlockInputRecord
A nullifier returned as a response to the `GetBlockInputs`


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| nullifier | [digest.Digest](#digest-Digest) |  | The nullifier ID |
| opening | [smt.SmtOpening](#smt-SmtOpening) |  | Merkle path to verify the nullifier&#39;s inclusion in the MMR |






<a name="responses-NullifierTransactionInputRecord"></a>

### NullifierTransactionInputRecord
A nullifier returned as a response to the `GetTransactionInputs`


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| nullifier | [digest.Digest](#digest-Digest) |  | The nullifier ID |
| block_num | [fixed32](#fixed32) |  | The block at which the nullifier has been consumed, zero if not consumed |






<a name="responses-NullifierUpdate"></a>

### NullifierUpdate
Represents a single nullifier update


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| nullifier | [digest.Digest](#digest-Digest) |  | Nullifier ID |
| block_num | [fixed32](#fixed32) |  | Block number |






<a name="responses-SubmitProvenTransactionResponse"></a>

### SubmitProvenTransactionResponse
Represents the result of submitting proven transaction


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| block_height | [fixed32](#fixed32) |  | The node&#39;s current block height |






<a name="responses-SyncNoteResponse"></a>

### SyncNoteResponse
Represents the result of syncing notes request


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| chain_tip | [fixed32](#fixed32) |  | Number of the latest block in the chain |
| block_header | [block.BlockHeader](#block-BlockHeader) |  | Block header of the block with the first note matching the specified criteria |
| mmr_path | [merkle.MerklePath](#merkle-MerklePath) |  | Merkle path to verify the block&#39;s inclusion in the MMR at the returned `chain_tip`.

An MMR proof can be constructed for the leaf of index `block_header.block_num` of an MMR of forest `chain_tip` with this path. |
| notes | [note.NoteSyncRecord](#note-NoteSyncRecord) | repeated | List of all notes together with the Merkle paths from `response.block_header.note_root` |






<a name="responses-SyncStateResponse"></a>

### SyncStateResponse
Represents the result of syncing state request


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| chain_tip | [fixed32](#fixed32) |  | Number of the latest block in the chain |
| block_header | [block.BlockHeader](#block-BlockHeader) |  | Block header of the block with the first note matching the specified criteria |
| mmr_delta | [mmr.MmrDelta](#mmr-MmrDelta) |  | Data needed to update the partial MMR from `request.block_num &#43; 1` to `response.block_header.block_num` |
| accounts | [account.AccountSummary](#account-AccountSummary) | repeated | List of account hashes updated after `request.block_num &#43; 1` but not after `response.block_header.block_num` |
| transactions | [transaction.TransactionSummary](#transaction-TransactionSummary) | repeated | List of transactions executed against requested accounts between `request.block_num &#43; 1` and `response.block_header.block_num` |
| notes | [note.NoteSyncRecord](#note-NoteSyncRecord) | repeated | List of all notes together with the Merkle paths from `response.block_header.note_root` |
| nullifiers | [NullifierUpdate](#responses-NullifierUpdate) | repeated | List of nullifiers created between `request.block_num &#43; 1` and `response.block_header.block_num` |





 

 

 

 



<a name="rpc-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## rpc.proto
Specification of the user facing gRPC API.

 

 

 


<a name="rpc-Api"></a>

### Api


| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| CheckNullifiers | [.requests.CheckNullifiersRequest](#requests-CheckNullifiersRequest) | [.responses.CheckNullifiersResponse](#responses-CheckNullifiersResponse) | Get a list of proofs for given nullifier hashes, each proof as a sparse Merkle Tree |
| CheckNullifiersByPrefix | [.requests.CheckNullifiersByPrefixRequest](#requests-CheckNullifiersByPrefixRequest) | [.responses.CheckNullifiersByPrefixResponse](#responses-CheckNullifiersByPrefixResponse) | Returns a list of nullifiers that match the specified prefixes and are recorded in the node. |
| GetAccountDetails | [.requests.GetAccountDetailsRequest](#requests-GetAccountDetailsRequest) | [.responses.GetAccountDetailsResponse](#responses-GetAccountDetailsResponse) | Returns the latest state of an account with the specified ID. |
| GetAccountProofs | [.requests.GetAccountProofsRequest](#requests-GetAccountProofsRequest) | [.responses.GetAccountProofsResponse](#responses-GetAccountProofsResponse) | Returns the latest state proofs of accounts with the specified IDs. |
| GetAccountStateDelta | [.requests.GetAccountStateDeltaRequest](#requests-GetAccountStateDeltaRequest) | [.responses.GetAccountStateDeltaResponse](#responses-GetAccountStateDeltaResponse) | Returns delta of the account states in the range from `from_block_num` (exclusive) to `to_block_num` (inclusive). |
| GetBlockByNumber | [.requests.GetBlockByNumberRequest](#requests-GetBlockByNumberRequest) | [.responses.GetBlockByNumberResponse](#responses-GetBlockByNumberResponse) | Retrieves block data by given block number. |
| GetBlockHeaderByNumber | [.requests.GetBlockHeaderByNumberRequest](#requests-GetBlockHeaderByNumberRequest) | [.responses.GetBlockHeaderByNumberResponse](#responses-GetBlockHeaderByNumberResponse) | Retrieves block header by given block number. Optionally, it also returns the MMR path and current chain length to authenticate the block&#39;s inclusion. |
| GetNotesById | [.requests.GetNotesByIdRequest](#requests-GetNotesByIdRequest) | [.responses.GetNotesByIdResponse](#responses-GetNotesByIdResponse) | Returns a list of notes matching the provided note IDs. |
| SubmitProvenTransaction | [.requests.SubmitProvenTransactionRequest](#requests-SubmitProvenTransactionRequest) | [.responses.SubmitProvenTransactionResponse](#responses-SubmitProvenTransactionResponse) | Submits proven transaction to the Miden network. |
| SyncNotes | [.requests.SyncNoteRequest](#requests-SyncNoteRequest) | [.responses.SyncNoteResponse](#responses-SyncNoteResponse) | Note synchronization request.

Specifies note tags that client is interested in. The server will return the first block which contains a note matching `note_tags` or the chain tip. |
| SyncState | [.requests.SyncStateRequest](#requests-SyncStateRequest) | [.responses.SyncStateResponse](#responses-SyncStateResponse) | Returns info which can be used by the client to sync up to the latest state of the chain for the objects (accounts, notes, nullifiers) the client is interested in.

This request returns the next block containing requested data. It also returns `chain_tip` which is the latest block number in the chain. Client is expected to repeat these requests in a loop until `response.block_header.block_num == response.chain_tip`, at which point the client is fully synchronized with the chain.

Each request also returns info about new notes, nullifiers etc. created. It also returns Chain MMR delta that can be used to update the state of Chain MMR. This includes both chain MMR peaks and chain MMR nodes.

For preserving some degree of privacy, note tags and nullifiers filters contain only high part of hashes. Thus, returned data contains excessive notes and nullifiers, client can make additional filtering of that data on its side. |

 



<a name="smt-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## smt.proto



<a name="smt-SmtLeaf"></a>

### SmtLeaf
A leaf in an SMT, sitting at depth 64. A leaf can contain 0, 1 or multiple leaf entries


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| empty | [uint64](#uint64) |  | An empty leaf |
| single | [SmtLeafEntry](#smt-SmtLeafEntry) |  | A single leaf entry |
| multiple | [SmtLeafEntries](#smt-SmtLeafEntries) |  | Multiple leaf entries |






<a name="smt-SmtLeafEntries"></a>

### SmtLeafEntries
Represents multiple leaf entries in an SMT


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| entries | [SmtLeafEntry](#smt-SmtLeafEntry) | repeated | The entries list |






<a name="smt-SmtLeafEntry"></a>

### SmtLeafEntry
Represents a single SMT leaf entry


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| key | [digest.Digest](#digest-Digest) |  | The key of the entry |
| value | [digest.Digest](#digest-Digest) |  | The value of the entry |






<a name="smt-SmtOpening"></a>

### SmtOpening
The opening of a leaf in an SMT


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| path | [merkle.MerklePath](#merkle-MerklePath) |  | The merkle path to the leaf |
| leaf | [SmtLeaf](#smt-SmtLeaf) |  | The leaf itself |





 

 

 

 



<a name="store-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## store.proto
Specification of the store RPC.

This provided access to the rollup data to the other nodes.

 

 

 


<a name="store-Api"></a>

### Api


| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| ApplyBlock | [.requests.ApplyBlockRequest](#requests-ApplyBlockRequest) | [.responses.ApplyBlockResponse](#responses-ApplyBlockResponse) | Applies changes of a new block to the DB and in-memory data structures. |
| CheckNullifiers | [.requests.CheckNullifiersRequest](#requests-CheckNullifiersRequest) | [.responses.CheckNullifiersResponse](#responses-CheckNullifiersResponse) | Get a list of proofs for given nullifier hashes, each proof as a sparse Merkle Tree |
| CheckNullifiersByPrefix | [.requests.CheckNullifiersByPrefixRequest](#requests-CheckNullifiersByPrefixRequest) | [.responses.CheckNullifiersByPrefixResponse](#responses-CheckNullifiersByPrefixResponse) | Returns a list of nullifiers that match the specified prefixes and are recorded in the node. |
| GetAccountDetails | [.requests.GetAccountDetailsRequest](#requests-GetAccountDetailsRequest) | [.responses.GetAccountDetailsResponse](#responses-GetAccountDetailsResponse) | Returns the latest state of an account with the specified ID. |
| GetAccountProofs | [.requests.GetAccountProofsRequest](#requests-GetAccountProofsRequest) | [.responses.GetAccountProofsResponse](#responses-GetAccountProofsResponse) | Returns the latest state proofs of accounts with the specified IDs. |
| GetAccountStateDelta | [.requests.GetAccountStateDeltaRequest](#requests-GetAccountStateDeltaRequest) | [.responses.GetAccountStateDeltaResponse](#responses-GetAccountStateDeltaResponse) | Returns delta of the account states in the range from `from_block_num` (exclusive) to `to_block_num` (inclusive). |
| GetBlockByNumber | [.requests.GetBlockByNumberRequest](#requests-GetBlockByNumberRequest) | [.responses.GetBlockByNumberResponse](#responses-GetBlockByNumberResponse) | Retrieves block data by given block number. |
| GetBlockHeaderByNumber | [.requests.GetBlockHeaderByNumberRequest](#requests-GetBlockHeaderByNumberRequest) | [.responses.GetBlockHeaderByNumberResponse](#responses-GetBlockHeaderByNumberResponse) | Retrieves block header by given block number. Optionally, it also returns the MMR path and current chain length to authenticate the block&#39;s inclusion. |
| GetBlockInputs | [.requests.GetBlockInputsRequest](#requests-GetBlockInputsRequest) | [.responses.GetBlockInputsResponse](#responses-GetBlockInputsResponse) | Returns data needed by the block producer to construct and prove the next block, including account states, nullifiers, and unauthenticated notes. |
| GetNoteAuthenticationInfo | [.requests.GetNoteAuthenticationInfoRequest](#requests-GetNoteAuthenticationInfoRequest) | [.responses.GetNoteAuthenticationInfoResponse](#responses-GetNoteAuthenticationInfoResponse) | Returns a list of Note inclusion proofs for the specified Note IDs. |
| GetNotesById | [.requests.GetNotesByIdRequest](#requests-GetNotesByIdRequest) | [.responses.GetNotesByIdResponse](#responses-GetNotesByIdResponse) | Returns a list of notes matching the provided note IDs. |
| GetTransactionInputs | [.requests.GetTransactionInputsRequest](#requests-GetTransactionInputsRequest) | [.responses.GetTransactionInputsResponse](#responses-GetTransactionInputsResponse) | Returns the data needed by the block producer to check validity of an incoming transaction. |
| ListAccounts | [.requests.ListAccountsRequest](#requests-ListAccountsRequest) | [.responses.ListAccountsResponse](#responses-ListAccountsResponse) | Lists all accounts of the current chain. |
| ListNotes | [.requests.ListNotesRequest](#requests-ListNotesRequest) | [.responses.ListNotesResponse](#responses-ListNotesResponse) | Lists all notes of the current chain. |
| ListNullifiers | [.requests.ListNullifiersRequest](#requests-ListNullifiersRequest) | [.responses.ListNullifiersResponse](#responses-ListNullifiersResponse) | Lists all nullifiers of the current chain. |
| SyncNotes | [.requests.SyncNoteRequest](#requests-SyncNoteRequest) | [.responses.SyncNoteResponse](#responses-SyncNoteResponse) | Note synchronization request.

Specifies note tags that client is interested in. The server will return the first block which contains a note matching `note_tags` or the chain tip. |
| SyncState | [.requests.SyncStateRequest](#requests-SyncStateRequest) | [.responses.SyncStateResponse](#responses-SyncStateResponse) | Returns info which can be used by the client to sync up to the latest state of the chain for the objects (accounts, notes, nullifiers) the client is interested in.

This request returns the next block containing requested data. It also returns `chain_tip` which is the latest block number in the chain. Client is expected to repeat these requests in a loop until `response.block_header.block_num == response.chain_tip`, at which point the client is fully synchronized with the chain.

Each request also returns info about new notes, nullifiers etc. created. It also returns Chain MMR delta that can be used to update the state of Chain MMR. This includes both chain MMR peaks and chain MMR nodes.

For preserving some degree of privacy, note tags and nullifiers filters contain only high part of hashes. Thus, returned data contains excessive notes and nullifiers, client can make additional filtering of that data on its side. |

 



<a name="transaction-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## transaction.proto



<a name="transaction-TransactionId"></a>

### TransactionId
Represents a transaction ID


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| id | [digest.Digest](#digest-Digest) |  | The transaction ID |






<a name="transaction-TransactionSummary"></a>

### TransactionSummary
Represents a transaction summary


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| transaction_id | [TransactionId](#transaction-TransactionId) |  | The transaction ID |
| block_num | [fixed32](#fixed32) |  | The block number |
| account_id | [account.AccountId](#account-AccountId) |  | The account ID |





 

 

 

 



## Scalar Value Types

| .proto Type | Notes | C++ | Java | Python | Go | C# | PHP | Ruby |
| ----------- | ----- | --- | ---- | ------ | -- | -- | --- | ---- |
| <a name="double" /> double |  | double | double | float | float64 | double | float | Float |
| <a name="float" /> float |  | float | float | float | float32 | float | float | Float |
| <a name="int32" /> int32 | Uses variable-length encoding. Inefficient for encoding negative numbers – if your field is likely to have negative values, use sint32 instead. | int32 | int | int | int32 | int | integer | Bignum or Fixnum (as required) |
| <a name="int64" /> int64 | Uses variable-length encoding. Inefficient for encoding negative numbers – if your field is likely to have negative values, use sint64 instead. | int64 | long | int/long | int64 | long | integer/string | Bignum |
| <a name="uint32" /> uint32 | Uses variable-length encoding. | uint32 | int | int/long | uint32 | uint | integer | Bignum or Fixnum (as required) |
| <a name="uint64" /> uint64 | Uses variable-length encoding. | uint64 | long | int/long | uint64 | ulong | integer/string | Bignum or Fixnum (as required) |
| <a name="sint32" /> sint32 | Uses variable-length encoding. Signed int value. These more efficiently encode negative numbers than regular int32s. | int32 | int | int | int32 | int | integer | Bignum or Fixnum (as required) |
| <a name="sint64" /> sint64 | Uses variable-length encoding. Signed int value. These more efficiently encode negative numbers than regular int64s. | int64 | long | int/long | int64 | long | integer/string | Bignum |
| <a name="fixed32" /> fixed32 | Always four bytes. More efficient than uint32 if values are often greater than 2^28. | uint32 | int | int | uint32 | uint | integer | Bignum or Fixnum (as required) |
| <a name="fixed64" /> fixed64 | Always eight bytes. More efficient than uint64 if values are often greater than 2^56. | uint64 | long | int/long | uint64 | ulong | integer/string | Bignum |
| <a name="sfixed32" /> sfixed32 | Always four bytes. | int32 | int | int | int32 | int | integer | Bignum or Fixnum (as required) |
| <a name="sfixed64" /> sfixed64 | Always eight bytes. | int64 | long | int/long | int64 | long | integer/string | Bignum |
| <a name="bool" /> bool |  | bool | boolean | boolean | bool | bool | boolean | TrueClass/FalseClass |
| <a name="string" /> string | A string must always contain UTF-8 encoded or 7-bit ASCII text. | string | String | str/unicode | string | string | string | String (UTF-8) |
| <a name="bytes" /> bytes | May contain any arbitrary sequence of bytes. | string | ByteString | str | []byte | ByteString | string | String (ASCII-8BIT) |

