# Rebase

## Purpose

Rebase the current branch onto its parent(s) as defined in the twig dependency tree. Respects the custom dependency
graph rather than relying solely on git's tracking branches, enabling correct rebasing in stacked PR workflows.

**CLI surface:** `twig rebase` (alias `rb`), flags: `--force`, `--show-graph`, `--autostash`, `-r` **Crates:**
`twig-core` (git ops, state), `twig-cli` (rebase command module)

## Requirements

### Requirement: Repository resolution

#### Scenario: Auto-detecting the repository from the working directory

WHEN the user runs `twig rebase` without `-r` THEN the repository is detected by traversing from the current working
directory upward using `detect_repository` (which delegates to `git2::Repository::discover`) AND if no repository is
found, the command fails with a "Not in a git repository" error

#### Scenario: Overriding the repository path with `-r`

WHEN the user runs `twig rebase -r <path>` THEN the command operates on the repository located at `<path>` instead of
auto-detecting from the working directory

### Requirement: HEAD validation

#### Scenario: HEAD is a branch

WHEN the user runs `twig rebase` AND HEAD points to a branch THEN the command proceeds with that branch as the current
branch

#### Scenario: HEAD is detached

WHEN the user runs `twig rebase` AND HEAD is detached (not pointing to a branch) THEN the command fails with a "HEAD is
not a branch. Cannot rebase." error

### Requirement: Parent resolution from dependency graph

#### Scenario: Current branch has twig-defined parent dependencies

WHEN the user runs `twig rebase` AND the current branch has one or more parent branches defined via twig's dependency
system (`get_dependency_parents`) THEN the command identifies those parents as the rebase targets AND rebases the
current branch onto each parent in sequence

#### Scenario: Current branch has no twig-defined parent dependencies

WHEN the user runs `twig rebase` AND the current branch has no parent dependencies in the twig state THEN the command
prints a warning: "No parent branches found for the current branch." AND prints guidance to use
`twig branch depend <parent-branch>` to define a parent AND the command exits successfully without performing any rebase

#### Scenario: No local branches found

WHEN the user runs `twig rebase` AND the resolved branch node set is empty (no local branches) THEN the command prints a
warning: "No local branches found." AND exits successfully without performing any rebase

### Requirement: Basic rebase onto parent

#### Scenario: Successfully rebasing the current branch onto its parent

WHEN the user runs `twig rebase` AND the current branch has a twig-defined parent dependency AND the branch is not
already up-to-date with the parent THEN `git rebase <parent>` is executed in the repository directory AND on success,
the CLI prints a success message: "Successfully rebased <branch> onto <parent>"

#### Scenario: Rebasing onto multiple parents sequentially

WHEN the user runs `twig rebase` AND the current branch has multiple twig-defined parents THEN the branch is rebased
onto each parent in the order returned by `get_dependency_parents` AND each subsequent rebase operates on the branch tip
produced by the previous rebase (cumulative, not from the original tip) AND success or failure is reported for each
parent individually AND if any rebase fails with an error, the command aborts without processing remaining parents

### Requirement: Up-to-date detection

#### Scenario: Branch is already up-to-date with parent

WHEN the user runs `twig rebase` AND git reports the branch is "up to date" with the parent (detected in stdout or
stderr) THEN the command prints an informational message: "Branch <branch> is already up-to-date with <parent>" AND no
rebase operation is performed AND the command continues to the next parent (if any)

### Requirement: Force rebase

#### Scenario: Forcing rebase when already up-to-date

WHEN the user runs `twig rebase --force` AND the branch is already up-to-date with a parent THEN the command prints
"Branch is up-to-date, but force flag is set. Rebasing anyway..." AND executes `git rebase --force-rebase <parent>` AND
on success, prints "Successfully force-rebased <branch> onto <parent>"

#### Scenario: Force rebase fails

WHEN the user runs `twig rebase --force` AND the forced rebase command fails (non-success exit, no CONFLICT marker) THEN
the command prints an error: "Failed to force-rebase <branch> onto <parent>" AND returns a "Rebase failed" error

#### Scenario: Force flag has no effect when branch is not up-to-date

WHEN the user runs `twig rebase --force` AND the branch is not up-to-date with the parent THEN a normal `git rebase` is
performed (the `--force-rebase` flag is only used after detecting the up-to-date condition)

### Requirement: Autostash

#### Scenario: Autostash stashes and restores uncommitted changes

WHEN the user runs `twig rebase --autostash` THEN the `--autostash` flag is passed to the underlying `git rebase`
command AND git automatically stashes uncommitted changes before the rebase and pops them afterward

#### Scenario: Autostash combined with force rebase

WHEN the user runs `twig rebase --force --autostash` AND the branch is already up-to-date THEN the force rebase command
includes both `--force-rebase` and `--autostash` flags

### Requirement: Dependency graph preview

#### Scenario: Showing the dependency graph before rebasing

WHEN the user runs `twig rebase --show-graph` THEN the full branch dependency tree is rendered to stdout before any
rebase operations begin AND the tree includes all root branches and their descendants AND orphaned branches (those with
no dependencies and not marked as roots) are listed separately AND the rebase proceeds normally after displaying the
graph

#### Scenario: Show-graph with no root branches

WHEN the user runs `twig rebase --show-graph` AND no root branches are found in the dependency tree THEN the command
prints a warning: "No root branches found. Cannot display dependency tree." AND continues with the rebase operation

### Requirement: Interactive conflict resolution

This is the canonical definition of the shared conflict-handling interaction. The `cascade-rebase` spec references these
scenarios and documents only its behavioral differences (see `cascade-rebase/spec.md`, "Conflict handling").

#### Scenario: Rebase encounters conflicts

WHEN a `git rebase` operation encounters conflicts (detected by "CONFLICT" in stdout or stderr) THEN the command prints
a warning: "Conflicts detected while rebasing `<branch>` onto `<parent>`" AND presents an interactive prompt with four
resolution options

#### Scenario: User selects "Continue" after resolving conflicts

WHEN the user selects "Continue - Resolve conflicts and continue the rebase" THEN `git rebase --continue` is executed
AND a success message is printed indicating the rebase completed after resolving conflicts

#### Scenario: User selects "Abort to original"

WHEN the user selects "Abort to original - Abort the rebase and return to the original branch" THEN `git rebase --abort`
is executed AND the command reports the rebase was aborted AND the command returns without processing any remaining
parents

#### Scenario: User selects "Abort stay here"

WHEN the user selects "Abort stay here - Abort the rebase but stay on the current branch" THEN `git rebase --abort` is
executed AND the command reports the rebase was aborted AND the command returns without processing any remaining parents

#### Scenario: User selects "Skip"

WHEN the user selects "Skip - Skip the current commit and continue" THEN `git rebase --skip` is executed AND the command
reports that a commit was skipped during the rebase

#### Scenario: Interactive prompt defaults to "Continue"

WHEN the conflict resolution prompt is displayed THEN the default selection is "Continue" (index 0) AND if the prompt
interaction fails, "Continue" is used as the fallback

### Requirement: Error handling

#### Scenario: Git rebase command fails to execute

WHEN the underlying `git rebase` command cannot be executed (e.g., git binary not found) THEN the command fails with a
"Failed to execute git rebase command" error

#### Scenario: Git rebase exits with non-zero status and no conflict

WHEN the `git rebase` command exits with a non-zero status AND the output does not contain "CONFLICT" or "up to date"
markers THEN the command prints an error: "Failed to rebase <branch> onto <parent>" AND returns a "Rebase failed" error

#### Scenario: Repository cannot be opened

WHEN the repository path is provided (via `-r` or auto-detection) AND `git2::Repository::open` fails THEN the command
fails with a "Failed to open git repository at <path>" error

### Requirement: Command alias

#### Scenario: Using the `rb` alias

WHEN the user runs `twig rb` THEN it behaves identically to `twig rebase` with the same arguments
