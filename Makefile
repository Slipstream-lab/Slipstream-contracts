# Slipstream-contracts developer tasks.
#
# Mirrors the checks CI runs (see .github/workflows/ci.yml) so contributors can
# reproduce them locally with a single command and without memorising the
# Soroban wasm target quirk (`wasm32v1-none`, required by soroban-sdk 27).

WASM_TARGET := wasm32v1-none

.DEFAULT_GOAL := help

.PHONY: help
help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'

.PHONY: fmt
fmt: ## Check formatting (cargo fmt --check)
	cargo fmt --all --check

.PHONY: fmt-fix
fmt-fix: ## Apply formatting
	cargo fmt --all

.PHONY: clippy
clippy: ## Lint with clippy, warnings as errors
	cargo clippy --workspace --all-targets -- -D warnings

.PHONY: test
test: ## Run the native test suite
	cargo test --workspace

.PHONY: wasm
wasm: ## Build the contract cdylibs for the Soroban wasm target
	@# Ensure the target is available. Non-fatal: on some local toolchains the
	@# std files are already present even when rustup cannot (re)install the
	@# component, and the build below is the real source of truth.
	-@rustup target add $(WASM_TARGET)
	cargo build --workspace --exclude harness --target $(WASM_TARGET) --release

.PHONY: check
check: fmt clippy test ## Run fmt + clippy + native tests (fast local gate)

.PHONY: all
all: check wasm ## Everything CI runs: fmt, clippy, test, wasm build
