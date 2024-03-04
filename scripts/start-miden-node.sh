#!/bin/bash
miden-node make-genesis --inputs-path genesis.toml
miden-node start --config miden-node.toml
