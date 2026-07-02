# mrt2-build/Makefile
# ===================
# Builds the shared MRT2 C++ inference library used by all players in this
# monorepo. Outputs into this directory:
#
#   libmagentart-core.a   — static library to link against
#   include/magentart/    — public C++ headers
#
# Can be invoked directly or via the root Makefile:
#   make -C mrt2-build [target]
#   make build-mrt2          (from repo root)
#
# Targets:
#   setup        Clone magenta-realtime & install Python/MLX dependencies
#   build-mrt2   Compile libmagentart-core.a and copy headers
#   clean        Remove cloned source and build artifacts
#   help         Show this message

.PHONY: help setup build-mrt2 mrt-init mrt-download clean

# --------------------------------------------------------------------------- #
# Paths — all absolute so the Makefile is safe to invoke from any CWD         #
# --------------------------------------------------------------------------- #

# Absolute path to this Makefile's own directory (mrt2-build/)
SELF_DIR := $(patsubst %/,%,$(dir $(abspath $(lastword $(MAKEFILE_LIST)))))

# Source: magenta-realtime is cloned inside mrt2-build/ (git-ignored)
MAGENTA_REALTIME_DIR ?= $(SELF_DIR)/magenta-realtime
GITHUB_REPO          ?= https://github.com/magenta/magenta-realtime.git

# CMake build directory (inside the cloned source tree)
CMAKE_BUILD_DIR := $(MAGENTA_REALTIME_DIR)/build

# Output directories (inside mrt2-build/ itself)
OUT_LIB_DIR     := $(SELF_DIR)
OUT_INCLUDE_DIR := $(SELF_DIR)/include

# Python venv for model management (mrt CLI)
PYTHON_VENV := $(SELF_DIR)/.venv

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
	@echo "=== MRT2 Shared Build System ==="
	@echo ""
	@echo "Build targets:"
	@echo "  setup            — Clone magenta-realtime & install Python/MLX deps"
	@echo "  build-mrt2       — Build libmagentart_all.a and copy headers"
	@echo "  clean            — Remove cloned source and all build artifacts"
	@echo ""
	@echo "Model management (via uv run — no venv activation needed):"
	@echo "  mrt-init         — Download shared codec assets (musiccoca, spectrostream)"
	@echo "  mrt-download     — Download a model  (MODEL=mrt2_small or mrt2_base)"
	@echo ""
	@echo "Examples:"
	@echo "  make mrt-init"
	@echo "  make mrt-download MODEL=mrt2_small"
	@echo ""
	@echo "Output:"
	@echo "  $(OUT_LIB_DIR)/libmagentart_all.a"
	@echo "  $(OUT_INCLUDE_DIR)/magentart/"
	@echo ""
	@echo "Override source location:"
	@echo "  make build-mrt2 MAGENTA_REALTIME_DIR=/path/to/magenta-realtime"

# --------------------------------------------------------------------------- #
# setup                                                                         #
# --------------------------------------------------------------------------- #

setup:
	@echo "=== Setting up MRT2 build environment ==="
	@if ! command -v uv >/dev/null 2>&1; then \
		echo "ERROR: 'uv' is required. Install it with:"; \
		echo "  curl -LsSf https://astral.sh/uv/install.sh | sh"; \
		exit 1; \
	fi
	@if [ ! -d "$(PYTHON_VENV)" ]; then \
		echo "Creating Python 3.12 venv at $(PYTHON_VENV)..."; \
		uv venv --python 3.12 "$(PYTHON_VENV)"; \
	fi
	@if ! command -v git >/dev/null 2>&1; then \
		echo "ERROR: 'git' is required to clone magenta-realtime."; \
		exit 1; \
	fi
	@if [ ! -d "$(MAGENTA_REALTIME_DIR)" ]; then \
		echo "Cloning magenta-realtime into $(MAGENTA_REALTIME_DIR)..."; \
		git clone --recursive "$(GITHUB_REPO)" "$(MAGENTA_REALTIME_DIR)"; \
	else \
		echo "magenta-realtime already present at $(MAGENTA_REALTIME_DIR)."; \
	fi
	@echo "Installing Python/MLX dependencies..."
	@uv pip install --python "$(PYTHON_VENV)/bin/python" \
		-e "$(MAGENTA_REALTIME_DIR)[mlx]"
	@echo "Setup complete."

# --------------------------------------------------------------------------- #
# build-mrt2                                                                    #
# --------------------------------------------------------------------------- #
# Uses cmake -S/-B syntax so there are no fragile nested 'cd' calls.

build-mrt2: setup
	@echo "=== Building libmagentart-core.a ==="
	@if [ ! -d "$(MAGENTA_REALTIME_DIR)" ]; then \
		echo "ERROR: source not found at $(MAGENTA_REALTIME_DIR)."; \
		echo "       Run 'make setup' first."; \
		exit 1; \
	fi
	cmake -S "$(MAGENTA_REALTIME_DIR)" \
	      -B "$(CMAKE_BUILD_DIR)" \
	      -DMAGENTART_BUILD_EXAMPLES=OFF \
	      -DMAGENTART_BUILD_TESTS=OFF \
	      -DCMAKE_BUILD_TYPE=Release \
	      -DCMAKE_POLICY_VERSION_MINIMUM=3.5
	cmake --build "$(CMAKE_BUILD_DIR)" \
	      --target magentart_core \
	      --parallel $(NPROC)
	@echo "Copying headers to $(OUT_INCLUDE_DIR)/magentart/ ..."
	@mkdir -p \
		"$(OUT_INCLUDE_DIR)/magentart" \
		"$(OUT_INCLUDE_DIR)/magentart/core" \
		"$(OUT_INCLUDE_DIR)/magentart/detail"
	@cp "$(MAGENTA_REALTIME_DIR)/core/include/magentart/"*.h \
	    "$(OUT_INCLUDE_DIR)/magentart/" 2>/dev/null || true
	@cp "$(MAGENTA_REALTIME_DIR)/core/include/magentart/core/"*.h \
	    "$(OUT_INCLUDE_DIR)/magentart/core/" 2>/dev/null || true
	@cp "$(MAGENTA_REALTIME_DIR)/core/include/magentart/detail/"*.h \
	    "$(OUT_INCLUDE_DIR)/magentart/detail/" 2>/dev/null || true
	@# cmake may place the library in the root or in a core/ subdirectory
	@# cmake names the library libmagentart_core.a (underscore)
	@if [ -f "$(CMAKE_BUILD_DIR)/core/libmagentart_core.a" ]; then \
		cp "$(CMAKE_BUILD_DIR)/core/libmagentart_core.a" "$(OUT_LIB_DIR)/libmagentart_core.a"; \
	elif [ -f "$(CMAKE_BUILD_DIR)/libmagentart_core.a" ]; then \
		cp "$(CMAKE_BUILD_DIR)/libmagentart_core.a" "$(OUT_LIB_DIR)/libmagentart_core.a"; \
	else \
		echo "ERROR: libmagentart_core.a not found after build."; \
		exit 1; \
	fi
	@echo "Merging transitive dependencies into libmagentart_all.a ..."
	@# Extract all .a paths from the cmake standalone link.txt and merge with libtool.
	@# This produces a single fat archive that SPM can link without needing to enumerate
	@# every transitive dep (TFLite, MLX, SentencePiece, abseil, ruy, …) in Package.swift.
	@LINK_TXT="$(CMAKE_BUILD_DIR)/examples/mrt2/standalone/CMakeFiles/mrt2_standalone.dir/link.txt"; \
	if [ -f "$$LINK_TXT" ]; then \
		grep -oE '\.\./\.\./\.\./[^ ]+\.a' "$$LINK_TXT" \
			| sed "s|../../../|$(CMAKE_BUILD_DIR)/|g" \
			> /tmp/mrt2_libs.txt; \
		xargs libtool -static -o "$(OUT_LIB_DIR)/libmagentart_all.a" < /tmp/mrt2_libs.txt; \
		echo "  merged  : $(OUT_LIB_DIR)/libmagentart_all.a ($$(du -sh $(OUT_LIB_DIR)/libmagentart_all.a | cut -f1))"; \
	else \
		echo "WARNING: link.txt not found — libmagentart_all.a not created."; \
		echo "         Run with MAGENTART_BUILD_EXAMPLES=ON to generate it."; \
	fi
	@echo ""
	@echo "Build complete:"
	@echo "  library : $(OUT_LIB_DIR)/libmagentart_core.a"
	@echo "  merged  : $(OUT_LIB_DIR)/libmagentart_all.a"
	@echo "  headers : $(OUT_INCLUDE_DIR)/magentart/"

# --------------------------------------------------------------------------- #
# mrt convenience targets                                                       #
# --------------------------------------------------------------------------- #
# Uses `uv run --directory` so no venv activation is required.
# The magenta-realtime checkout has pyproject.toml + uv.lock, making it a
# self-contained uv project that uv can resolve without touching the system.

MRT := uv run --directory "$(MAGENTA_REALTIME_DIR)" mrt

mrt-init: setup
	@echo "=== Downloading shared codec assets (musiccoca, spectrostream) ==="
	$(MRT) models init
	@echo ""
	@echo "Assets written to ~/Documents/Magenta/magenta-rt-v2/resources/"

mrt-download: setup
	@if [ -z "$(MODEL)" ]; then \
		echo "Usage: make mrt-download MODEL=mrt2_small"; \
		echo "       make mrt-download MODEL=mrt2_base"; \
		exit 1; \
	fi
	@echo "=== Downloading model: $(MODEL) ==="
	$(MRT) models download "$(MODEL)"

# --------------------------------------------------------------------------- #
# clean                                                                         #
# --------------------------------------------------------------------------- #

clean:
	@echo "=== Cleaning MRT2 build artifacts ==="
	@rm -rf "$(MAGENTA_REALTIME_DIR)" && echo "Removed cloned source."       || true
	@rm -f  "$(OUT_LIB_DIR)/libmagentart-core.a" && echo "Removed library." || true
	@rm -rf "$(OUT_INCLUDE_DIR)/magentart"        && echo "Removed headers." || true
	@rm -rf "$(PYTHON_VENV)"                       && echo "Removed venv."   || true
	@echo "Clean complete."
