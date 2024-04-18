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

# Build the node crate
RUN cargo install --features testing --path bin/node
RUN miden-node make-genesis --inputs-path bin/node/genesis.toml

# Run Miden node
FROM debian:bookworm-slim

# Update machine & install required packages
# The instalation of sqlite3 is needed for correct function of the SQLite database
RUN apt-get update && \
    apt-get -y upgrade && \
    apt-get install -y --no-install-recommends \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/*

# Copy artifacts from the builder stage
COPY --from=builder /app/genesis.dat genesis.dat
COPY --from=builder /app/accounts accounts
COPY --from=builder /usr/local/cargo/bin/miden-node /usr/local/bin/miden-node

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

# Expose RPC port
EXPOSE 57291

# Start the Miden node
# Miden node does not spawn sub-processes, so it can be used as the PID1
CMD miden-node start --config miden-node.toml node
