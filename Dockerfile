# Miden-node Dockerfile

#### Setup image builder
FROM rust:1.75.0-bullseye AS builder

# Install dependencies
RUN apt-get update && apt-get -y upgrade
RUN apt-get install -y gcc llvm clang bindgen pkg-config

# Setup workdir
WORKDIR /app
COPY . miden-node
RUN cd miden-node && make

### Run Miden-Node
FROM ubuntu:22.04
RUN apt-get update && apt-get -y upgrade && apt-get install -y make libssl-dev libsqlite3-dev curl
RUN /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
RUN source .bashrc
RUN brew install grpcurl
COPY --from=builder /app/miden-node/scripts/start-miden-node.sh start-miden-node.sh
COPY --from=builder /app/miden-node/node/miden-node.toml miden-node.toml
COPY --from=builder /app/miden-node/node/genesis.toml genesis.toml
COPY --from=builder /usr/local/cargo/bin/miden-node /usr/local/bin/miden-node
RUN chmod +x /start-miden-node.sh
EXPOSE 57291
CMD [ "/start-miden-node.sh" ]
