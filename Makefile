FEATURES=testing

install:
	cargo install --features=${FEATURES} --path node

run:
	miden-node make-genesis --inputs-path node/genesis.toml
	miden-node start --config node/miden-node.toml
