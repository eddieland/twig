### Makefile

.PHONY: help
help: ## Display this help
	@echo "Usage: make [target]"
	@echo ""
	@echo "Targets:"
	@awk 'BEGIN {section="General"} /^### /{section=substr($$0,5); printf "\n\033[1m%s\033[0m\n", section} /^[a-zA-Z0-9_-]+:.*?## / {match($$0, /## (.*)$$/, a); printf "  \033[36m%-18s\033[0m %s\n", substr($$1,1,length($$1)-1), a[1]}' $(MAKEFILE_LIST)

### Development

.PHONY: fmt
fmt: ## Format code using rustfmt
	cargo fmt --all
	cargo clippy --fix --allow-dirty --workspace
	uvx ruff format examples/plugins/twig-backup || :
	uvx ruff check --fix --select F,E,W --ignore F841 examples/plugins/twig-backup || :
	uvx --from mdformat --with mdformat-gfm mdformat --wrap 120 . || :

.PHONY: lint
lint: ## Run clippy for linting
	cargo clippy --workspace -- -D warnings

.PHONY: lint-all
lint-all: ## Run clippy with all features
	cargo clippy --all-features -- -D warnings

.PHONY: test
test: build ## Run tests
	cargo nextest run --workspace
	cd plugins/twig-flow && cargo nextest run
	cd plugins/twig-prune && cargo nextest run

.PHONY: test-all
test-all: ## Run tests with all features
	cargo nextest run --all-features

.PHONY: check
check: ## Run cargo check
	cargo check --workspace

.PHONY: doc
doc: ## Generate documentation
	cargo doc --workspace --no-deps

.PHONY: watch-test
watch-test: ## Run tests in watch mode (requires cargo-watch)
	cargo watch -x "nextest run --workspace"

.PHONY: all
all: fmt lint test ## Run fmt, lint, and test

### Snapshot Testing

.PHONY: insta-review
insta-review: ## Review Insta snapshots
	cargo insta review --workspace

.PHONY: insta-accept
insta-accept: ## Accept all pending Insta snapshots
	cargo insta accept --workspace

.PHONY: insta-reject
insta-reject: ## Reject all pending Insta snapshots
	cargo insta reject --workspace

.PHONY: update-snapshots
update-snapshots: ## Run tests and update snapshots
	INSTA_UPDATE=1 cargo nextest run --workspace

### Analysis

.PHONY: cloc
cloc: ## Count lines of code using Docker
	docker run --rm -v "$(PWD):/tmp" aldanial/cloc /tmp \
		--exclude-dir=.git,.github,.twig,example,docs,ref,target \
		--fullpath

### Coverage

.PHONY: coverage
coverage: ## Run code coverage
	cargo llvm-cov nextest --workspace

.PHONY: coverage-html
coverage-html: ## Generate HTML coverage report
	cargo llvm-cov nextest --workspace --html

.PHONY: coverage-open
coverage-open: ## Generate HTML coverage report and open it in browser
	cargo llvm-cov nextest --workspace --html --open

.PHONY: coverage-report
coverage-report: ## Generate LCOV report
	cargo llvm-cov nextest --workspace --lcov --output-path lcov.info

### Build

.PHONY: build
build: ## Build the project
	cargo build --workspace
	cd plugins/twig-flow && cargo build
	cd plugins/twig-prune && cargo build

.PHONY: release
release: ## Build release version
	cargo build --release --workspace

.PHONY: release-size
release-size: ## Build size-optimized release version
	cargo build --release --workspace
	@echo "\nBinary size before compression:"
	@du -h target/release/twig

.PHONY: clean
clean: ## Clean build artifacts
	cargo clean

.PHONY: run
run: ## Run the application
	cargo run --workspace

### Installation

.PHONY: install
install: ## Install twig locally
	cargo install --path twig-cli

.PHONY: install-flow-plugin
install-flow-plugin: ## Install twig flow plugin
	cargo install --path plugins/twig-flow

.PHONY: install-mcp
install-mcp: ## Install twig-mcp MCP server
	cargo install --path twig-mcp

.PHONY: install-prune-plugin
install-prune-plugin: ## Install twig prune plugin
	cargo install --path plugins/twig-prune

.PHONY: install-dev-tools
install-dev-tools: ## Install development tools
	rustup show # Ensures rust-toolchain.toml is applied
	cargo install cargo-watch
	cargo install cargo-outdated
	cargo install cargo-nextest
	cargo install cargo-llvm-cov
	cargo install cargo-insta
	uv tool install pre-commit

.PHONY: pre-commit-setup
pre-commit-setup: ## Set up pre-commit hooks
	pre-commit install

.PHONY: pre-commit-run
pre-commit-run: ## Run pre-commit hooks manually
	pre-commit run --all-files
