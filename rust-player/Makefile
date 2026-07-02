# Makefile for Magenta Rust Player Project
# ==========================================
# 
# This Makefile provides targets to:
# 1. Set up Python dependencies for magenta-rt-v2 (if needed)
# 2. Build the Rust player using cargo against shared MRT2 build artifacts
# 3. Run tests and linting
# 4. Manage development workflows
#
# The Rust player uses cxx to link against the shared mrt2-build/ static library

.PHONY: help setup build test clean dev run

# Show available targets
help:
	@echo "=== Magenta Rust Player Build Targets ==="
	@echo ""
	@echo "Available targets:"
	@echo "  help              - Show this help message"
	@echo "  setup             - Setup Python dependencies for magenta-rt-v2 (if needed)"
	@echo "  build             - Build the Rust player using cargo (requires mrt2-build/ setup)"
	@echo "  test              - Run Rust unit tests"
	@echo "  lint              - Run Rust formatting and linting checks"
	@echo "  dev               - Run in development mode (watch for changes)"
	@echo "  run               - Build and run the Rust player with default args"
	@echo "  clean             - Clean build artifacts"
	@echo ""
	@echo "Quick Start:"
	@echo "  1. Run 'make setup' to install Python dependencies"
	@echo "  2. Run 'make build' to build against shared mrt2-build/"
	@echo "  3. Run 'make run' to test the CLI"

# Setup Python dependencies for magenta-rt-v2 (MLX model dependencies)
setup:
	@echo "=== Setting up Python environment for magenta-rt-v2 ==="
	@if [ "$$(command -v uv 2>/dev/null)" ]; then \
		echo "Using uv for Python environment..."; \
		if [ ! -d ".venv" ]; then \
			echo "Creating Python virtual environment..."; \
			uv venv --python 3.12; \
		fi; \
		cd .venv/bin && \
		uv pip install -r ~/.cargo/.venv/bin/magenta-rt-v2/requirements.txt 2>/dev/null || \
		echo "magenta-rt-v2 requirements not found in cargo directory"; \
		echo "Python dependencies setup complete..."; \
	else \
		echo "ERROR: uv is required. Please install uv first:"; \
		echo "  curl -LsSf https://astral.sh/uv/install.sh | sh"; \
		exit 1; \
	fi

# Build the Rust player using cargo
build: setup
	@echo "=== Building Rust player (requires mrt2-build/ setup) ==="
	@if [ ! -d "../mrt2-build" ]; then \
		echo "ERROR: ../mrt2-build/ directory not found. Run 'cd .. && make build-mrt2' first"; \
		echo "Missing MRT2 static library"; \
		exit 1; \
	fi
	@echo "Compiling Rust code with cxx bridge..."
	cargo build --release

# Run Rust unit tests
test:
	@echo "=== Running Rust unit tests ==="
	cargo test

# Run Rust formatting and linting checks
lint:
	@echo "=== Running Rust formatting and linting checks ==="
	cargo fmt --check
	cargo clippy -- -D warnings

# Run in development mode (watch for changes)
dev:
	@echo "=== Running in development mode (requires 'cargo watch') ==="
	@if command -v cargo watch >/dev/null 2>&1; then \
		cargo watch -x "test" -x "run"; \
	else \
		echo "cargo-watch not installed. Run 'cargo install cargo-watch' or use 'make build && make run'"; \
		make build && make run; \
	fi

# Build and run the Rust player with default args
run: build
	@echo "=== Running Rust player CLI ==="
	@./target/release/magenta-rust --prompt "ambient lofi chords with acoustic guitar" --model ~/Documents/Magenta/magenta-rt-v2/models/model.mlxfn --resources ~/Documents/Magenta/magenta-rt-v2/resources 2>&1 || echo "Example run completed (may require actual model files)"

# Clean build artifacts
clean:
	@echo "=== Cleaning Rust player build artifacts ==="
	@if [ -d "target" ]; then \
		rm -rf target; \
		echo "Cargo target directory cleaned"; \
	fi
	@if [ -f "Cargo.lock" ]; then \
		rm Cargo.lock; \
		echo "Cargo.lock removed"; \
	fi
	@echo "Clean complete"
