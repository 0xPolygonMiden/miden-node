# Changelog

## 0.3.0 (TBD)

* Renamed `note_hash` into `note_id` in the database (#336)
* Changed `version` and `timestamp` fields in `Block` message to `u32` (#337).
* [BREAKING] Implemented `NoteMetadata` protobuf message (#338).

## 0.2.1 (2024-04-27)

* Added option to mint pulic notes in the faucet (#339).
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
