# System Test Plan: Batch NarInfo Querying

## Goal

Reliable, cross-platform system test for batch narinfo querying. The test runs end-to-end: populate a local binary cache, serve it via nix-serve-ng, proxy through selector4nix, send batch HTTP requests, verify responses.

## Architecture

```
selector4nix-system-test-nar-info-querying (Rust binary)
  │
  │ CLI args or env vars → discover:
  │   selector4nix, nix, nix-serve
  │
  ├─ 1. TempDir + write test files
  ├─ 2. nix store add-file --store "file://$cache?compression=none" × N
  │     → collect store path hashes
  ├─ 3. Start nix-serve --store "file://$cache" --port A (child process)
  ├─ 4. Write config.toml → Start selector4nix --config-file ... (child process)
  ├─ 5. Poll /nix-cache-info → wait for ready
  ├─ 6. Batch HTTP GET /{hash}.narinfo via reqwest
  ├─ 7. Assert responses
  └─ 8. TempDir + child processes cleanup (RAII)
```

Two levels of Nix packaging:

1. **Build wrapper** (normal derivation) — compiles the Rust binary + injects dependency paths via `makeWrapper`. Produces a self-contained executable. Placed in `passthru.tests`.
2. **Check FOD** (fixed-output derivation) — runs the build wrapper. On success, writes a fixed file to `$out`. On failure, aborts. Placed in `checks`, so `nix flake check` actually executes the test.

## File Changes

### New Files

| File | Purpose |
|------|---------|
| `tests/system/nar-info-querying/Cargo.toml` | Workspace member: narinfo querying test crate |
| `tests/system/nar-info-querying/src/main.rs` | Test binary: CLI, orchestration, assertions |
| `nix/system-test-nar-info-querying.nix` | Nix package: compile Rust binary + makeWrapper (normal derivation) |
| `nix/flake/check.nix` | FOD wrappers for `checks` output |
| `.github/workflows/ci.yml` | CI: build + run system test on Linux & macOS |

### Modified Files

| File | Change |
|------|--------|
| `Cargo.toml` | `workspace.members` add `"tests/system/*"`; `workspace.dependencies` add `tempfile` |
| `nix/package.nix` | Add `passthru.tests`, `cargoBuildFlags`, switch to `finalAttrs` pattern |
| `nix/flake/package.nix` | Expose `system-test-nar-info-querying` package |

### Directory Structure for Multiple System Tests

```
tests/system/
  nar-info-querying/          # First system test
    Cargo.toml
    src/
      main.rs
  # future-test/              # Future system tests follow same pattern
  #   Cargo.toml
  #   src/
  #     main.rs
```

Workspace `Cargo.toml` uses glob to include all:
```toml
[workspace]
members = [".", "components/*", "tests/system/*"]
```

## Rust Binary Design

### CLI Interface

```
selector4nix-system-test-nar-info-querying [OPTIONS]

Options:
  --selector4nix <PATH>   Path to selector4nix binary [env: SELECTOR4NIX_BIN, default: from PATH]
  --nix <PATH>            Path to nix binary [env: NIX_BIN, default: from PATH]
  --nix-serve <PATH>      Path to nix-serve binary [env: NIX_SERVE_BIN, default: from PATH]
```

Priority: CLI arg > env var > PATH discovery.

### Test Flow

1. **Create temp directory** — `tempfile::TempDir` for binary cache + config
2. **Write test files** — N files with varying content (e.g., short strings, empty, multi-line, binary-ish)
3. **Populate binary cache** — for each test file:
   ```
   nix store add-file --store "file://$cache?compression=none" <file>
   ```
   Parse stdout to extract store path → extract 32-char hash.
   Set `NIX_CONFIG=experimental-features=nix-command` when spawning `nix`.
4. **Allocate ports** — `TcpListener::bind("127.0.0.1:0")` × 2, get port numbers, drop listeners
5. **Start nix-serve-ng** — child process:
   ```
   nix-serve --store "file://$cache" --port <upstream_port>
   ```
6. **Start selector4nix** — write config TOML, then child process:
   ```
   selector4nix --config-file <config>
   ```
   Config content:
   ```toml
   [server]
   ip = "127.0.0.1"
   port = <proxy_port>

   [network]
   periodic_probing = false

   [[substituters]]
   url = "http://127.0.0.1:<upstream_port>/"
   ```
7. **Wait for ready** — poll `GET /nix-cache-info` on both ports, with timeout
8. **Run test cases** (see below)
9. **Cleanup** — `TempDir` auto-deleted on drop, child processes killed on drop

### Subprocess Management

```rust
struct SubprocessGuard { child: Child }

impl Drop for SubprocessGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
```

### Test Cases

| Case | Description | Assertion |
|------|-------------|-----------|
| `single_narinfo_fetch` | GET one valid hash | 200, body contains `StorePath:` and `URL:` |
| `batch_narinfo_fetch` | Concurrent GET all hashes | All 200, all bodies valid narinfo format |
| `not_found_narinfo` | GET random invalid hash (32 hex chars) | 404 |
| `cached_narinfo` | GET same hash twice | Both 200, both valid |
| `batch_mixed` | Mix of valid and invalid hashes | Valid → 200, invalid → 404 |

### Port Allocation Strategy

```rust
fn find_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}
```

There is a TOCTOU race between `drop` and the child process binding, but it is negligible in practice (CI runners have low port contention).

## Nix Packaging

Two distinct derivation types — do not confuse them.

### Derivation Type 1: Build Wrapper (normal derivation, NOT a FOD)

Compiles the Rust test binary and wraps it with dependency paths. Placed in `passthru.tests`. Building this derivation only verifies compilation; it does NOT run the test.

#### `tests/system/nar-info-querying/Cargo.toml`

```toml
[package]
name = "selector4nix-system-test-nar-info-querying"
version = "0.0.0"
edition = "2024"

[dependencies]
anyhow = { workspace = true }
clap = { workspace = true }
futures = { workspace = true }
reqwest = { workspace = true }
tokio = { workspace = true }
tempfile = { workspace = true }
```

#### `nix/system-test-nar-info-querying.nix`

```nix
{ runCommand, makeWrapper, rustPlatform
, selector4nix, nix, nix-serve-ng, src, cargoLock }:

let
  testBinary = rustPlatform.buildRustPackage {
    pname = "selector4nix-system-test-nar-info-querying-bin";
    version = "0.0.0";
    inherit src cargoLock;
    cargoBuildFlags = [ "-p" "selector4nix-system-test-nar-info-querying" ];
    doCheck = false;
  };
in
runCommand "system-test-nar-info-querying" {
  nativeBuildInputs = [ makeWrapper ];
} ''
  mkdir -p $out/bin
  makeWrapper ${testBinary}/bin/selector4nix-system-test-nar-info-querying \
    $out/bin/system-test-nar-info-querying \
    --set SELECTOR4NIX_BIN ${selector4nix}/bin/selector4nix \
    --set NIX_BIN ${nix}/bin/nix \
    --set NIX_SERVE_BIN ${nix-serve-ng}/bin/nix-serve
''
```

### Derivation Type 2: Check FOD (fixed-output derivation)

Runs the build wrapper. This IS a FOD — it has a declared `outputHash`, so Nix grants it relaxed sandbox restrictions (including network access, which allows binding localhost ports). Placed in `checks`, so `nix flake check` executes the test.

If the test passes: the derivation writes a fixed file to `$out` and succeeds.
If the test fails: the test binary exits non-zero, the derivation aborts, `nix flake check` reports failure.

The output is deterministic (always the same fixed file on success), so the FOD hash is stable.

#### `nix/flake/check.nix`

```nix
{ config, lib, ... }:
{
  perSystem = { config, pkgs, ... }:
  let
    wrapper = config.packages.selector4nix.passthru.tests.system-test-nar-info-querying;
  in
  {
    checks = {
      version = config.packages.selector4nix.passthru.tests.version;

      system-test-nar-info-querying = pkgs.runCommand "check-system-test-nar-info-querying" {
        outputHash = "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=";
        outputHashAlgo = "sha256";
        outputHashMode = "flat";
        nativeBuildInputs = [ wrapper ];
      } ''
        system-test-nar-info-querying
        touch $out
      '';
    };
  };
}
```

Note: `sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=` is the SRI hash of an empty file (the output of `touch $out`). Replace with actual hash after first build if needed.

### Summary of Derivation Types

| | Build Wrapper | Check FOD |
|---|---|---|
| **Purpose** | Compile + inject paths | Actually run the test |
| **Derivation type** | Normal | Fixed-output |
| **Sandbox** | Standard (no network) | Relaxed (network allowed) |
| **Located in** | `passthru.tests` | `checks` |
| **`nix flake check`** | Built as dependency of FOD | Built (runs test) |
| **`nix build .#system-test-nar-info-querying`** | Produces self-contained binary | Not directly accessible |
| **CI** | Not run separately | Included in `nix flake check` |

### `nix/package.nix` Changes

```nix
rustPlatform.buildRustPackage (finalAttrs: {
  pname = "selector4nix";
  version = "0.4.2";

  src = lib.fileset.toSource { /* ... unchanged ... */ };
  cargoLock = { lockFile = ../Cargo.lock; };

  cargoBuildFlags = [ "-p" "selector4nix" ];

  passthru.tests = {
    version = testers.testVersion { package = finalAttrs.finalPackage; };
    system-test-nar-info-querying = callPackage ./system-test-nar-info-querying.nix {
      selector4nix = finalAttrs.finalPackage;
      inherit rustPlatform;
      inherit (finalAttrs) src cargoLock;
    };
  };

  meta = { /* ... unchanged ... */ };
})
```

Key changes:
- Switch to `(finalAttrs: ...)` pattern to reference `finalAttrs.finalPackage`
- Add `cargoBuildFlags` to scope the build to the main package only
- Add `passthru.tests` with version check and system test build wrapper

### `nix/flake/package.nix` Changes

```nix
packages = {
  # ... existing packages ...

  system-test-nar-info-querying =
    config.packages.selector4nix.passthru.tests.system-test-nar-info-querying;
};
```

Exposes the build wrapper as a package for direct use: `nix build .#system-test-nar-info-querying` produces the self-contained test binary.

## CI

### `.github/workflows/ci.yml`

```yaml
name: ci
on:
  push:
    branches: [main]
  pull_request:

jobs:
  test:
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: DeterminateSystems/nix-installer-action@v16
      - uses: DeterminateSystems/magic-nix-cache-action@v9

      - run: nix flake check
      - run: nix build .#system-test-nar-info-querying
      - run: ./result/bin/system-test-nar-info-querying
```

- `nix flake check` — builds all checks including the FOD that runs the system test
- `nix build .#system-test-nar-info-querying` — build the self-contained test binary
- `./result/bin/system-test-nar-info-querying` — run the test outside Nix sandbox (redundant with `nix flake check` but useful for debugging)

## Coverage Matrix

| Test | Linux | macOS | Trigger |
|------|-------|-------|---------|
| `cargo test` (unit + integration) | `nix build` checkPhase | `nix build` checkPhase | `nix build` |
| `nix flake check` (FOD) | Runs system test | Runs system test | `nix flake check` |
| Build wrapper (manual run) | CI + local | CI + local | `nix build .#system-test-nar-info-querying` + execute |

## Key Details

- **`nix-serve-ng` executable name** is `nix-serve` (its `mainProgram`)
- **`nix store add-file`** requires `nix-command` experimental feature; set via `NIX_CONFIG=experimental-features=nix-command` env var when spawning `nix`
- **`file://` binary cache** stores narinfo + NAR files as plain files, no SQLite database needed
- **`compression=none`** in the store URI avoids needing xz/zstd in the test environment
- **`periodic_probing = false`** in test config avoids background noise from substituter probing
- **FOD hash** is the hash of an empty file (`touch $out`). Since the output is always the same on success, the hash is stable. If the test logic changes the output, update the hash accordingly
- **FOD sandbox relaxation** allows the test to bind localhost ports and start subprocesses, which is required for the end-to-end HTTP test

## Development Workflow

### Prerequisites

The devshell must provide `nix-serve-ng` (and `nix` if not already available from the Lix installation):

```nix
# nix/flake/devshell.nix changes
devShells.default = pkgs.mkShellNoCC {
  packages = [
    config.packages.rust-toolchain
    pkgs.nixfmt
    pkgs.nixfmt-tree
    pkgs.nix-serve-ng
  ];
};
```

### Running the System Test in Dev Environment

Each system test crate has a `run.sh` at its root for quick iteration:

```
tests/system/nar-info-querying/
  Cargo.toml
  src/
    main.rs
  run.sh              # Dev entry point
```

#### `tests/system/nar-info-querying/run.sh`

```sh
#!/bin/sh
set -e
cargo build -p selector4nix "$@"
exec cargo run -p selector4nix-system-test-nar-info-querying -- \
  --selector4nix ./target/debug/selector4nix
```

Usage in devshell:

```sh
# Build selector4nix + run system test
./tests/system/nar-info-querying/run.sh

# Or release mode
./tests/system/nar-info-querying/run.sh --release
# Note: release binary path would need adjustment, or use --selector4nix explicitly

# Or manual, step by step:
cargo build -p selector4nix
cargo run -p selector4nix-system-test-nar-info-querying -- \
  --selector4nix ./target/debug/selector4nix
```

The test binary discovers `nix` and `nix-serve` from PATH (provided by devshell). Only `--selector4nix` needs explicit path because `cargo build` outputs to `./target/debug/`, which is not on PATH.

### Justfile Integration (Optional)

If the project adopts `just`, add recipes for each system test:

```just
# Justfile
build:
  cargo build -p selector4nix

system-test-nar-info-querying: build
  cargo run -p selector4nix-system-test-nar-info-querying \
    -- --selector4nix ./target/debug/selector4nix

system-test-nar-info-querying-release: (build "--release")
  cargo run -p selector4nix-system-test-nar-info-querying \
    -- --selector4nix ./target/release/selector4nix
```

This provides `just system-test-nar-info-querying` as the shortest entry point.

## Future System Tests

To add a new system test:

1. Create `tests/system/new-test-name/Cargo.toml` + `src/main.rs` + `run.sh`
2. Create `nix/system-test-new-test-name.nix` (build wrapper, same pattern)
3. Add to `passthru.tests` in `nix/package.nix`
4. Add FOD to `checks` in `nix/flake/check.nix`
5. Optionally expose as package in `nix/flake/package.nix`
6. Optionally add `just` recipe in `Justfile`
