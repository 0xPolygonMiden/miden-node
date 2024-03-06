# Miden node Dockerfile

# Setup image builder
FROM rust:1.76-slim-bookworm AS builder

# Install dependencies
RUN apt-get update && \
    apt-get -y upgrade && \
    apt-get install -y llvm clang bindgen pkg-config libssl-dev libsqlite3-dev && \
    rm -rf /var/lib/apt/lists/*

# Copy source code
WORKDIR /app
COPY . .

# Cache Cargo dependencies
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo fetch --manifest-path node/Cargo.toml

# Build the node crate
RUN cargo install --features testing --path node
RUN miden-node make-genesis --inputs-path node/genesis.toml

# Run Miden node
FROM debian:bookworm-slim

# Install required packages
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    libssl-dev \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy artifacts from the builder stage
COPY --from=builder /app/node/miden-node.toml miden-node.toml
COPY --from=builder /app/genesis.dat genesis.dat
COPY --from=builder /app/accounts accounts
COPY --from=builder /usr/local/cargo/bin/miden-node /usr/local/bin/miden-node

# Expose RPC port
EXPOSE 57291

# Start the Miden node
CMD miden-node start --config miden-node.toml
