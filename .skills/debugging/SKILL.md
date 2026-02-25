---
name: debugging
description: >-
  Debug and investigate twig issues using logging, tracing, and diagnostics.
  Use when troubleshooting unexpected behavior, investigating bugs, adding
  tracing instrumentation to code, understanding error messages, or running
  system diagnostics. Covers verbosity flags, RUST_LOG, tracing macros, and
  twig self diagnose.
---

# Debugging with Logging

Twig uses the `tracing` crate for structured logging. Verbosity is controlled
via the `-v` flag or the `RUST_LOG` environment variable.

## Verbosity levels

| Flag | Level | What you see |
|---|---|---|
| (none) | WARN | Warnings and errors only |
| `-v` | INFO | High-level operation progress |
| `-vv` | DEBUG | Internal decision-making, state changes |
| `-vvv` | TRACE | Everything, including per-commit data |

### Examples

```
twig tree -v              # Info: which branches are processed
twig cascade -vv          # Debug: rebase decisions per branch
twig switch PROJ-123 -vvv # Trace: full Jira/GitHub API flow
```

## Using RUST_LOG for granular control

The `-v` flag sets a global level. For finer control, use `RUST_LOG` to target
specific crates or modules:

```powershell
# All twig crates at debug, everything else at warn
$env:RUST_LOG = "warn,twig_cli=debug,twig_core=debug"
twig cascade

# Trace only the cascade module
$env:RUST_LOG = "warn,twig_cli::cli::cascade=trace"
twig cascade

# Trace the rebase module specifically
$env:RUST_LOG = "warn,twig_cli::cli::rebase=trace"
twig rebase

# Debug the switch command's Jira/GitHub lookups
$env:RUST_LOG = "warn,twig_cli::cli::switch=debug"
twig switch PROJ-123

# Full trace for MCP server debugging
$env:RUST_LOG = "debug,twig_mcp=trace"
twig mcp-server
```

Both `-v` and `RUST_LOG` work simultaneously — `RUST_LOG` takes precedence for
modules it targets. The tracing subscriber is initialized with
`EnvFilter::from_default_env()` combined with the `-v` level.

## Adding tracing to code

When investigating a bug, add temporary tracing statements to narrow down the
issue.

### Tracing macros

```rust
use tracing::{trace, debug, info, warn, error};

// High-level operation progress (visible with -v)
info!("Starting cascade from branch: {}", branch_name);

// Internal logic decisions (visible with -vv)
debug!("Branch {} has {} children to process", branch, count);
debug!(?repo_state, "Loaded repository state");  // Debug-format with ?

// Detailed per-item data (visible with -vvv)
trace!("Processing commit: {} by {}", hash, author);
trace!(?dependency_map, "Resolved dependencies");

// Problems that don't stop execution
warn!("Failed to store Jira association: {}", e);

// Problems that stop execution
error!("Rebase failed for branch {}: {}", branch, e);
```

### Structured fields

Use structured fields for machine-parseable output:

```rust
debug!(branch = %name, parent = %parent, depth = depth, "Rebasing branch");
trace!(commit_hash = %hash, author = %author, "Processing commit");
```

### Spans for scoped context

Wrap operations in spans to group related log lines:

```rust
use tracing::{info_span, debug_span};

let span = info_span!("cascade", branch = %start_branch);
let _guard = span.enter();

// All tracing inside this scope shows the cascade context
debug!("Processing child: {}", child);
```

## User-facing output vs tracing

Twig separates user-facing messages from debug logging:

| Purpose | Mechanism |
|---|---|
| User-facing success/error/info | `twig_core::output::{print_success, print_error, print_warning, print_info}` |
| Developer diagnostics | `tracing::{trace!, debug!, info!, warn!, error!}` |

**Rule**: User-facing functions (`print_*`) always display regardless of
verbosity. Tracing macros are filtered by `-v` / `RUST_LOG`.

When debugging, use tracing macros so output only appears when verbosity is
increased. Don't use `println!` or `eprintln!` for debug output.

## Enhanced error messages

Twig has a structured error system in `twig-cli/src/enhanced_errors.rs`. Errors
include:

- **Category** (GitRepository, BranchOperation, Network, Configuration, etc.)
- **Message** — what went wrong
- **Details** — technical context (logged at debug level)
- **Suggestions** — actionable fix recommendations shown to the user

When investigating an error, run the failing command with `-vv` to see the
`details` field that may not be shown at default verbosity.

## System diagnostics

Run a full health check:

```
twig self diagnose
```

This checks:
- System information
- Configuration directories and files
- Credentials (Jira, GitHub .netrc)
- Git configuration
- Tracked repositories
- Required dependencies

## Debugging workflow

### Step 1: Reproduce with verbose output

```
twig <command> -vv 2>&1 | Out-File debug-output.txt
```

### Step 2: Narrow to a specific module

```powershell
$env:RUST_LOG = "warn,twig_cli::cli::<module>=trace"
twig <command> 2>&1 | Out-File trace-output.txt
```

### Step 3: Add targeted tracing

If logs aren't sufficient, add `debug!()` / `trace!()` calls at key decision
points in the code, rebuild, and re-run:

```
cargo build -p twig
twig <command> -vvv
```

### Step 4: Check state files

Inspect twig's persisted state:

```powershell
# Repo-local state
Get-Content ".twig\state.json" | ConvertFrom-Json | ConvertTo-Json -Depth 10

# Global registry
$dataDir = [Environment]::GetFolderPath("LocalApplicationData")
Get-Content "$dataDir\twig\registry.json" | ConvertFrom-Json | ConvertTo-Json -Depth 10
```

### Step 5: Check git state

```
git status
git log --oneline --graph --all | Select-Object -First 20
git reflog | Select-Object -First 10
```

## Tips

- **Always use `-vv` first** before adding code changes — the existing tracing
  often reveals the issue.
- **Clean up tracing before committing** — remove temporary `debug!()` calls
  added for investigation. Permanent tracing should be intentional.
- **Pipe stderr**: Tracing output goes to stderr. Use `2>&1` to capture both
  stdout and stderr when saving to a file.
- **RUST_LOG syntax**: Use commas to separate targets
  (`target1=level,target2=level`). Module paths use `::` not `/`.
