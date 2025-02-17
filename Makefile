.DEFAULT_GOAL := help

.PHONY: help
help:
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

# -- variables ------------------------------------------------------------------------------------

WARNINGS=RUSTDOCFLAGS="-D warnings"
BUILD_PROTO=BUILD_PROTO=1

PROVER_DIR="miden-base"
PROVER_REPO="https://github.com/0xPolygonMiden/miden-base.git"
PROVER_BRANCH="next"
PROVER_PORT=8082

# -- linting --------------------------------------------------------------------------------------

.PHONY: clippy
clippy: ## Runs Clippy with configs
	cargo clippy --locked --workspace --all-targets --all-features --exclude miden-faucet -- -D warnings


.PHONY: fix
fix: ## Runs Fix with configs
	cargo fix --allow-staged --allow-dirty --workspace --all-features --exclude miden-faucet


.PHONY: format
format: ## Runs Format using nightly toolchain
	cargo +nightly fmt --all


.PHONY: format-check
format-check: ## Runs Format using nightly toolchain but only in check mode
	cargo +nightly fmt --all --check


.PHONY: toml
toml: ## Runs Format for all TOML files
	taplo fmt


.PHONY: toml-check
toml-check: ## Runs Format for all TOML files but only in check mode
	taplo fmt --check --verbose


.PHONY: workspace-check
workspace-check: ## Runs a check that all packages have `lints.workspace = true`
	cargo workspace-lints


.PHONY: lint
lint: format fix clippy toml workspace-check ## Runs all linting tasks at once (Clippy, fixing, formatting, workspace)

# --- docs ----------------------------------------------------------------------------------------

.PHONY: doc
doc: ## Generates & checks documentation
	$(WARNINGS) cargo doc --all-features --keep-going --release --locked


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

# --- remote prover -------------------------------------------------------------------------------

.PHONY: clean-prover
clean-prover: ## Uninstall prover
	cargo uninstall miden-proving-service || echo 'prover not installed'

.PHONY: prover
prover: setup-miden-base update-prover-branch build-prover ## Setup prover directory

.PHONY: setup-miden-base
setup-miden-base: ## Clone the miden-base repository if it doesn't exist
	if [ ! -d $(PROVER_DIR) ]; then git clone $(PROVER_REPO) $(PROVER_DIR); fi

.PHONY: update-prover-branch
update-prover-branch: setup-miden-base ## Checkout and update the specified branch in miden-base
	cd $(PROVER_DIR) && git checkout $(PROVER_BRANCH) && git pull origin $(PROVER_BRANCH)

.PHONY: build-prover
build-prover: update-prover-branch ## Update dependencies and build the prover binary with specified features
	cd $(PROVER_DIR) && cargo update && cargo build --bin miden-proving-service --locked $(PROVER_FEATURES_TESTING) --release

.PHONY: start-prover
start-prover: ## Run prover. This requires the base repo to be present at `miden-base`
	cd $(PROVER_DIR) && RUST_LOG=info cargo run --bin miden-proving-service $(PROVER_FEATURES_TESTING) --release --locked -- start-worker -p $(PROVER_PORT) --batch-prover

.PHONY: kill-prover
kill-prover: ## Kill prover process
	pkill miden-tx-prover || echo 'process not running'
