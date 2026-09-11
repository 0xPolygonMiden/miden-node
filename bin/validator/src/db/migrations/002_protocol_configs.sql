CREATE TABLE protocol_configs (
    commitment      BLOB PRIMARY KEY,
    protocol_config BLOB NOT NULL,
    CHECK (length(commitment) = 32)
) WITHOUT ROWID;
