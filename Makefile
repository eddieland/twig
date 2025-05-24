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

.PHONY: lint
lint: ## Run clippy for linting
	cargo clippy -- -D warnings

.PHONY: lint-all
lint-all: ## Run clippy with all features
	cargo clippy --all-features -- -D warnings

.PHONY: test
test: build ## Run tests
	cargo nextest run

.PHONY: test-all
test-all: ## Run tests with all features
	cargo nextest run --all-features

.PHONY: check
check: ## Run cargo check
	cargo check

.PHONY: doc
doc: ## Generate documentation
	cargo doc --no-deps

.PHONY: watch-test
watch-test: ## Run tests in watch mode (requires cargo-watch)
	cargo watch -x "nextest run"

.PHONY: all
all: fmt lint test docker-build ## Run verify-config, fmt, lint, and test

### Build

.PHONY: build
build: ## Build the project
	cargo build

.PHONY: release
release: ## Build release version
	cargo build --release

.PHONY: clean
clean: ## Clean build artifacts
	cargo clean

.PHONY: run
run: ## Run the application
	cargo run

### Installation

.PHONY: install
install: ## Install edlicense locally
	cargo install --path .


.PHONY: install-dev-tools
install-dev-tools: ## Install development tools
	rustup show # Ensures rust-toolchain.toml is applied
	cargo install cargo-watch
	cargo install cargo-outdated
	cargo install cargo-nextest
