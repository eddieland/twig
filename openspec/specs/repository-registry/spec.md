# Repository Registry

## Purpose

Track and manage multiple git repositories in a global registry. Enables cross-repo operations like fetching all repos,
executing git commands across repos, and maintaining a central inventory of projects the user works with.

**CLI surface:** `twig git add/remove/list/exec/fetch`, flags: `-a/--all`, `-r/--repo` **Crates:** `twig-core`
(state::Registry), `twig-cli` (git command module)

## Requirements

### Requirement: Registry storage

#### Scenario: Registry file location

WHEN twig reads or writes the repository registry THEN it uses the file at `${XDG_DATA_HOME}/twig/registry.json` AND the
file contains a JSON array of Repository objects with `path`, `name`, and `last_fetch` fields

#### Scenario: Path canonicalization

WHEN a repository path is provided to any registry operation THEN the path is resolved to an absolute path using
`fs::canonicalize()` which resolves symlinks AND worktree paths are resolved to the main repository path via
`resolve_to_main_repo_path()`

### Requirement: Adding a repository (`twig git add`)

#### Scenario: Adding the current directory

WHEN the user runs `twig git add` without specifying a path THEN the current working directory is used as the repository
path AND the path is canonicalized and resolved to the main repo if it is a worktree AND the repository is added to the
registry with its directory basename as the name AND a `.twig/.gitignore` file is created with `*\n` content AND no
output is printed on success

#### Scenario: Adding a specific path

WHEN the user runs `twig git add <path>` THEN the specified path is canonicalized and resolved to the main repo if it is
a worktree AND the repository is added to the registry with the directory basename as the name AND a `.twig/.gitignore`
file is created with `*\n` content AND no output is printed on success

#### Scenario: Adding a repository that is already registered

WHEN the user runs `twig git add` AND the canonicalized path matches an existing entry in the registry (exact string
comparison after canonicalization) THEN the command returns successfully without adding a duplicate AND no output is
printed

### Requirement: Removing a repository (`twig git remove`)

#### Scenario: Removing the current directory

WHEN the user runs `twig git remove` without specifying a path THEN the current working directory is canonicalized and
resolved to the main repo if it is a worktree AND the matching entry is removed from the registry

#### Scenario: Removing a specific path

WHEN the user runs `twig git remove <path>` THEN the specified path is canonicalized and resolved to the main repo if it
is a worktree AND the matching entry is removed from the registry

#### Scenario: Removing a repository that is not in the registry

WHEN the user runs `twig git remove` AND the resolved path does not match any entry in the registry THEN the command
returns successfully without error AND no output is printed (silent, idempotent removal)

#### Scenario: Using the `rm` alias

WHEN the user runs `twig git rm` THEN it behaves identically to `twig git remove` with the same arguments

### Requirement: Listing repositories (`twig git list`)

#### Scenario: Listing when repositories are registered

WHEN the user runs `twig git list` AND there are repositories in the registry THEN a "Tracked Repositories" header is
displayed AND each repository is shown as `<name> (<path>)` with colors

#### Scenario: Listing when no repositories are registered

WHEN the user runs `twig git list` AND the registry is empty THEN a warning indicating no repositories are registered is
printed AND guidance is printed about using `twig git add` to register repositories

#### Scenario: Using the `ls` alias

WHEN the user runs `twig git ls` THEN it behaves identically to `twig git list`

### Requirement: Fetching repositories (`twig git fetch`)

#### Scenario: Fetching a single repo from the current directory

WHEN the user runs `twig git fetch` without `-a` or `-r` THEN the repository is detected from the current working
directory AND `git fetch` is executed for that repository AND the `last_fetch` timestamp is updated in the registry to
the current time in RFC3339 UTC format

#### Scenario: Fetching a specific repo with `-r`

WHEN the user runs `twig git fetch -r <path>` THEN the repository at the specified path is fetched AND the `last_fetch`
timestamp is updated in the registry AND the `update_fetch_time` function resolves worktree paths internally via
`resolve_to_main_repo_path`

#### Scenario: Fetching all registered repos

WHEN the user runs `twig git fetch -a` THEN all repositories in the registry are fetched in parallel via tokio with a
100ms stagger between task spawns AND each repository's `last_fetch` timestamp is updated on success AND a summary is
printed showing "Successful: N" AND if any failures occurred, "Failed: N" is also shown

#### Scenario: Fetch failure isolation in multi-repo mode

WHEN the user runs `twig git fetch -a` AND one or more repositories fail to fetch THEN the remaining repositories
continue to be fetched AND failures are reported individually AND the aggregated success/failure counts are shown in the
summary

### Requirement: Executing commands across repos (`twig git exec`)

#### Scenario: Executing a command in the current directory

WHEN the user runs `twig git exec <command>` without `-a` or `-r` THEN the command is split on whitespace with the first
token as the program and remaining tokens as arguments AND the command is executed in the current repository's directory
AND stdout and stderr are printed AND success or failure is reported with the exit code

#### Scenario: Executing a command with no program specified

WHEN the user runs `twig git exec` AND the command string resolves to no program THEN "git" is used as the default
program

#### Scenario: Executing a command in a specific repo with `-r`

WHEN the user runs `twig git exec -r <path> <command>` THEN the command is executed with `current_dir` set to the
specified repository path AND stdout and stderr are printed AND success or failure is reported with the exit code

#### Scenario: Executing a command in all registered repos

WHEN the user runs `twig git exec -a <command>` THEN the command is executed in all registered repositories in parallel
via tokio with a 100ms stagger between task spawns AND stdout and stderr are printed for each repository AND success or
failure is reported per repository with exit codes

#### Scenario: Exec failure isolation in multi-repo mode

WHEN the user runs `twig git exec -a <command>` AND one or more repositories fail THEN the remaining repositories
continue to execute the command AND failures are reported individually AND aggregated success/failure counts are shown
in the summary

### Requirement: Worktree resolution differences

#### Scenario: Add and remove resolve worktrees to main repo

WHEN the user runs `twig git add` or `twig git remove` from within a git worktree THEN the path is resolved to the main
repository via `resolve_to_main_repo_path` before any registry lookup or modification

#### Scenario: Fetch and exec detect repository without worktree resolution

WHEN the user runs `twig git fetch` or `twig git exec` with a path THEN the repository is detected via
`detect_repository_from_path` which does NOT resolve worktree paths to the main repo AND the command operates on the
detected repository path directly

#### Scenario: Fetch update_fetch_time resolves worktrees internally

WHEN `twig git fetch` updates the `last_fetch` timestamp in the registry THEN the `update_fetch_time` function
internally resolves worktree paths to the main repo via `resolve_to_main_repo_path` AND this ensures the correct
registry entry is updated regardless of whether the fetch was initiated from a worktree
