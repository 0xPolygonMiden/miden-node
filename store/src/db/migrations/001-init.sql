CREATE TABLE
    block_headers
(
    block_num    INTEGER NOT NULL,
    block_header BLOB    NOT NULL,

    PRIMARY KEY (block_num),
    CONSTRAINT block_header_block_num_is_u32 CHECK (block_num BETWEEN 0 AND 0xFFFFFFFF)
) STRICT, WITHOUT ROWID;

CREATE TABLE
    notes
(
    block_num   INTEGER NOT NULL,
    batch_index INTEGER NOT NULL, -- Index of batch in block, starting from 0
    note_index  INTEGER NOT NULL, -- Index of note in batch, starting from 0
    note_hash   BLOB    NOT NULL,
    sender      INTEGER NOT NULL,
    tag         INTEGER NOT NULL,
    merkle_path BLOB    NOT NULL,
    details     BLOB,

    PRIMARY KEY (block_num, batch_index, note_index),
    CONSTRAINT fk_block_num FOREIGN KEY (block_num) REFERENCES block_headers (block_num),
    CONSTRAINT notes_block_num_is_u32 CHECK (block_num BETWEEN 0 AND 0xFFFFFFFF),
    CONSTRAINT notes_batch_index_is_u32 CHECK (batch_index BETWEEN 0 AND 0xFFFFFFFF),
    CONSTRAINT notes_note_index_is_u32 CHECK (note_index BETWEEN 0 AND 0xFFFFFFFF)
) STRICT, WITHOUT ROWID;

CREATE TABLE
    accounts
(
    account_id   INTEGER NOT NULL,
    account_hash BLOB    NOT NULL,
    block_num    INTEGER NOT NULL,
    details      BLOB,

    PRIMARY KEY (account_id),
    CONSTRAINT fk_block_num FOREIGN KEY (block_num) REFERENCES block_headers (block_num),
    CONSTRAINT accounts_block_num_is_u32 CHECK (block_num BETWEEN 0 AND 0xFFFFFFFF)
) STRICT, WITHOUT ROWID;

CREATE TABLE
    nullifiers
(
    nullifier        BLOB    NOT NULL,
    nullifier_prefix INTEGER NOT NULL,
    block_num        INTEGER NOT NULL,

    PRIMARY KEY (nullifier),
    CONSTRAINT fk_block_num FOREIGN KEY (block_num) REFERENCES block_headers (block_num),
    CONSTRAINT nullifiers_nullifier_is_digest CHECK (length(nullifier) = 32),
    CONSTRAINT nullifiers_nullifier_prefix_is_u16 CHECK (nullifier_prefix BETWEEN 0 AND 0xFFFF),
    CONSTRAINT nullifiers_block_num_is_u32 CHECK (block_num BETWEEN 0 AND 0xFFFFFFFF)
) STRICT, WITHOUT ROWID;
