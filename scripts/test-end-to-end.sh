#!/bin/bash

# Create miden-client.toml file
# Needed for the Miden client to work properly
{
    echo "[rpc]"
    echo "endpoint = { protocol = \"http\", host = \"localhost\", port = 57291 }"
    echo ""
    echo "[store]"
    echo "database_filepath = \"store.sqlite3\""
} > miden-client.toml

# Test flow 1

# Create accounts
miden-client account new basic-immutable
miden-client account new fungible-faucet --token-symbol JACK --decimals 3 --max-supply 1000

# Capture IDs
basic_account=$(miden-client account -l | grep -oE ' 0x[a-fA-F0-9]{16} ' | head -n 1)
faucet_account=$(miden-client account -l | grep -oE ' 0x[a-fA-F0-9]{16} ' | head -n 2 | tail -n 1)

# Sync client and create tx between faucet and account
miden-client sync
miden-client tx new mint $basic_account $faucet_account 777
sleep 15
miden-client sync

# Extract note ID
note_id=$(miden-client input-notes -l | grep -oE ' 0x[a-fA-F0-9]{64} ' | head -n 1)

# Basic account consumes faucet generated note
miden-client tx new consume-notes $basic_account $note_id

# Check that the funds have been received successfully by the account
if miden-client account show $basic_account -v | grep -q ' 777 '; then
    echo "Funds successfully received."
else
    echo "Failed to receive funds from faucet"
    exit 1
fi

echo "Test flow 1 - success ✅"
