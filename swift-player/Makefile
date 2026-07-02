# swift-player/Makefile
# =====================
# Builds the native SwiftUI macOS player.
#
# The MRT2 C++ inference library is built once in the shared mrt2-build/
# directory. This Makefile delegates setup and engine compilation there.
#
# Preferred entry point (from repo root):
#   make build-swift     # builds mrt2 if needed, then the Swift player
#   make run-swift       # incremental rebuild + launch as a proper .app
#
# Standalone (from this directory):
#   make setup           # delegates to ../mrt2-build
#   make build-mrt2      # delegates to ../mrt2-build
#   make build-swift     # release binary + .app bundle → .build/MagentaPlayer.app
#   make run-swift       # build (if needed) + open .app (keyboard input works)
#   make clean           # remove Swift build artifacts only
#
# Override the shared library path (e.g. CI):
#   make build-swift MRT2_BUILD_DIR=/path/to/mrt2-build

.PHONY: help setup build-mrt2 build-swift run-swift all clean clean-legacy

# --------------------------------------------------------------------------- #
# Paths                                                                         #
# --------------------------------------------------------------------------- #

SELF_DIR := $(patsubst %/,%,$(dir $(abspath $(lastword $(MAKEFILE_LIST)))))

MRT2_BUILD_DIR ?= $(abspath $(SELF_DIR)/../mrt2-build)

SWIFT_BUILD_DIR := $(SELF_DIR)/.build
RELEASE_BINARY  := $(SWIFT_BUILD_DIR)/release/magenta-player

# The .app bundle — assembled from the release binary + Info.plist.
# Launching via `open` gives the app proper keyboard focus and a Dock icon.
APP_BUNDLE   := $(SWIFT_BUILD_DIR)/MagentaPlayer.app
APP_MACOS    := $(APP_BUNDLE)/Contents/MacOS
APP_BINARY   := $(APP_MACOS)/magenta-player
INFO_PLIST   := $(SELF_DIR)/Resources/Info.plist

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
	@echo "  build-swift  Release build + assemble .app bundle"
	@echo "  run-swift    Build (if needed) and launch MagentaPlayer.app via open"
	@echo "  all          build-mrt2 → build-swift"
	@echo "  clean        Remove Swift .build/ artifacts"
	@echo ""
	@echo "Shared library : $(MRT2_BUILD_DIR)"
	@echo "App bundle     : $(APP_BUNDLE)"

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

# build-swift compiles the Swift package then assembles a proper .app bundle.
# Launching the bundle via `open` gives the app a Dock presence and routes
# keyboard events correctly — plain executables from `swift run` do not.
build-swift: build-mrt2
	@echo "=== Building Swift player (release) ==="
	@if ! command -v swift >/dev/null 2>&1; then \
		echo "ERROR: Swift compiler not found."; \
		echo "Install Xcode Command Line Tools:  xcode-select --install"; \
		exit 1; \
	fi
	MRT2_BUILD_DIR="$(MRT2_BUILD_DIR)" swift build -c release --product magenta-player
	@echo ""
	@echo "=== Assembling MagentaPlayer.app bundle ==="
	@mkdir -p "$(APP_MACOS)" "$(APP_BUNDLE)/Contents/Resources"
	@cp "$(RELEASE_BINARY)" "$(APP_BINARY)"
	@cp "$(INFO_PLIST)" "$(APP_BUNDLE)/Contents/Info.plist"
	@cp "$(SELF_DIR)/Resources/about-logo.png" "$(APP_BUNDLE)/Contents/Resources/"
	@echo ""
	@echo "Build complete:"
	@echo "  binary : $(RELEASE_BINARY)"
	@echo "  bundle : $(APP_BUNDLE)"
	@echo ""
	@echo "Launch with:  make run-swift"
	@echo "           or open \"$(APP_BUNDLE)\""

# run-swift builds incrementally (swift build, not swift run) then opens the
# .app bundle so macOS routes keyboard events to it properly.
run-swift: build-mrt2
	@echo "=== Building and launching MagentaPlayer.app ==="
	@if ! command -v swift >/dev/null 2>&1; then \
		echo "ERROR: Swift compiler not found."; \
		echo "Install Xcode Command Line Tools:  xcode-select --install"; \
		exit 1; \
	fi
	MRT2_BUILD_DIR="$(MRT2_BUILD_DIR)" swift build --product magenta-player
	@mkdir -p "$(APP_MACOS)" "$(APP_BUNDLE)/Contents/Resources"
	@cp "$(SWIFT_BUILD_DIR)/debug/magenta-player" "$(APP_BINARY)"
	@cp "$(INFO_PLIST)" "$(APP_BUNDLE)/Contents/Info.plist"
	@cp "$(SELF_DIR)/Resources/about-logo.png" "$(APP_BUNDLE)/Contents/Resources/"
	@echo "Opening $(APP_BUNDLE) ..."
	open "$(APP_BUNDLE)"

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
		echo "Removed swift-player/magenta-realtime/."; \
	else \
		echo "Nothing to remove — swift-player/magenta-realtime/ does not exist."; \
	fi
