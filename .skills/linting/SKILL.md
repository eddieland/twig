---
name: linting
description: >-
  Lint and format twig Rust code. Use when running clippy, rustfmt, checking
  code style, fixing warnings, or formatting. Covers cargo clippy, cargo fmt,
  and Make targets for the twig workspace.
---

# Linting & Formatting

## Quick reference

| Task | Command |
|---|---|
| Auto-format + auto-fix | `make fmt` |
| Lint (deny warnings) | `make lint` |
| Lint all features | `make lint-all` |

## Formatting (`make fmt`)

```
make fmt
```

This runs, in order:

1. `cargo fmt --all` — Rust formatting
2. `cargo clippy --fix --allow-dirty --workspace` — auto-fix clippy lints
3. Ruff format + check on `examples/plugins/twig-backup` (Python plugin
   examples)

Always run `make fmt` before committing. It handles both Rust and Python files.

## Linting (`make lint`)

```
make lint
```

Runs `cargo clippy --workspace -- -D warnings`. All warnings are treated as
errors. This is what CI enforces.

To lint with all feature flags enabled:

```
make lint-all
```

Runs `cargo clippy --all-features -- -D warnings`.

## Running clippy on a single crate

```
cargo clippy -p twig-core -- -D warnings
cargo clippy -p twig-cli -- -D warnings
```

## Common clippy fixes

When clippy reports issues, try auto-fix first:

```
cargo clippy --fix --allow-dirty -p <crate-name>
```

If auto-fix can't resolve it, address the lint manually. Common categories:

- **`needless_return`** — Remove explicit `return` at end of functions
- **`unused_imports`** — Remove dead `use` statements
- **`clone_on_copy`** — Use copy instead of `.clone()` for Copy types
- **`redundant_closure`** — Pass function directly instead of wrapping in
  closure

## Workspace-level lint bans

The workspace `Cargo.toml` explicitly **denies** these lints — clippy will
treat them as errors when you run `make lint`:

| Lint | What to do instead |
|---|---|
| `unwrap_used` | Use `?` or explicit error handling |
| `panic` | Return an error; never panic in library code |
| `print_stdout` | Use `twig_core::output::print_*` functions |
| `print_stderr` | Use `twig_core::output::print_*` functions |
| `dbg_macro` | Remove `dbg!()` before committing |
| `todo` | No unfinished stubs in committed code |
| `unimplemented` | No unfinished stubs in committed code |

## Pre-commit hooks

The project supports pre-commit hooks:

```
make pre-commit-setup    # Install hooks (one-time)
make pre-commit-run      # Run manually on all files
```

## Toolchain note

The project uses Rust **nightly** with clippy and rustfmt components pinned in
`rust-toolchain.toml`. No need to install them separately — `rustup` handles it
automatically.
