# Changelog

## 0.4.0 (2024-07-04)

### Features

* Changed sync endpoint to return a list of committed transactions (#377).
* Added `aux` column to notes table (#384).
* Changed state sync endpoint to return a list of `TransactionSummary` objects instead of just transaction IDs (#386).
* Added support for unauthenticated transaction notes (#390).

### Enhancements

* Standardized CI and Makefile across Miden repositories (#367)
* Removed client dependency from faucet (#368).
* Fixed faucet note script so that it uses the `aux` input (#387).
* Added crate to distribute node RPC protobuf files (#391).
* Add `init` command for node and faucet (#392).


## 0.3.0 (2024-05-15)

* Added option to mint pulic notes in the faucet (#339).
* Renamed `note_hash` into `note_id` in the database (#336)
* Changed `version` and `timestamp` fields in `Block` message to `u32` (#337).
* [BREAKING] Implemented `NoteMetadata` protobuf message (#338).
* Added `GetBlockByNumber` endpoint (#340).
* Added block authentication data to the `GetBlockHeaderByNumber` RPC (#345).
* Enabled support for HTTP/1.1 requests for the RPC component (#352).

## 0.2.1 (2024-04-27)

* Combined node components into a single binary (#323).

## 0.2.0 (2024-04-11)

* Implemented Docker-based node deployment (#257).
* Improved build process (#267, #272, #278).
* Implemented Nullifier tree wrapper (#275).
* [BREAKING] Added support for public accounts (#287, #293, #294).
* [BREAKING] Added support for public notes (#300, #310).
* Added `GetNotesById` endpoint (#298).
* Implemented amd64 debian packager (#312).

## 0.1.0 (2024-03-11)

* Initial release.
