# Miden node Dockerfile

# Setup image builder
FROM rust:1.76-slim-bookworm AS builder

# Set labels
LABEL org.opencontainers.image.authors=miden@polygon.io \
      org.opencontainers.image.url=https://0xpolygonmiden.github.io/ \
      org.opencontainers.image.documentation=https://github.com/0xPolygonMiden/miden-node \
      org.opencontainers.image.source=https://github.com/0xPolygonMiden/miden-node \
      org.opencontainers.image.vendor=Polygon \
      org.opencontainers.image.licenses=MIT

ARG CREATED
ARG VERSION
ARG COMMIT
LABEL org.opencontainers.image.created=$CREATED \
      org.opencontainers.image.version=$VERSION \
      org.opencontainers.image.revision=$COMMIT

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
    apt-get -y upgrade && \
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
# Miden node does not spawn sub-processes, so it can be used as the PID1
CMD miden-node start --config miden-node.toml
