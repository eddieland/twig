# Fixup Commits

## Purpose

Interactively select from recent commits and create fixup commits that will be automatically squashed during the next
interactive rebase. Filters by author, date range, and existing fixup status. Supports vim-mode navigation for power
users.

**CLI surface:** `twig fixup` (alias `fix`), flags: `--limit`, `--days`, `--all-authors`, `--include-fixups`,
`--dry-run`, `--vim-mode` **Crates:** `twig-core` (git ops), `twig-cli` (fixup command module)

## Requirements

### Requirement: Staged changes validation

The command must verify that staged changes exist before proceeding. Without staged content, a fixup commit cannot be
created.

#### Scenario: No staged changes

- WHEN the user runs `twig fixup`
- AND there are no staged changes in the repository
- THEN the command prints a warning: "No staged changes found. Stage changes first before creating a fixup commit."
- AND the command exits without error (exit code 0)
- AND no interactive selector is shown

#### Scenario: Staged changes present

- WHEN the user runs `twig fixup`
- AND there are staged changes in the repository
- THEN the command proceeds to collect commit candidates

### Requirement: Repository detection

The command must be run inside a Git repository.

#### Scenario: Not in a git repository

- WHEN the user runs `twig fixup`
- AND the current directory is not inside a git repository
- THEN the command exits with an error: "Not in a git repository"

### Requirement: Commit collection with default filters

By default, the command collects recent commits from the current branch filtered by author, date range, and fixup
status.

#### Scenario: Default commit collection

- WHEN the user runs `twig fixup` with no flags
- THEN the command collects up to 20 commits (default `--limit`)
- AND only includes commits from the last 30 days (default `--days`)
- AND only includes commits authored by the current git user (from `user.name` config)
- AND excludes commits whose messages start with "fixup!"
- AND commits are returned in reverse chronological order (newest first)

#### Scenario: No matching commits found

- WHEN the user runs `twig fixup`
- AND no commits match the filtering criteria
- THEN the command prints a warning: "No recent commits found. Try increasing --limit or --days."
- AND the command exits without error

### Requirement: Limit flag

The `--limit` flag controls the maximum number of commits to consider.

#### Scenario: Custom limit

- WHEN the user runs `twig fixup --limit 5`
- THEN at most 5 commits are collected from HEAD walking backward
- AND the most recent commits are returned first

#### Scenario: Limit interaction with date filter

- WHEN the user runs `twig fixup --limit 50 --days 7`
- THEN at most 50 commits are considered
- AND only commits from the last 7 days are included
- AND commits older than 7 days are skipped but do not count against the limit

### Requirement: Days flag

The `--days` flag restricts commits to a rolling time window.

#### Scenario: Custom days window

- WHEN the user runs `twig fixup --days 7`
- THEN only commits with a timestamp within the last 7 days are included
- AND older commits are skipped during the revwalk

### Requirement: All-authors flag

By default, only commits from the current user are shown. The `--all-authors` flag removes this filter.

#### Scenario: Default author filtering

- WHEN the user runs `twig fixup`
- AND the repository has commits from multiple authors
- THEN only commits where the author name matches the current git `user.name` are included

#### Scenario: All authors included

- WHEN the user runs `twig fixup --all-authors`
- AND the repository has commits from multiple authors
- THEN commits from all authors are included in the candidate list

### Requirement: Include-fixups flag

By default, existing fixup commits are excluded from the candidate list. The `--include-fixups` flag includes them.

#### Scenario: Default fixup exclusion

- WHEN the user runs `twig fixup`
- AND the recent history contains commits with messages starting with "fixup!"
- THEN those fixup commits are excluded from the candidate list

#### Scenario: Include fixup commits

- WHEN the user runs `twig fixup --include-fixups`
- AND the recent history contains commits with messages starting with "fixup!"
- THEN those fixup commits are included in the candidate list

### Requirement: Commit scoring and sorting

Collected commits are scored using a weighted algorithm and sorted by relevance before being presented to the user.

#### Scenario: Scoring factors

- WHEN commits are collected and ready for display
- THEN each commit receives a score between 0.0 and 1.0 based on:
  - Branch uniqueness (50% weight): 0.50 if the commit is only reachable from the current branch (not reachable from the
    comparison branch), 0.0 otherwise
  - Recency (25% weight): scaled linearly from 1.0 (just now) to 0.0 (at the `--days` boundary)
  - Authorship (15% weight): 0.15 if the commit author matches the current git user, 0.0 otherwise
  - Jira association (10% weight): 0.10 if the commit's Jira issue matches the current branch's Jira issue, 0.0
    otherwise
- AND commits are sorted in descending order by score (highest first)

#### Scenario: Branch uniqueness dominance

- WHEN a commit exists only on the current branch (not reachable from the parent/comparison branch)
- AND another commit exists on both the current branch and the parent branch
- THEN the branch-unique commit scores higher even if the shared commit is more recent, by the current user, and has a
  matching Jira issue

#### Scenario: Comparison branch resolution for uniqueness

- WHEN determining branch uniqueness
- THEN the comparison branch is resolved with the following priority:
  1. Configured dependency parent from `RepoState::get_dependency_parents()`
  1. Default root branch from `RepoState::get_default_root()`
  1. Fallback to `origin/main`, then `origin/master`
- AND if no comparison branch is found, branch uniqueness detection is skipped

#### Scenario: Jira issue extraction for scoring

- WHEN scoring commits for Jira association
- THEN Jira issue keys are extracted from commit messages using the configured Jira parser
- AND the current branch's Jira issue is obtained via `get_current_branch_jira_issue()`
- AND the bonus is awarded only when both the commit and branch have a Jira issue and they match exactly

### Requirement: Interactive commit selector

The command presents a TUI-based interactive selector for choosing a fixup target.

#### Scenario: Selector display format

- WHEN the interactive selector is shown
- THEN each commit is displayed in the format:
  `<short_hash> <relative_time> <branch_unique_indicator> <jira_indicator> <author_indicator> <message> (<author>)`
- AND `short_hash` is the first 7 characters of the commit hash
- AND `relative_time` is human-readable (e.g., "2h ago", "3d ago", "30m ago", "just now")
- AND `branch_unique_indicator` is a star symbol if the commit is unique to the current branch, a space otherwise
- AND `jira_indicator` is a ticket symbol if the commit has a Jira issue, a space otherwise
- AND `author_indicator` is a filled circle for the current user, an open circle for other users

#### Scenario: Fuzzy search filtering

- WHEN the user types in the search input
- THEN the commit list is filtered using case-insensitive fuzzy matching via nucleo
- AND the matching is performed against the full display text of each candidate
- AND results are sorted by fuzzy match score (best match first)
- AND the selection index resets to the first item after each keystroke

#### Scenario: User cancels selection

- WHEN the user presses Escape (in default mode) or Ctrl+C
- THEN the selector closes
- AND the command exits silently without creating a fixup commit

#### Scenario: Empty candidate list passed to selector

- WHEN the selector receives an empty candidate list
- THEN it immediately returns None without displaying the TUI

### Requirement: Default mode navigation

In default mode (no `--vim-mode`), the selector provides a unified interface where typing and navigation coexist.

#### Scenario: Default mode keyboard controls

- WHEN the user runs `twig fixup` (without `--vim-mode`)
- THEN typing any character adds to the search query and filters the list
- AND Up/Down arrow keys navigate the commit list
- AND Enter selects the currently highlighted commit
- AND Escape cancels the selection
- AND Ctrl+C cancels the selection
- AND Backspace removes the last character from the search query
- AND there is no mode switching (Tab, `/`, `q`, `j`, `k` are treated as text input or ignored)

### Requirement: Vim mode navigation

When `--vim-mode` is enabled, the selector has two distinct modes: Search and Navigation.

#### Scenario: Vim mode starts in Search mode

- WHEN the user runs `twig fixup --vim-mode`
- THEN the selector starts in Search mode
- AND the search input is focused and highlighted in yellow

#### Scenario: Vim mode Search mode controls

- WHEN the selector is in Search mode (vim-mode)
- THEN typing characters adds to the search query
- AND Backspace removes the last character
- AND Tab switches to Navigation mode
- AND Enter switches to Navigation mode
- AND Escape switches to Navigation mode

#### Scenario: Vim mode Navigation mode controls

- WHEN the selector is in Navigation mode (vim-mode)
- THEN `j` or Down arrow moves to the next commit
- AND `k` or Up arrow moves to the previous commit
- AND Enter selects the currently highlighted commit
- AND Escape cancels the selection
- AND `q` cancels the selection
- AND `/` switches to Search mode
- AND Tab switches to Search mode

#### Scenario: Navigation wraps around

- WHEN the user navigates past the last commit in the list
- THEN the selection wraps to the first commit
- AND navigating before the first commit wraps to the last commit

### Requirement: Fixup commit creation

After selecting a target commit, the command creates a fixup commit using git.

#### Scenario: Fixup commit created

- WHEN the user selects a commit from the interactive selector
- AND `--dry-run` is not set
- THEN the command runs `git commit --fixup <full-commit-hash>`
- AND on success, prints: "Fixup commit created successfully."

#### Scenario: Git commit --fixup failure

- WHEN `git commit --fixup` fails
- THEN the command prints an error: "Failed to create fixup commit."
- AND the command exits with an error containing the git stderr output

### Requirement: Dry run mode

The `--dry-run` flag shows what would happen without making changes.

#### Scenario: Dry run output

- WHEN the user runs `twig fixup --dry-run`
- AND selects a commit from the interactive selector
- THEN the command prints: "Would create fixup commit for: \<short_hash> <message>"
- AND no git commit is created

### Requirement: Command alias

The fixup command supports a short alias for quick access.

#### Scenario: Alias resolution

- WHEN the user runs `twig fix`
- THEN it is equivalent to running `twig fixup`
- AND all flags and behavior are identical
