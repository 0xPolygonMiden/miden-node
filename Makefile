.DEFAULT_GOAL := help

.PHONY: help
help:
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

# -- variables ------------------------------------------------------------------------------------

WARNINGS=RUSTDOCFLAGS="-D warnings"
BUILD_PROTO=BUILD_PROTO=1

# -- linting --------------------------------------------------------------------------------------

.PHONY: clippy
clippy: ## Runs Clippy with configs
	cargo clippy --locked --workspace --all-targets --all-features -- -D warnings --allow clippy::arc_with_non_send_sync


.PHONY: fix
fix: ## Runs Fix with configs
	cargo fix --allow-staged --allow-dirty --all-targets --all-features


.PHONY: format
format: ## Runs Format using nightly toolchain
	cargo +nightly fmt --all


.PHONY: format-check
format-check: ## Runs Format using nightly toolchain but only in check mode
	cargo +nightly fmt --all --check


.PHONY: lint
lint: format fix clippy ## Runs all linting tasks at once (Clippy, fixing, formatting)

# --- docs ----------------------------------------------------------------------------------------

.PHONY: doc
doc: ## Generates & checks documentation
	$(WARNINGS) cargo doc --all-features --keep-going --release --locked
	docker run --rm \
      -v $(PWD)/docs:/out \
      -v $(PWD)/proto:/protos \
      pseudomuto/protoc-gen-doc --doc_opt=/out/api.tmpl,api.md


.PHONY: doc-serve
doc-serve: ## Serves documentation site
	./scripts/serve-doc-site.sh

# --- testing -------------------------------------------------------------------------------------

.PHONY: test
test:  ## Runs all tests
	cargo nextest run --all-features --workspace --no-capture

# --- checking ------------------------------------------------------------------------------------

.PHONY: check
check: ## Check all targets and features for errors without code generation
	${BUILD_PROTO} cargo check --all-features --all-targets --locked

# --- building ------------------------------------------------------------------------------------

.PHONY: build
build: ## Builds all crates and re-builds ptotobuf bindings for proto crates
	${BUILD_PROTO} cargo build --locked

# --- installing ----------------------------------------------------------------------------------

.PHONY: install-node
install-node: ## Installs node
	${BUILD_PROTO} cargo install --path bin/node --locked

.PHONY: install-faucet
install-faucet: ## Installs faucet
	${BUILD_PROTO} cargo install --path bin/faucet --locked

.PHONY: install-node-testing
install-node-testing: ## Installs node with testing feature enabled
	${BUILD_PROTO} cargo install --features testing --path bin/node --locked

.PHONY: install-faucet-testing
install-faucet-testing: ## Installs faucet with testing feature enabled
	${BUILD_PROTO} cargo install --features testing --path bin/faucet --locked

# --- docker --------------------------------------------------------------------------------------

.PHONY: docker-build-node
docker-build-node: ## Builds the Miden node using Docker
	@CREATED=$$(date) && \
	VERSION=$$(cat bin/node/Cargo.toml | grep -m 1 '^version' | cut -d '"' -f 2) && \
	COMMIT=$$(git rev-parse HEAD) && \
	docker build --build-arg CREATED="$$CREATED" \
        		 --build-arg VERSION="$$VERSION" \
          		 --build-arg COMMIT="$$COMMIT" \
                 -f bin/node/Dockerfile \
                 -t miden-node-image .

.PHONY: docker-run-node
docker-run-node: ## Runs the Miden node as a Docker container
	docker volume create miden-db
	@ABSOLUTE_PATH="$$(pwd)/config/miden-node.toml" && \
	docker run --name miden-node \
			   -p 57291:57291 \
               -v miden-db:/db \
               -v "$${ABSOLUTE_PATH}:/miden-node.toml" \
               -d miden-node-image
