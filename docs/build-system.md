# Build System — Lessons Learned

## cmake Target Names Use Underscores

cmake library targets use underscores, not hyphens, even if the project name
has hyphens. Always verify with `cmake --build <dir> --target help`:

```
magentart_core        ← correct cmake target name
magentart-core        ← wrong — "No rule to make target" error
```

The output library follows the same convention:
```
libmagentart_core.a   ← correct
libmagentart-core.a   ← wrong — copy step silently fails
```

## Use `cmake -S/-B` Not Nested `cd` in Makefiles

The classic `cd <src> && cmake . && cd build && make` pattern produces
confusing relative path bugs when variables are set at the outer Makefile
level. Use the `-S`/`-B` form — all paths are explicit:

```makefile
cmake -S "$(MAGENTA_REALTIME_DIR)" \
      -B "$(CMAKE_BUILD_DIR)" \
      -DCMAKE_BUILD_TYPE=Release
cmake --build "$(CMAKE_BUILD_DIR)" \
      --target magentart_core \
      --parallel $(NPROC)
```

## Makefile Self-Directory Pattern

Makefiles invoked via `make -C <dir>` or from a different CWD will resolve
relative paths incorrectly. Always compute an absolute base at the top of
every Makefile:

```makefile
SELF_DIR := $(patsubst %/,%,$(dir $(abspath $(lastword $(MAKEFILE_LIST)))))
```

Then use `$(SELF_DIR)/relative/path` for all output paths. This makes the
Makefile safe to call from any working directory.

## cmake Flags That Don't Exist Just Warn

Passing `-DMAGENTART_BUILD_EXAMPLES=OFF` when the project doesn't define that
option produces only a cmake warning:
```
CMake Warning: Manually-specified variables were not used by the project
```
cmake still configures and builds successfully — the flag is silently ignored.
Check the project's `CMakeLists.txt` for the actual option names with
`grep -r "option(" CMakeLists.txt`.

## Shared Library Build Output Location

cmake may place the `.a` file at the build root or in a target subdirectory
depending on project structure. Check both:

```makefile
if [ -f "$(CMAKE_BUILD_DIR)/core/libmagentart_core.a" ]; then
    cp ...core/libmagentart_core.a ...
elif [ -f "$(CMAKE_BUILD_DIR)/libmagentart_core.a" ]; then
    cp ...libmagentart_core.a ...
fi
```

## `uv run --directory` Avoids Venv Activation

Never ask users to activate a venv or use a full path like
`.venv/bin/mrt models init`. If the project has a `pyproject.toml` and
`uv.lock`, `uv run --directory` resolves the environment automatically:

```bash
uv run --directory mrt2-build/magenta-realtime mrt models init
```

Expose this through a Makefile target so users only type `make mrt-init` — the
uv invocation is an implementation detail.

## Long Builds Need `nohup` or Background Logging

cmake builds that take 10–30 minutes must not be run with a captured stdout
in any tool with a timeout. Use:

```bash
nohup cmake --build "$BUILD_DIR" --target magentart_core --parallel 10 \
  > mrt2-build/build.log 2>&1 &
```

Monitor with `tail -f mrt2-build/build.log`. The build is complete when the
last line is `[100%] Built target magentart_core`.

## Export `MRT2_BUILD_DIR` from the Root Makefile

Sub-makes inherit exported variables automatically. The root Makefile should:

```makefile
export MRT2_BUILD_DIR := $(abspath mrt2-build)
```

Then `Package.swift` can read `ProcessInfo.processInfo.environment["MRT2_BUILD_DIR"]`
and sub-Makefiles get it without any extra passing. If the variable is not
exported, `swift build` launched from the `swift-player/` directory won't find
the library.
