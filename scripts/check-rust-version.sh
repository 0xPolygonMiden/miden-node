#!/bin/bash

# Define the expected Rust version
EXPECTED_VERSION="1.75"

# Check rust-toolchain file
TOOLCHAIN_VERSION=$(cat rust-toolchain)
if [ "$TOOLCHAIN_VERSION" != "$EXPECTED_VERSION" ]; then
    echo "Mismatch in rust-toolchain. Expected $EXPECTED_VERSION, found $TOOLCHAIN_VERSION"
    exit 1
fi

# Check each Cargo.toml file
for file in $(find . -name Cargo.toml); do
    CARGO_VERSION=$(grep "rust-version" $file | cut -d '"' -f 2)
    if [ "$CARGO_VERSION" != "$EXPECTED_VERSION" ]; then
        echo "Mismatch in $file. Expected $EXPECTED_VERSION, found $CARGO_VERSION"
        exit 1
    fi
done

echo "All rust versions match ✅"
