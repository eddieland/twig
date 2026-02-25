---
name: building
description: >-
  Build the twig Rust workspace. Use when compiling the project, building
  individual crates, creating release binaries, or checking for compilation
  errors. Covers cargo build, cargo check, and Make targets for the twig
  workspace.
---

# Building

## Quick reference

| Task | Command |
|---|---|
| Full workspace build (debug) | `make build` or `cargo build --workspace` |
| Single crate | `cargo build -p <crate-name>` |
| Type-check only (fast) | `make check` or `cargo check --workspace` |
| Release build | `make release` or `cargo build --release --workspace` |

## Workspace structure

This is a Cargo workspace. The root `Cargo.toml` defines these members:

- `twig` — binary crate (re-exports `twig-cli`)
- `twig-cli` — main CLI logic (default member)
- `twig-core` — shared state, config, output
- `twig-gh` — GitHub client
- `twig-jira` — Jira client
- `twig-mcp` — MCP server
- `twig-test-utils` — test helpers (dev only)
- `no-worries` — panic handler
- `plugins/twig-flow` — flow plugin

The `default-members` is `["twig-cli"]`, so a bare `cargo build` only builds
`twig-cli`. Use `--workspace` to build everything.

## Building a single crate

Always use `-p` to stay within the workspace context:

```
cargo build -p twig-core
cargo build -p twig-gh
cargo build -p twig-cli
```

Never `cd` into a crate directory and run `cargo build` there — use `-p` from
the workspace root.

## Type-checking (fastest feedback)

```
make check
```

This runs `cargo check --workspace` which validates types without producing
binaries — much faster than a full build.

## Debug build

```
make build
```

The resulting binary is at `target/debug/twig.exe`.

## Release build

```
make release
```

The release profile enables LTO and single codegen unit for maximum
optimization. The binary is at `target/release/twig.exe`.

## Toolchain

The project uses Rust **nightly** (pinned in `rust-toolchain.toml`) with
edition 2024 and MSRV 1.91.0. Components: clippy, rustfmt, rust-src,
llvm-tools-preview.

## CI profiles

Two additional Cargo profiles exist for CI:

- `ci-windows` — inherits dev, zero optimization, max parallelism (fastest
  compile)
- `release-windows` — inherits release with thin LTO and more codegen units
  (balanced)

Build with: `cargo build --profile ci-windows --workspace`

## Troubleshooting

- **Linker errors on Windows**: The `git2` crate uses `vendored-openssl` on
  Windows targets. If you see OpenSSL errors, ensure the vendored feature is
  active.
- **Slow builds**: Use `make check` for type-checking feedback. Only do full
  builds when you need to run the binary.
- **Stale binary**: After code changes, always rebuild before testing. The
  PATH-installed binary (`~/.cargo/bin/twig`) is only updated by
  `cargo install --path twig-cli`.
