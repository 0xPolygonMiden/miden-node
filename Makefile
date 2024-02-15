FEATURES=testing

install: 
	cargo install --features=${FEATURES} --path node --force --locked

run: 
	miden-node make-genesis --inputs-path node/genesis.toml
	miden-node start --config node/miden-node.toml

fmt:
