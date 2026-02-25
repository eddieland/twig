---
name: installing
description: >-
  Install twig development builds locally. Use when installing the twig binary
  from source, setting up dev tools like nextest and cargo-watch, or installing
  the twig-flow plugin. Covers cargo install and Make targets.
---

# Installing Development Builds

## Installing twig locally

```
make install
```

This runs `cargo install --path twig-cli`, which compiles a release build and
places the `twig` binary in `~/.cargo/bin/` (on your PATH).

After installation, verify:

```
twig --version
```

## Installing the flow plugin

```
make install-flow-plugin
```

Runs `cargo install --path plugins/twig-flow`.

## Installing development tools

```
make install-dev-tools
```

This installs all required dev tooling:

| Tool | Purpose |
|---|---|
| `cargo-nextest` | Test runner (required — never use `cargo test`) |
| `cargo-watch` | File watcher for `make watch-test` |
| `cargo-outdated` | Dependency staleness checker |
| `cargo-llvm-cov` | Code coverage via `make coverage` |
| `cargo-insta` | Snapshot test management |
| `pre-commit` (via uv) | Git pre-commit hooks |

It also runs `rustup show` to ensure the nightly toolchain from
`rust-toolchain.toml` is installed.

## Debug binary vs installed binary

There are two ways to run twig from source:

### Debug binary (fast iteration)

```
cargo build -p twig
# Binary at: target/debug/twig.exe
```

Use this during development. Fastest compile times but slower runtime.

### Installed binary (PATH)

```
make install
# Binary at: ~/.cargo/bin/twig.exe
```

Use this when you want `twig` on your PATH to behave like a release build.
**The PATH binary is only updated when you explicitly run `make install` or
`cargo install`** — it does NOT auto-update after `cargo build`.

## Workflow recommendation

During development:

1. Make code changes
2. `cargo build -p twig` (or `make check` for type-checking only)
3. Test with `target/debug/twig.exe` or the manual-testing skill
4. When satisfied, `make install` to update the PATH binary
