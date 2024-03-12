#!/bin/bash

# Check rust-toolchain file
TOOLCHAIN_VERSION=$(cat rust-toolchain)

# Check each Cargo.toml file
CARGO_VERSION=$(cat Cargo.toml | grep "rust-version" | cut -d '"' -f 2)
if [ "$CARGO_VERSION" != "$TOOLCHAIN_VERSION" ]; then
    echo "Mismatch in $file. Expected $TOOLCHAIN_VERSION, found $CARGO_VERSION"
    exit 1
fi

echo "Rust versions match ✅"
