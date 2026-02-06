# Shell Completions

Twig provides tab completion for commands, subcommands, flags, and dynamic values like branch names, Jira issue keys, and GitHub PR IDs.

## Setup

Twig supports two complementary completion mechanisms:

1. **Static completions** via `twig self completion <SHELL>` -- generates a shell script covering all commands and flags.
2. **Dynamic completions** via `CompleteEnv` -- provides runtime values (branches, Jira keys, PR IDs) when you press Tab.

### Bash

Add to `~/.bashrc`:

```bash
# Static completions
eval "$(twig self completion bash)"

# Dynamic completions (requires bash >= 4.4 with bash-completion >= 2.12)
source <(COMPLETE=bash twig)
```

### Zsh

Add to `~/.zshrc`:

```zsh
# Static completions
eval "$(twig self completion zsh)"

# Dynamic completions
source <(COMPLETE=zsh twig)
```

If you manage completions via a directory (e.g., `~/.zfunc`), you can write the output to a file instead:

```zsh
twig self completion zsh > ~/.zfunc/_twig
```

Then ensure `~/.zfunc` is in your `fpath` before calling `compinit`.

### Fish

```fish
# Static completions
twig self completion fish | source

# Dynamic completions
source (COMPLETE=fish twig | psub)
```

Or write to Fish's completions directory for auto-loading:

```fish
twig self completion fish > ~/.config/fish/completions/twig.fish
```

### PowerShell

Add to your PowerShell profile (`$PROFILE`):

```powershell
twig self completion powershell | Out-String | Invoke-Expression
```

## What Gets Completed

### Static completions

All subcommands and flags are covered. For example:

```
twig sw<Tab>        -> twig switch
twig self <Tab>     -> completion  diagnose  plugins  update
twig switch --<Tab> -> --no-create  --parent  --root
```

### Dynamic completions

Several commands complete argument values at runtime by reading local Git state and `.twig/state.json`:

| Command | Completed values |
| --- | --- |
| `twig switch` | Branch names, Jira issue keys, GitHub PR IDs |
| `twig branch depend` | Branch names |
| `twig branch rm-dep` | Branch names |
| `twig branch parent` | Branch names |
| `twig branch root add` | Branch names |
| `twig branch root remove` | Branch names |
| `twig adopt --parent` | Branch names |
| `twig flow` (plugin) | Branch names, Jira issue keys, GitHub PR IDs |

Dynamic candidates are:

- **Branch names** -- all local Git branches.
- **Jira issue keys** -- keys stored in `.twig/state.json` (e.g., `PROJ-123`).
- **GitHub PR IDs** -- PR numbers stored in `.twig/state.json`, offered with and without `#` prefix (e.g., `#42` and `42`).

Matching is case-insensitive and prefix-based: typing `feat` will match `feature/auth`, `Feature/login`, etc.

## How It Works

Twig uses the [`clap_complete`](https://docs.rs/clap_complete) crate with the `unstable-dynamic` feature.

- `twig self completion <SHELL>` calls `clap_complete::generate()` to emit a static script from the clap command tree.
- On startup, `CompleteEnv::with_factory(Cli::command).complete()` checks for the `COMPLETE` environment variable. When a shell sets this variable during tab completion, Twig prints candidate values and exits instead of running a normal command.
- Custom completers (`TargetCompleter`, `BranchCompleter`) implement the `ValueCompleter` trait and are attached to specific arguments via `#[arg(add = ...)]`.

### Key source files

| File | Role |
| --- | --- |
| `twig-cli/src/main.rs` | `CompleteEnv` initialization |
| `twig-cli/src/completion.rs` | Static completion generation (`twig self completion`) |
| `twig-cli/src/cli/completion.rs` | Clap subcommand definition for `twig self completion` |
| `twig-core/src/complete.rs` | `TargetCompleter`, `BranchCompleter`, candidate collection |
| `twig-cli/src/complete.rs` | Re-exports and backward-compatible aliases |

## Plugins

Plugins that use clap can integrate with the same `CompleteEnv` pattern. The `twig-flow` plugin demonstrates this -- it calls `CompleteEnv::with_factory(Cli::command).complete()` in its own entry point and attaches completers to its arguments.

See `plugins/twig-flow/src/complete.rs` for an example of a plugin providing its own dynamic completions.
