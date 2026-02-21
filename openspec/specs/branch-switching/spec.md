# Branch Switching

## Purpose

Intelligently switch to branches by name, Jira issue key, GitHub PR number/URL, or interactively. Optionally creates new
branches with automatic parent dependency linking. Serves as the primary entry point for starting work on an issue or
PR.

**CLI surface:** `twig switch` (alias `sw`), flags: `--root`, `--no-create`, `-p/--parent` **Crates:** `twig-core`
(git/switch, state), `twig-cli` (switch command module)

## Input Detection

### Requirement: Automatic input type detection

The `detect_switch_input()` function classifies raw user input into one of five variants so the switch command can
dispatch to the correct handler without requiring the user to specify the input type explicitly.

#### Scenario: GitHub PR URL is detected

WHEN the input contains `github.com` and `/pull/` AND the URL can be parsed as a valid GitHub PR URL THEN the input is
classified as `GitHubPrUrl` with the extracted PR number

#### Scenario: Jira issue URL is detected

WHEN the input contains `atlassian.net/browse/` or starts with `http` and contains `/browse/` AND a Jira issue key can
be extracted from the URL path THEN the input is classified as `JiraIssueUrl` with the extracted issue key

#### Scenario: GitHub PR ID with PR# prefix is detected

WHEN the input matches the pattern `PR#<number>` AND the number portion parses as a valid unsigned integer THEN the
input is classified as `GitHubPrId` with the parsed number

#### Scenario: GitHub PR ID with # prefix is detected

WHEN the input matches the pattern `#<number>` AND the number portion parses as a valid unsigned integer THEN the input
is classified as `GitHubPrId` with the parsed number

#### Scenario: Bare numeric input is detected as GitHub PR ID

WHEN the input is a bare unsigned integer (e.g. `42`) THEN the input is classified as `GitHubPrId` with the parsed
number

#### Scenario: Jira issue key is detected

WHEN a Jira ticket parser is configured AND the input matches a Jira issue key pattern (e.g. `PROJ-123`) THEN the input
is classified as `JiraIssueKey` with the normalized issue key

#### Scenario: Fallback to branch name

WHEN the input does not match any GitHub PR, Jira URL, numeric PR ID, or Jira issue key pattern THEN the input is
classified as `BranchName` with the original input string

#### Scenario: Detection priority order

WHEN the input is evaluated THEN the detection checks are applied in this order: GitHub PR URL, Jira issue URL, GitHub
PR ID (numeric with optional `PR#` or `#` prefix), Jira issue key, branch name AND the first matching check determines
the classification

## Switch by Branch Name

### Requirement: Switch to an existing local branch

#### Scenario: Branch exists locally

WHEN the user runs `twig switch <branch-name>` AND `<branch-name>` exists as a local branch THEN the working tree is
checked out to that branch AND a success message is printed

#### Scenario: Already on the target branch

WHEN the user runs `twig switch <branch-name>` AND the current HEAD is already on `<branch-name>` THEN the outcome
reports `AlreadyCurrent` AND no checkout operation is performed

### Requirement: Switch to a remote branch

#### Scenario: Branch exists on origin but not locally

WHEN the user runs `twig switch <branch-name>` AND `<branch-name>` does not exist as a local branch AND
`origin/<branch-name>` exists as a remote tracking reference THEN a local branch `<branch-name>` is created from the
remote branch tip AND the local branch's upstream is set to `origin/<branch-name>` AND the working tree is checked out
to the new local branch AND a success message indicates the branch was checked out from origin

### Requirement: Create a new branch when missing

#### Scenario: Branch does not exist locally or on origin

WHEN the user runs `twig switch <branch-name>` AND `<branch-name>` does not exist locally or on origin AND `--no-create`
is not specified THEN a new branch `<branch-name>` is created AND the branch base is resolved from the `--parent` option
(defaulting to HEAD) AND the working tree is checked out to the new branch AND a success message indicates the branch
was created

#### Scenario: Branch does not exist and --no-create is specified

WHEN the user runs `twig switch <branch-name> --no-create` AND `<branch-name>` does not exist locally THEN a warning is
printed indicating the branch does not exist AND no branch is created AND the command returns successfully

## Switch by Jira Issue

### Requirement: Switch to a branch associated with a Jira issue

#### Scenario: Existing branch association found in state

WHEN the user runs `twig switch PROJ-123` (or a Jira issue URL) AND `.twig/state.json` contains a branch metadata entry
linking a branch to `PROJ-123` THEN the working tree is checked out to the associated branch AND no API call to Jira is
made

#### Scenario: No association exists and creation is allowed

WHEN the user runs `twig switch PROJ-123` AND no branch is associated with `PROJ-123` in the repository state AND
`--no-create` is not specified THEN the Jira API is called to fetch the issue summary AND a branch name is generated
from the issue key and summary (e.g. `PROJ-123/example-feature`) AND the branch base is resolved from the `--parent`
option AND the new branch is created and checked out AND a Jira issue association is stored in `.twig/state.json`
linking the new branch to `PROJ-123`

#### Scenario: No association exists and --no-create is specified

WHEN the user runs `twig switch PROJ-123 --no-create` AND no branch is associated with `PROJ-123` THEN a warning is
printed indicating no branch was found for the issue AND no branch is created

## Switch by GitHub PR

### Requirement: Switch to a branch associated with a GitHub PR

#### Scenario: Existing branch association found in state

WHEN the user runs `twig switch 42` (or `PR#42`, `#42`, or a GitHub PR URL) AND `.twig/state.json` contains a branch
metadata entry linking a branch to PR #42 THEN the working tree is checked out to the associated branch AND no API call
to GitHub is made

#### Scenario: No association exists and PR is from the same repository

WHEN the user runs `twig switch 42` AND no branch is associated with PR #42 AND `--no-create` is not specified THEN the
GitHub API is called to fetch the PR details AND the PR head branch name is extracted AND the remote branch is fetched
from origin AND a local tracking branch is created pointing at the PR head commit AND the upstream is set to
`origin/<head-branch>` AND the working tree is checked out AND a parent dependency is recorded if `--parent` was
specified AND a GitHub PR association is stored in `.twig/state.json`

#### Scenario: No association exists and PR is from a fork

WHEN the user runs `twig switch 42` AND the PR head repository differs from the origin repository (i.e. the PR is from a
fork) THEN a new remote named `fork-<owner-login>` is created pointing at the fork's clone URL AND the URL scheme (SSH
vs HTTPS) is chosen to match the origin remote's scheme AND the PR head branch is fetched from the fork remote AND a
local tracking branch is created with upstream set to `fork-<owner-login>/<head-branch>` AND the working tree is checked
out AND the PR association is stored in `.twig/state.json`

#### Scenario: Fork remote already exists with matching URL

WHEN a fork remote with the computed name already exists AND its URL matches the fork's clone URL THEN the existing
remote is reused AND no new remote is created

#### Scenario: Fork remote name collision with different URL

WHEN a fork remote with the computed name already exists AND its URL differs THEN a numeric suffix is appended to the
remote name (e.g. `fork-forker-2`) AND the process repeats until a unique name is found

#### Scenario: No association exists and --no-create is specified

WHEN the user runs `twig switch 42 --no-create` AND no branch is associated with PR #42 THEN a warning is printed
indicating no branch was found for the PR AND no branch is created

## Root Switching

### Requirement: Switch to dependency tree root

#### Scenario: Current branch has a dependency chain

WHEN the user runs `twig switch --root` AND the current branch has parent dependencies THEN the dependency chain is
traversed upward (following the first parent at each step) until a branch with no parents is found AND the working tree
is checked out to that root branch

#### Scenario: Current branch is already the root

WHEN the user runs `twig switch --root` AND the current branch has no parent dependencies THEN an informational message
is printed indicating the branch is already at the dependency tree root AND no checkout is performed

#### Scenario: Root branch does not exist locally

WHEN the user runs `twig switch --root` AND the computed dependency tree root branch does not exist as a local branch
THEN the command fails with an error indicating the root branch does not exist locally AND suggests a broken dependency
chain

#### Scenario: HEAD is detached

WHEN the user runs `twig switch --root` AND HEAD is not on a branch (detached HEAD) THEN the command fails with an error
indicating it cannot determine the dependency tree root

#### Scenario: --root and input are mutually exclusive

WHEN the user runs `twig switch --root <input>` (both flags and a positional argument) THEN the command fails with an
error indicating that `--root` and a positional input cannot be used together

## Parent Dependency Linking

### Requirement: Parent option controls branch base and dependency recording

#### Scenario: No --parent flag specified

WHEN the user runs `twig switch <input>` without the `-p`/`--parent` flag AND a new branch is created THEN the branch is
created from the current HEAD commit AND no parent dependency is recorded in `.twig/state.json`

#### Scenario: --parent flag without a value (defaults to "current")

WHEN the user runs `twig switch <input> -p` (flag present with no value) AND a new branch is created THEN the branch is
created from the tip of the currently checked-out branch AND a parent dependency is recorded linking the new branch to
the current branch

#### Scenario: --parent current

WHEN the user runs `twig switch <input> -p current` AND a new branch is created THEN the branch is created from the tip
of the currently checked-out branch AND a parent dependency is recorded AND if HEAD is detached (not on a branch) the
command fails with an error

#### Scenario: --parent with a branch name

WHEN the user runs `twig switch <input> -p <branch>` AND `<branch>` exists as a local or remote branch AND a new branch
is created THEN the branch is created from the tip of `<branch>` AND a parent dependency is recorded linking the new
branch to `<branch>`

#### Scenario: --parent with a non-existent branch name

WHEN the user runs `twig switch <input> -p <branch>` AND `<branch>` does not exist locally or on origin THEN the command
fails with an error suggesting the user create the parent first using `twig branch depend` or `twig branch root add`

#### Scenario: --parent with a Jira issue key

WHEN the user runs `twig switch <input> -p PROJ-456` AND a Jira ticket parser is configured AND `PROJ-456` matches a
Jira issue key pattern AND a branch associated with `PROJ-456` exists in the repository state THEN the branch is created
from the tip of the associated branch AND a parent dependency is recorded linking the new branch to the associated
branch

#### Scenario: --parent none

WHEN the user runs `twig switch <input> -p none` AND a new branch is created THEN the branch is created from the current
HEAD commit AND no parent dependency is recorded (same behavior as omitting `--parent`)

#### Scenario: --parent is ignored for existing branches

WHEN the user runs `twig switch <branch-name> -p <parent>` AND `<branch-name>` already exists locally THEN the working
tree is checked out to the existing branch AND the `--parent` value is ignored AND no dependency is modified

## Association Storage

### Requirement: Jira issue association is persisted on branch creation

#### Scenario: Branch created from Jira issue stores metadata

WHEN a new branch is created via a Jira issue switch THEN a `BranchMetadata` entry is added to `.twig/state.json`
containing the branch name, the Jira issue key, a `created_at` timestamp in RFC 3339 format, and no GitHub PR number AND
the state file is saved to disk

### Requirement: GitHub PR association is persisted on branch creation

#### Scenario: Branch created from GitHub PR stores metadata

WHEN a new branch is created via a GitHub PR switch THEN a `BranchMetadata` entry is added to `.twig/state.json`
containing the branch name, the GitHub PR number, a `created_at` timestamp in RFC 3339 format, and no Jira issue key AND
the state file is saved to disk

## Command Validation

### Requirement: Input is required when --root is not specified

#### Scenario: No input and no --root

WHEN the user runs `twig switch` with no positional argument and no `--root` flag THEN the command fails with an error
indicating no input was provided AND the error message suggests running `twig switch --help` for more information
