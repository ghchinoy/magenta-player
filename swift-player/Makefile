# swift-player/Makefile
# =====================
# Builds the native SwiftUI macOS player.
#
# The MRT2 C++ inference library is built once in the shared mrt2-build/
# directory. This Makefile delegates setup and engine compilation there.
#
# Preferred entry point (from repo root):
#   make build-swift     # builds mrt2 if needed, then the Swift player
#   make run-swift       # incremental rebuild + launch
#
# Standalone (from this directory):
#   make setup           # delegates to ../mrt2-build
#   make build-mrt2      # delegates to ../mrt2-build
#   make build-swift     # release binary → .build/release/magenta-player
#   make run-swift       # incremental build + launch
#   make clean           # remove Swift build artifacts only
#
# Override the shared library path (e.g. CI):
#   make build-swift MRT2_BUILD_DIR=/path/to/mrt2-build

.PHONY: help setup build-mrt2 build-swift run-swift all clean clean-legacy

# --------------------------------------------------------------------------- #
# Paths                                                                         #
# --------------------------------------------------------------------------- #

# Absolute path to this Makefile's directory (swift-player/)
SELF_DIR := $(patsubst %/,%,$(dir $(abspath $(lastword $(MAKEFILE_LIST)))))

# Shared MRT2 library — resolve relative to swift-player/ unless overridden
# by an environment variable (set by the root Makefile via 'export').
MRT2_BUILD_DIR ?= $(abspath $(SELF_DIR)/../mrt2-build)

SWIFT_BUILD_DIR := $(SELF_DIR)/.build

# --------------------------------------------------------------------------- #
# Parallelism                                                                   #
# --------------------------------------------------------------------------- #

UNAME_S := $(shell uname -s)
ifeq ($(UNAME_S),Darwin)
    NPROC := $(shell sysctl -n hw.ncpu)
else
    NPROC := $(shell nproc 2>/dev/null || echo 4)
endif

# --------------------------------------------------------------------------- #
# help                                                                          #
# --------------------------------------------------------------------------- #

help:
	@echo "=== Magenta Swift Player ==="
	@echo ""
	@echo "Targets:"
	@echo "  setup        Delegate to shared mrt2-build: clone + Python/MLX deps"
	@echo "  build-mrt2   Delegate to shared mrt2-build: compile C++ engine"
	@echo "  build-swift  Swift release build → .build/release/magenta-player"
	@echo "  run-swift    Incremental build + launch the app"
	@echo "  all          build-mrt2 → build-swift"
	@echo "  clean        Remove Swift .build/ artifacts"
	@echo ""
	@echo "Shared library: $(MRT2_BUILD_DIR)"
	@echo "Binary target : $(SWIFT_BUILD_DIR)/release/magenta-player"
	@echo "                $(SWIFT_BUILD_DIR)/MRT2  (copy)"

# --------------------------------------------------------------------------- #
# Shared engine (delegated)                                                     #
# --------------------------------------------------------------------------- #

setup:
	@echo "=== Delegating setup to shared mrt2-build ==="
	$(MAKE) -C "$(MRT2_BUILD_DIR)" setup

build-mrt2:
	@echo "=== Delegating C++ engine build to shared mrt2-build ==="
	$(MAKE) -C "$(MRT2_BUILD_DIR)" build-mrt2

# --------------------------------------------------------------------------- #
# Swift player                                                                  #
# --------------------------------------------------------------------------- #

# build-swift depends on build-mrt2 so `make build-swift` from this directory
# always ensures the shared library is up to date first.
build-swift: build-mrt2
	@echo "=== Building Swift player (release) ==="
	@if ! command -v swift >/dev/null 2>&1; then \
		echo "ERROR: Swift compiler not found."; \
		echo "Install Xcode Command Line Tools:  xcode-select --install"; \
		exit 1; \
	fi
	MRT2_BUILD_DIR="$(MRT2_BUILD_DIR)" swift build -c release --product magenta-player
	@mkdir -p "$(SWIFT_BUILD_DIR)"
	@cp "$(SWIFT_BUILD_DIR)/release/magenta-player" "$(SWIFT_BUILD_DIR)/MRT2"
	@echo ""
	@echo "Build complete:"
	@echo "  $(SWIFT_BUILD_DIR)/release/magenta-player"
	@echo "  $(SWIFT_BUILD_DIR)/MRT2  (copy)"

# run-swift does NOT depend on build-mrt2 — it uses swift run which does an
# incremental Swift-only rebuild. The C++ library only matters once the real
# bridge is wired up.
run-swift:
	@echo "=== Launching Magenta Player ==="
	@if ! command -v swift >/dev/null 2>&1; then \
		echo "ERROR: Swift compiler not found."; \
		echo "Install Xcode Command Line Tools:  xcode-select --install"; \
		exit 1; \
	fi
	MRT2_BUILD_DIR="$(MRT2_BUILD_DIR)" swift run magenta-player

# --------------------------------------------------------------------------- #
# Aggregate                                                                     #
# --------------------------------------------------------------------------- #

all: build-swift

# --------------------------------------------------------------------------- #
# Clean (Swift artifacts only — shared engine cleaned via root Makefile)        #
# --------------------------------------------------------------------------- #

clean:
	@echo "=== Cleaning Swift build artifacts ==="
	@rm -rf "$(SELF_DIR)/.build" && echo "Removed .build/"
	@echo "Clean complete."
	@echo "(To also clean the shared MRT2 library: make clean-mrt2 from repo root)"

# Remove the legacy magenta-realtime clone that lived here before the
# mrt2-build migration. Safe to run once; no-ops if the directory is gone.
clean-legacy:
	@echo "=== Removing legacy swift-player/magenta-realtime/ clone ==="
	@if [ -d "$(SELF_DIR)/magenta-realtime" ]; then \
		rm -rf "$(SELF_DIR)/magenta-realtime"; \
		echo "Removed swift-player/magenta-realtime/ ($(shell du -sh $(SELF_DIR)/magenta-realtime 2>/dev/null | cut -f1) freed)."; \
	else \
		echo "Nothing to remove — swift-player/magenta-realtime/ does not exist."; \
	fi
