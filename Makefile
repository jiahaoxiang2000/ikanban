.PHONY: all build build-release build-dev clean test check lint run-server run-client run run-all help

# Default target
all: build

# Build all crates in release mode
build-release:
	cargo build --release

# Build all crates in debug mode (default)
build: build-dev

build-dev:
	cargo build

# Clean build artifacts
clean:
	cargo clean

# Run tests (when available)
test:
	cargo test

# Check code for errors (faster than build)
check:
	cargo check

# Run clippy for linting
lint:
	cargo clippy

# Run the server
run-server:
	cargo run --bin ikanban-server

# Run the TUI client
run-client:
	cargo run --bin ikanban

# Help target
help:
	@echo "Available make targets:"
	@echo ""
	@echo "  build          - Build all crates in debug mode (default)"
	@echo "  build-release  - Build all crates in release mode"
	@echo "  build-dev      - Build all crates in debug mode"
	@echo "  clean          - Clean build artifacts"
	@echo "  test           - Run tests"
	@echo "  check          - Check code for errors (fast)"
	@echo "  lint           - Run clippy linter"
	@echo "  run-server     - Start the API server"
	@echo "  run-client     - Start the TUI client"
	@echo "  help           - Show this help message"
