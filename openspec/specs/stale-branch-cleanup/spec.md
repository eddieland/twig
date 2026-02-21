# Stale Branch Cleanup

## Purpose

Detect branches that haven't been updated within a configurable time window and offer interactive pruning. Also powers
the twig-prune plugin which identifies branches with merged PRs or completed Jira issues for safe deletion, including
twig state cleanup.

**CLI surface:** `twig git stale-branches` (alias `stale`), flags: `-d/--days`, `-p/--prune`, `--json`, `-r` **Plugin:**
`twig-prune` (merged PR / completed issue detection) **Crates:** `twig-core` (state eviction, git ops), `twig-cli` (git
command module)

## Requirements

### Requirement: Repository resolution

#### Scenario: Auto-detecting the repository from the working directory

WHEN the user runs `twig git stale-branches` without `-r` THEN the repository is detected by traversing from the current
working directory upward using `detect_repository` AND if no repository is found, the command fails with a "No
repository specified and not in a git repository" error

#### Scenario: Overriding the repository path with `-r`

WHEN the user runs `twig git stale-branches -r <path>` THEN the command operates on the repository located at `<path>`
instead of auto-detecting from the working directory AND if the path does not exist, the command fails with a
"Repository path does not exist: <path>" error

### Requirement: Flag validation

#### Scenario: `--json` and `--prune` are mutually exclusive

WHEN the user runs `twig git stale-branches --json --prune` THEN the command fails immediately with a "--json cannot be
used together with --prune" error AND no branch scanning is performed

#### Scenario: `--days` must be a positive number

WHEN the user runs `twig git stale-branches -d <value>` AND `<value>` is not a valid positive integer THEN the command
fails with a "Days must be a positive number" error

### Requirement: Staleness threshold

#### Scenario: Default threshold of 30 days

WHEN the user runs `twig git stale-branches` without `-d` THEN branches whose last commit is older than 30 days are
considered stale

#### Scenario: Custom threshold via `--days`

WHEN the user runs `twig git stale-branches -d <N>` THEN branches whose last commit is older than N days are considered
stale AND the cutoff is computed as the current system time minus N days (N * 24 * 60 * 60 seconds)

### Requirement: Stale branch detection

#### Scenario: Iterating local branches and comparing commit times

WHEN the command scans for stale branches THEN it iterates all local branches via `git2` AND for each branch, retrieves
the last commit time via `branch.get().peel_to_commit()?.time()` AND compares the commit timestamp against the computed
cutoff time AND branches with a last commit time before the cutoff are included in the stale list

#### Scenario: Root branches are excluded from stale detection

WHEN the command scans for stale branches AND a branch is marked as a root branch in the repository state
(`repo_state.is_root()`) THEN that branch is excluded from the stale list regardless of its last commit time

#### Scenario: No stale branches found

WHEN the command scans for stale branches AND no branches meet the staleness criteria THEN the command prints "No stale
branches found in <path>" AND exits successfully

### Requirement: Display mode (default)

#### Scenario: Displaying stale branches in the default format

WHEN the user runs `twig git stale-branches` without `--prune` or `--json` AND stale branches are found THEN the command
prints "Found N stale branches in <path>:" AND lists the branches sorted chronologically (oldest first) AND each branch
displays: the branch name (cyan bold), last commit date (yellow), and relative time (dimmed, e.g. "45 days ago") AND a
footer prints: "Run `twig git stale-branches --prune` for interactive cleanup with detailed guidance."

#### Scenario: Branch info enrichment in display mode

WHEN the command displays stale branches in the default format THEN each branch is enhanced with metadata before
display: the parent branch from the dependency graph (`get_dependency_parents`), novel commits via merge-base
computation, and linked Jira issue / GitHub PR from branch metadata

#### Scenario: Parent branch is shown when available

WHEN a stale branch has a parent defined in the twig dependency graph THEN the display includes the parent branch name
below the branch header

#### Scenario: Novel commits are previewed with a cap of three

WHEN a stale branch has novel commits (commits between the branch tip and the merge-base with its parent) THEN up to 3
commits are shown with their abbreviated hash (8 characters, yellow) and first-line message AND if there are more than
3, a "showing 3 of N" suffix is appended

#### Scenario: Jira and GitHub PR links are shown when available

WHEN a stale branch has associated Jira issue or GitHub PR metadata in the repository state THEN the linked Jira issue
key and/or GitHub PR number are displayed

### Requirement: JSON output mode

#### Scenario: Outputting stale branch data as JSON

WHEN the user runs `twig git stale-branches --json` THEN the command outputs a pretty-printed JSON array via
`serde_json::to_string_pretty` AND each element contains the fields: `name`, `last_commit_date`, `parent_branch`,
`novel_commits` (array of objects with `hash` and `message`), `jira_issue`, and `github_pr` AND branches are enriched
with the same metadata as display mode before serialization AND no interactive prompts or styled text are produced

### Requirement: Interactive prune mode

#### Scenario: Entering prune mode

WHEN the user runs `twig git stale-branches --prune` AND stale branches are found THEN the command prints "Finding
branches not updated in the last N days..." followed by "Found N stale branches." AND branches are sorted alphabetically
for consistent ordering

#### Scenario: Per-branch detail display during pruning

WHEN prune mode iterates over each stale branch THEN the branch is enhanced with parent, novel commits, and Jira/PR
metadata via `enhance_branch_info` AND a progress indicator `[current/total]` is displayed AND the following details are
shown: branch name, last commit date with relative time, parent branch (or "(none)"), novel commits with hash and
message, linked Jira issue, and linked GitHub PR

#### Scenario: User confirms deletion

WHEN the prune mode displays a branch and prompts "Delete branch '<name>'? \[y/N\]:" AND the user enters "y" or "yes"
(case-insensitive) THEN the branch is deleted via `repo.find_branch().delete()` AND the branch name is added to the
deleted list in the prune summary

#### Scenario: User declines deletion

WHEN the prune mode displays a branch and prompts "Delete branch '<name>'? \[y/N\]:" AND the user enters anything other
than "y" or "yes" (including pressing Enter for the default "N") THEN the branch is not deleted AND it is counted as
skipped in the prune summary

#### Scenario: Deletion fails

WHEN a branch deletion fails THEN the error is recorded in the prune summary with the branch name and error message AND
an error message "Failed to delete <name>: <error>" is printed AND the command continues to the next branch

#### Scenario: Config cleanup error workaround (libgit2 issue 4247)

WHEN branch deletion via `git2` raises a config-class error containing "could not find key" THEN the command checks
whether the branch reference was actually deleted despite the error AND if the branch no longer exists, the deletion is
treated as successful AND if the branch still exists, the original error is propagated

#### Scenario: No stale branches found in prune mode

WHEN the user runs `twig git stale-branches --prune` AND no branches meet the staleness criteria THEN the command prints
"No stale branches found in <path>" AND exits successfully without any prompts

### Requirement: Prune summary

#### Scenario: Displaying the prune summary after interactive pruning

WHEN interactive pruning completes THEN a "Prune Summary" is printed containing: total stale count, deleted count with
branch names, skipped count, and error count with branch names

### Requirement: Branch info enhancement

#### Scenario: Resolving parent branch from the dependency graph

WHEN a stale branch is enhanced THEN the parent is looked up via `repo_state.get_dependency_parents()` AND the first
entry (if any) is used as the parent branch

#### Scenario: Computing novel commits via merge-base

WHEN a stale branch has a resolved parent branch AND both branches exist locally THEN the merge-base between the branch
and its parent is computed AND a revwalk from the branch tip to the merge-base yields the novel commits AND each commit
records an 8-character abbreviated hash and the first line of the commit message

#### Scenario: Novel commit computation fails gracefully

WHEN the merge-base or revwalk computation fails (e.g., the parent branch does not exist locally) THEN the error is
logged at debug level via tracing AND the novel commits list remains empty AND the command continues without failing

#### Scenario: Jira and PR metadata from branch metadata

WHEN a stale branch has an entry in the repository state's branch metadata THEN the `jira_issue` and `github_pr` fields
are populated from that metadata

### Requirement: State eviction

#### Scenario: Evicting deleted branches from twig state

WHEN `evict_stale_branches` is called with a set of locally existing branch names THEN branches in the `branches`
HashMap that are not in the local set and are not root branches are removed AND dependencies whose `child` field
references a removed (non-local, non-root) branch are removed AND if any entries were removed, indices are rebuilt via
`rebuild_indices` AND an `EvictionStats` is returned with counts of removed branches and dependencies

#### Scenario: Root branches are preserved during eviction

WHEN `evict_stale_branches` is called AND a branch is marked as a root branch in the repository state THEN that branch
is retained in the `branches` HashMap even if it is not present in the local branch set AND dependencies referencing
that root branch as a child are also retained

#### Scenario: Eviction is a no-op when all branches exist

WHEN `evict_stale_branches` is called AND every branch in the state is present in the local branch set THEN no entries
are removed AND the returned `EvictionStats` has zero counts AND indices are not rebuilt

### Requirement: Command alias

#### Scenario: Using the `stale` alias

WHEN the user runs `twig git stale` THEN it behaves identically to `twig git stale-branches` with the same arguments

### Requirement: twig-prune plugin

#### Scenario: Discovering branches eligible for pruning

WHEN the `twig-prune` plugin runs THEN it opens the repository from the plugin context (`TWIG_CURRENT_REPO`) AND
collects all local branches AND excludes the current branch (`TWIG_CURRENT_BRANCH`) and root branches from the candidate
list

#### Scenario: Identifying branches with merged GitHub PRs

WHEN an eligible branch has a GitHub PR number in the twig branch metadata THEN the plugin fetches the PR from the
GitHub API AND if the PR has a non-null `merged_at` field, the branch is added to the prune candidate list with a
description "PR #N (title)"

#### Scenario: Identifying branches with completed Jira issues

WHEN an eligible branch has a Jira issue key in the twig branch metadata AND the branch was not already matched by a
merged PR THEN the plugin fetches the Jira issue AND if the issue status (lowercased) is one of "done", "closed", or
"resolved", the branch is added to the prune candidate list

#### Scenario: Dry run mode

WHEN the user runs `twig-prune --dry-run` (or `-n`) THEN the plugin lists all prune candidates with their descriptions
AND prints "Dry run -- no branches were deleted." AND no branches are actually deleted

#### Scenario: Skip-prompts mode

WHEN the user runs `twig-prune --yes-i-really-want-to-skip-prompts` THEN all prune candidates are deleted without
individual confirmation prompts

#### Scenario: Interactive confirmation per candidate

WHEN `twig-prune` runs without `--dry-run` or `--yes-i-really-want-to-skip-prompts` THEN each candidate is displayed
with its description AND the user is prompted "Delete '<name>'?" with a default of "no" AND the branch is deleted only
if the user confirms

#### Scenario: State cleanup after pruning

WHEN `twig-prune` deletes one or more branches THEN it reloads the set of remaining local branches AND calls
`evict_stale_branches` on the repo state with that set AND saves the updated state AND if saving fails, an error is
printed but the command does not abort

#### Scenario: Graceful degradation when services are unavailable

WHEN the GitHub client cannot be created THEN PR checks are skipped with a warning AND when the Jira host is not
configured or the Jira client cannot be created THEN Jira checks are skipped with an informational message AND the
plugin continues with whatever candidates were found from available services
