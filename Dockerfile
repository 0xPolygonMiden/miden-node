# Miden-node Dockerfile

# Setup image builder
FROM rust:1.75.0-bullseye AS builder

# Install dependencies
RUN apt-get update && apt-get -y upgrade && apt-get install -y gcc llvm clang bindgen pkg-config

# Setup workdir
WORKDIR /app
COPY . miden-node
RUN cd miden-node && make
RUN miden-node make-genesis --inputs-path node/genesis.toml

# Run Miden-Node
FROM ubuntu:22.04
RUN apt-get update && apt-get -y upgrade && apt-get install -y make libssl-dev libsqlite3-dev curl
COPY --from=builder /app/miden-node/node/miden-node.toml miden-node.toml
COPY --from=builder /app/miden-node/genesis.dat genesis.dat
COPY --from=builder /app/miden-node/accounts accounts
COPY --from=builder /usr/local/cargo/bin/miden-node /usr/local/bin/miden-node
EXPOSE 57291
CMD ["miden-node start --config miden-node.toml"]
