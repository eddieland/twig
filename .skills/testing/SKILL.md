---
name: testing
description: >-
  Run tests for the twig Rust workspace. Use when running unit tests, integration
  tests, snapshot tests, code coverage, or working with insta snapshots. Always
  uses cargo-nextest, never cargo test.
---

# Testing

## Critical rule

**Never use `cargo test`.** Always use `cargo nextest run`. This is enforced by
CI and the Makefile.

**Exception:** Doc tests require `cargo test --doc` since nextest does not
support them:

```
cargo test -p twig-core --doc   # Run doc tests for a crate
```

## Quick reference

| Task | Command |
|---|---|
| Run all tests | `make test` |
| Run all tests (all features) | `make test-all` |
| Run tests in watch mode | `make watch-test` |
| Run a specific test | `cargo nextest run --workspace -E 'test(name)'` |
| Run tests for one crate | `cargo nextest run -p <crate-name>` |
| Code coverage | `make coverage` |
| Coverage HTML report | `make coverage-html` |

## Running all tests

```
make test
```

This first runs `make build`, then `cargo nextest run --workspace`.

## Running specific tests

By test name (substring match):

```
cargo nextest run --workspace -E 'test(test_branch_depend)'
```

By crate:

```
cargo nextest run -p twig-core
cargo nextest run -p twig-cli
cargo nextest run -p twig-gh
```

By test file (for integration tests in `tests/`):

```
cargo nextest run --test basic_test
cargo nextest run --test rebase_cascade_test
```

## Snapshot testing (insta)

The project uses `insta` for snapshot tests. Snapshots are stored alongside test
files.

### Workflow

1. Run tests — new or changed snapshots create `.snap.new` files
2. Review pending changes:
   ```
   make insta-review
   ```
3. Accept all:
   ```
   make insta-accept
   ```
4. Or reject all:
   ```
   make insta-reject
   ```

### Updating all snapshots at once

```
make update-snapshots
```

This runs tests with `INSTA_UPDATE=1` which auto-accepts all snapshot changes.

## Code coverage

```
make coverage            # Terminal summary
make coverage-html       # HTML report
make coverage-open       # HTML report + open in browser
make coverage-report     # LCOV output to lcov.info
```

Coverage uses `cargo llvm-cov nextest` (requires `cargo-llvm-cov` from
`make install-dev-tools`).

## Test utilities

Shared test helpers live in `twig-test-utils`. Import them in dev-dependencies:

```toml
[dev-dependencies]
twig-test-utils = { path = "twig-test-utils" }
```

Available helpers:

| Helper | Purpose |
|---|---|
| `setup_test_env()` | Create isolated XDG config dirs |
| `setup_test_env_with_init()` | XDG dirs + initialized twig config |
| `setup_test_env_with_registry()` | XDG dirs + pre-populated registry |
| `GitRepoTestGuard` | Temp Git repo that cleans up on drop |
| `create_branch()` | Create a branch in a test repo |
| `create_commit()` | Create a commit with file content |
| `checkout_branch()` | Switch branches in a test repo |
| `NetrcGuard` | Mock `.netrc` credentials |

### Example test

```rust
use twig_test_utils::{GitRepoTestGuard, create_commit, create_branch};

#[test]
fn test_my_feature() {
    let repo_guard = GitRepoTestGuard::new();
    let repo = &repo_guard.repo;

    create_commit(repo, "file.txt", "content", "Initial commit").unwrap();
    create_branch(repo, "feature-branch").unwrap();

    // ... test twig logic against this repo
}
```

## Integration tests

Integration tests live in the `tests/` directory at the workspace root:

- `basic_test.rs` — core functionality
- `batch_operations_test.rs` — batch operations
- `jira_strict_mode_test.rs` — Jira strict mode
- `rebase_cascade_test.rs` — cascade rebase flows
- `xdg_override_test.rs` — config directory overrides

## Tips

- **Parallel by default**: nextest runs tests in parallel. If tests interfere
  with each other, ensure they use isolated `GitRepoTestGuard` instances.
- **Flaky tests**: Re-run with `--retries 2` to identify flaky tests.
- **Verbose output**: `cargo nextest run --workspace -- --nocapture` to see
  `println!` / `tracing` output from tests.
- **Filter syntax**: nextest uses `-E` for filter expressions. See
  [nextest filter docs](https://nexte.st/docs/filtersets/) for full syntax.
