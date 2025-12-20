CI := 1

.PHONY: help build dev-fmt all check fmt clippy test deny security

# Default target - show help
.DEFAULT_GOAL := help

# Show this help message
help:
	@awk '/^# / { desc=substr($$0, 3) } /^[a-zA-Z0-9_-]+:/ && desc { target=$$1; sub(/:$$/, "", target); printf "%-20s - %s\n", target, desc; desc="" }' Makefile | sort

# Build the workspace
build:
	cargo build --workspace

# Fix formatting issues
dev-fmt:
	cargo fmt --all

# Run all checks and build
all: check build

# Check code formatting
fmt:
	cargo fmt --all -- --check

# Run clippy linter
clippy:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

# Run all tests
test:
	cargo test --workspace

# Check licenses and dependencies
deny:
	@command -v cargo-deny >/dev/null || (echo "Installing cargo-deny..." && cargo install cargo-deny)
	cargo deny check

# Run all security checks
security: deny

# Run all quality checks
check: fmt clippy test
