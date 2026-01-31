# Shell Completions

Twig provides comprehensive shell completion support to enhance your command-line experience. This includes both static completion scripts for command and option completion, and dynamic runtime completions that provide context-aware suggestions for branch names, Jira issues, and GitHub PRs.

## Quick Setup

Generate and install a completion script for your shell:

### Bash

```bash
twig self completion bash > ~/.twig-completion.bash
echo 'source ~/.twig-completion.bash' >> ~/.bashrc
source ~/.bashrc
```

### Zsh

```bash
twig self completion zsh > ~/.twig-completion.zsh
echo 'source ~/.twig-completion.zsh' >> ~/.zshrc
source ~/.zshrc
```

### Fish

```bash
twig self completion fish > ~/.config/fish/completions/twig.fish
```

### PowerShell

```powershell
twig self completion powershell >> $PROFILE
```

## How It Works

Twig uses a two-layer completion system:

### 1. Static Completion Scripts

The `twig self completion <shell>` command generates a shell script that registers twig with your shell's completion system. This script provides:

- Command and subcommand completion (`twig sw<TAB>` → `twig switch`)
- Option and flag completion (`twig switch --<TAB>` → `--root`, `--verbose`, etc.)
- Help text for commands and options

Unlike some tools that require `eval` at shell startup, twig generates a static script file. This has two advantages:

1. **Security**: You can inspect the script before sourcing it
2. **Performance**: No runtime overhead at shell startup

### 2. Dynamic Runtime Completions

When you tab-complete command arguments, twig provides context-aware suggestions in real time. These completions are generated dynamically from:

- **Local Git branches**: All branches in your current repository
- **Jira issue keys**: Issues linked to branches (from `.twig/state.json`)
- **GitHub PR IDs**: Pull requests linked to branches (from `.twig/state.json`)

Dynamic completions are powered by `clap_complete` with the `CompleteEnv` runtime, activated automatically when your shell requests completions.

## Commands with Dynamic Completion

The following commands support intelligent argument completion:

| Command | Completions Provided |
|---------|---------------------|
| `twig switch <target>` | Branches, Jira keys, PR IDs |
| `twig branch depend <child> <parent>` | Branches only |
| `twig branch remove-dep <child> <parent>` | Branches only |
| `twig branch parent <branch>` | Branches only |
| `twig adopt --parent <branch>` | Branches only |
| `twig-flow <target>` | Branches, Jira keys, PR IDs |

### Example: Switch Command

```bash
# Tab-complete branch names
twig switch feat<TAB>
# → feature/authentication
# → feature/dashboard
# → feature/user-profile

# Tab-complete Jira issues
twig switch PROJ<TAB>
# → PROJ-123
# → PROJ-456
# → PROJ-789

# Tab-complete PR IDs (with or without #)
twig switch #12<TAB>
# → #123
# → #124

twig switch 12<TAB>
# → 123
# → 124
```

## Completion Behavior

### Case-Insensitive Matching

Completions are case-insensitive, so typing `feat`, `FEAT`, or `Feat` will all match `feature/my-branch`.

### PR ID Formats

GitHub PR IDs are suggested both with and without the `#` prefix:
- `#123` - Explicit PR format
- `123` - Numeric-only format

Both formats work with `twig switch` and other commands that accept PR IDs.

### Candidate Deduplication

If a branch name appears in multiple sources (e.g., a branch named `PROJ-123` that also happens to be a Jira key), it will only appear once in the completion list.

## Troubleshooting

### Completions not working

1. **Verify the completion script is sourced**: Check your shell's startup file (`.bashrc`, `.zshrc`, etc.) includes the source line.

2. **Regenerate the script**: If you've updated twig, regenerate the completion script:
   ```bash
   twig self completion bash > ~/.twig-completion.bash
   ```

3. **Start a new shell session**: Completion changes require a new shell session or explicit re-sourcing.

### Dynamic completions showing stale data

Dynamic completions read from your repository's `.twig/state.json` file. Run `twig sync` to refresh the branch-to-issue mappings:

```bash
twig sync
```

### Performance

Completions are designed to be fast (sub-100ms). The Rust implementation avoids the startup latency common with interpreted-language CLIs. If completions feel slow, check if:

- The repository has an unusually large number of branches
- The `.twig/state.json` file has become very large

## Architecture

For developers interested in the implementation:

- **Static generation**: `twig-cli/src/completion.rs` uses `clap_complete::generate`
- **Dynamic completers**: `twig-core/src/complete.rs` provides `TargetCompleter` and `BranchCompleter`
- **CLI integration**: Commands use `#[arg(add = target_completer())]` to attach completers

The completion system is built on the `clap_complete` crate with the `unstable-dynamic` feature, which enables the `CompleteEnv` runtime for dynamic argument completion.

## Plugin Support

Plugins can also provide dynamic completions. The `twig-flow` plugin demonstrates this by re-exporting the completion utilities from `twig-core`:

```rust
use twig_core::complete::{target_completer, TargetCompleter};
```

Plugin authors can use the same `TargetCompleter` and `BranchCompleter` types to provide consistent completion behavior across the twig ecosystem.
