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
RUN cargo install --path block-producer

# Run Miden node
FROM debian:bookworm-slim

# Update machine & install required packages
# The instalation of sqlite3 is needed for correct function of the SQLite database
RUN apt-get update && \
    apt-get -y upgrade && \
    rm -rf /var/lib/apt/lists/*

# Copy artifacts from the builder stage
COPY --from=builder /usr/local/cargo/bin/miden-node-block-producer /usr/local/bin/miden-node-block-producer

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
EXPOSE 48046

# Start the Miden node
# Miden node does not spawn sub-processes, so it can be used as the PID1
CMD miden-node-block-producer serve
