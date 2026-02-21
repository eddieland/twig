# Worktrees

## Purpose

Manage git worktrees for parallel multi-branch development. Create worktrees linked to branches, list active worktrees,
and clean up stale ones. Integrates with branch creation flows (e.g., `twig jira create-branch --with-worktree`).

**CLI surface:** `twig worktree create/list/clean` (alias `wt`) **Crates:** `twig-core` (state, git ops), `twig-cli`
(worktree command module)

## Requirements

### Requirement: Repository resolution

Repository resolution follows the shared behavior defined in `repository-resolution/spec.md`. This command uses the
`--repo` (or `-r`) flag for the repository path override.

### Requirement: Worktree path convention

#### Scenario: Determining the worktree directory location

WHEN a worktree is created for a repository at `<repo_path>` THEN the worktree is placed at
`<repo_path>/../<repo_name>-worktrees/<sanitized_branch_name>` where `<repo_name>` is the final component of the
repository path AND the `-worktrees` directory is created if it does not already exist

#### Scenario: Sanitizing branch names for directory paths

WHEN the branch name contains forward slashes (e.g., `feature/my-branch`) THEN all `/` characters are replaced with `-`
to produce a safe directory name (e.g., `feature-my-branch`) AND this sanitized name is used as both the worktree
directory name and the worktree identifier in git

### Requirement: Creating a worktree for an existing branch

#### Scenario: Branch already exists locally

WHEN the user runs `twig worktree create <branch>` AND the branch already exists as a local branch THEN the command
indicates it is using the existing branch AND creates a linked worktree at the conventional path using the existing
branch AND records the worktree in the repository state AND prints a success message with the branch name and worktree
path

#### Scenario: Worktree directory already exists on disk

WHEN the user runs `twig worktree create <branch>` AND the computed worktree path already exists on disk THEN the
command fails with an error indicating the directory already exists AND advises the user to remove it or use a different
branch name

#### Scenario: A git worktree with the sanitized name is already registered

WHEN the user runs `twig worktree create <branch>` AND git already has a worktree registered with the sanitized branch
name THEN the command fails with an error indicating a worktree with that name already exists

#### Scenario: A branch with the sanitized name conflicts with the worktree name

WHEN the user runs `twig worktree create <branch>` AND the branch exists AND a separate branch with the sanitized name
already exists (e.g., creating worktree for `feature/foo` when branch `feature-foo` also exists) THEN the command fails
with an error indicating the name conflict AND advises the user to delete the conflicting branch or use a different name

### Requirement: Creating a worktree for a new branch

#### Scenario: Branch does not exist locally

WHEN the user runs `twig worktree create <branch>` AND the branch does not exist as a local branch THEN the command
indicates it is creating a new branch AND creates a new branch from HEAD AND creates a linked worktree at the
conventional path AND records the worktree in the repository state AND prints a success message with the branch name and
worktree path

#### Scenario: HEAD is not a direct reference when creating a new branch

WHEN the user runs `twig worktree create <branch>` AND the branch does not exist AND HEAD is not a direct reference THEN
the command fails with an error indicating HEAD is not a direct reference

### Requirement: Worktree state recording

#### Scenario: Persisting worktree metadata after creation

WHEN a worktree is successfully created THEN a `Worktree` entry is added to the repository state in `.twig/state.json`
with fields: `name` (sanitized branch name), `path` (absolute worktree directory path), `branch` (original branch name),
and `created_at` (RFC 3339 timestamp) AND the state is saved to disk

#### Scenario: Replacing a duplicate worktree entry in state

WHEN a worktree is added to the state AND an entry with the same `name` already exists THEN the existing entry is
removed before the new entry is added AND only the new entry is retained

### Requirement: Listing worktrees

#### Scenario: Repository has active worktrees

WHEN the user runs `twig worktree list` AND the repository has one or more git worktrees THEN the command prints a
"Worktrees" header AND for each worktree displays: the branch name, the worktree path, and the creation timestamp (if
metadata is available in the state)

#### Scenario: Repository has no worktrees

WHEN the user runs `twig worktree list` AND the repository has no git worktrees THEN the command prints a warning
indicating no worktrees were found AND prints guidance to create one with `twig worktree create`

#### Scenario: Worktree exists in git but not in twig state

WHEN the user runs `twig worktree list` AND a git worktree exists that has no corresponding entry in the twig state AND
other state entries do exist THEN the worktree is still listed with its branch name and path AND the creation timestamp
is shown as "Unknown (no metadata available)"

### Requirement: Cleaning stale worktrees

#### Scenario: Stale worktrees are detected and removed

WHEN the user runs `twig worktree clean` AND one or more tracked worktrees have paths that no longer exist on disk THEN
each stale worktree reference is pruned from git's internal tracking AND the corresponding entry is removed from the
twig state AND the updated state is saved AND a success message reports the count of cleaned worktree references

#### Scenario: No stale worktrees found

WHEN the user runs `twig worktree clean` AND all tracked worktrees still exist on disk THEN the command prints a message
indicating no stale worktrees were found AND the state is saved (preserving all entries)

#### Scenario: Repository has no worktrees to clean

WHEN the user runs `twig worktree clean` AND the repository has no git worktrees at all THEN the command prints a
warning indicating no worktrees were found for the repository

### Requirement: Jira integration with worktrees

#### Scenario: Creating a branch with a worktree via Jira

WHEN the user runs `twig jira create-branch <key> --with-worktree` (or `-w`) THEN the issue is fetched from Jira AND a
branch name is generated from the issue AND `create_worktree()` is called instead of a normal branch checkout AND branch
metadata (including the Jira issue association) is stored in the state the same way as a non-worktree branch creation

### Requirement: Main repository resolution from worktrees

#### Scenario: Resolving the main repository when running from a worktree

WHEN a twig command that uses the global registry (e.g., `twig repo remove`, `twig fetch`) is run from within a worktree
directory THEN `resolve_to_main_repo_path()` uses `repo.commondir().parent()` to find the main repository's working
directory AND returns the main repo path instead of the worktree path AND registry operations (add, remove,
update_fetch_time) operate on the main repository entry

#### Scenario: Resolving the main repository when running from a regular repository

WHEN `resolve_to_main_repo_path()` is called from a regular (non-worktree) repository THEN it returns the repository's
own working directory (same behavior as `detect_repository_from_path`)

#### Scenario: Resolving the main repository from a non-repository path

WHEN `resolve_to_main_repo_path()` is called from a path that is not inside any git repository THEN it returns `None`

### Requirement: State operations on worktrees

#### Scenario: Getting a worktree by name

WHEN `get_worktree(name)` is called on the repository state THEN it returns the first worktree entry whose `name` field
matches the given name AND returns `None` if no match is found

#### Scenario: Removing a worktree from state by name

WHEN `remove_worktree(name)` is called on the repository state THEN all entries whose `name` field matches the given
name are removed AND the method returns `true` if any entries were removed AND returns `false` if no entries matched

#### Scenario: Listing all worktrees from state

WHEN `list_worktrees()` is called on the repository state THEN it returns a slice of all worktree entries in the state

### Requirement: Command aliases

#### Scenario: Using the `wt` alias for the worktree command

WHEN the user runs `twig wt` THEN it behaves identically to `twig worktree` with the same subcommand and arguments

#### Scenario: Using the `new` alias for the create subcommand

WHEN the user runs `twig worktree new <branch>` THEN it behaves identically to `twig worktree create <branch>`

#### Scenario: Using the `ls` alias for the list subcommand

WHEN the user runs `twig worktree ls` THEN it behaves identically to `twig worktree list`

### Requirement: Error handling

#### Scenario: Git worktree creation fails

WHEN the underlying `git2` worktree creation fails (e.g., branch already checked out in another worktree, permission
denied) THEN the command fails with an error indicating the worktree could not be created for the branch AND includes a
diagnostic listing possible causes
