#!/bin/bash

grpcurl -plaintext -proto rpc.proto -import-path proto/proto/ localhost:57291 rpc.Api/GetBlockHeaderByNumber > response.json

if grep -q "blockHeader" response.json; then
    echo "gRPC test passed."
    exit 0
else
    echo "gRPC test failed."
    exit 1
fi
