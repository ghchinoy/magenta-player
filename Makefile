# magenta-player root Makefile
# ============================
# Orchestrates the shared MRT2 build and all player sub-projects.
#
# Quick start (Swift player):
#   make setup
#   make build-mrt2
#   make run-swift
#
# Full build (all players):
#   make all
#
# Targets:
#   setup         — Set up Python/MLX environment and clone magenta-realtime
#   build-mrt2    — Build shared libmagentart-core.a + headers into mrt2-build/
#   build-swift   — Build the Swift player (release binary)
#   run-swift     — Build (incremental) and launch the Swift player
#   build-rust    — Build the Rust player CLI
#   all           — setup → build-mrt2 → build-swift build-rust
#   clean         — Remove all build artifacts (mrt2 library + all players)
#   clean-mrt2    — Remove only the shared MRT2 library and cloned source
#   clean-swift   — Remove only Swift build artifacts
#   clean-rust    — Remove only Rust build artifacts
#   help          — Show this message

.PHONY: help setup build-mrt2 build-swift run-swift build-rust all \
        mrt-init mrt-download \
        clean clean-mrt2 clean-swift clean-rust

# --------------------------------------------------------------------------- #
# Sub-project directories                                                       #
# --------------------------------------------------------------------------- #

MRT2_DIR  := mrt2-build
SWIFT_DIR := swift-player
RUST_DIR  := rust-player

# Expose the shared library location to sub-project Makefiles
export MRT2_BUILD_DIR := $(abspath $(MRT2_DIR))

# --------------------------------------------------------------------------- #
# help                                                                          #
# --------------------------------------------------------------------------- #

help:
	@echo "=== Magenta Players — Root Build ==="
	@echo ""
	@echo "Shared engine:"
	@echo "  setup              Set up Python/MLX env and clone magenta-realtime"
	@echo "  build-mrt2         Build libmagentart_all.a into mrt2-build/"
	@echo ""
	@echo "Model management (run once after setup):"
	@echo "  mrt-init           Download shared codec assets (musiccoca, spectrostream)"
	@echo "  mrt-download       Download a model weights file"
	@echo "                     Usage: make mrt-download MODEL=mrt2_small"
	@echo ""
	@echo "Swift player:"
	@echo "  build-swift        Release build → swift-player/.build/release/magenta-player"
	@echo "  run-swift          Incremental build + launch"
	@echo ""
	@echo "Rust player:"
	@echo "  build-rust         cargo build → rust-player/target/release/magenta-rust-player"
	@echo ""
	@echo "Aggregate:"
	@echo "  all                setup → build-mrt2 → build-swift + build-rust"
	@echo "  clean              Remove all build artifacts"

# --------------------------------------------------------------------------- #
# Shared engine                                                                 #
# --------------------------------------------------------------------------- #

setup:
	$(MAKE) -C $(MRT2_DIR) setup

build-mrt2: setup
	$(MAKE) -C $(MRT2_DIR) build-mrt2

# --------------------------------------------------------------------------- #
# Model management                                                              #
# --------------------------------------------------------------------------- #

mrt-init:
	$(MAKE) -C $(MRT2_DIR) mrt-init

mrt-download:
	$(MAKE) -C $(MRT2_DIR) mrt-download MODEL=$(MODEL)

# --------------------------------------------------------------------------- #
# Swift player                                                                  #
# --------------------------------------------------------------------------- #

build-swift: build-mrt2
	$(MAKE) -C $(SWIFT_DIR) build-swift

run-swift:
	$(MAKE) -C $(SWIFT_DIR) run-swift

# --------------------------------------------------------------------------- #
# Rust player                                                                   #
# --------------------------------------------------------------------------- #

build-rust: build-mrt2
	$(MAKE) -C $(RUST_DIR) build

# --------------------------------------------------------------------------- #
# Aggregate                                                                     #
# --------------------------------------------------------------------------- #

all: build-mrt2 build-swift build-rust

# --------------------------------------------------------------------------- #
# Clean                                                                         #
# --------------------------------------------------------------------------- #

clean: clean-mrt2 clean-swift clean-rust

clean-mrt2:
	$(MAKE) -C $(MRT2_DIR) clean

clean-swift:
	$(MAKE) -C $(SWIFT_DIR) clean

clean-rust:
	@if command -v cargo >/dev/null 2>&1; then \
		cargo clean --manifest-path $(RUST_DIR)/Cargo.toml; \
	else \
		rm -rf $(RUST_DIR)/target && echo "Removed rust-player/target/"; \
	fi
