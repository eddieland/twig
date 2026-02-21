# GitHub Integration

## Purpose

Interact with GitHub pull requests from the terminal. Link PRs to branches, list PRs with filtering, view PR status with
review and CI check details, open PRs in the browser, and verify GitHub authentication. Credentials sourced from
~/.netrc.

**CLI surface:** `twig github check`, `twig github checks`, `twig github open`, `twig github pr link/list/status`
**Crates:** `twig-gh` (client, endpoints, models), `twig-core` (github utils, state), `twig-cli` (github command module)

## Requirements

### Requirement: Credential loading

#### Scenario: Credentials found in .netrc

WHEN a GitHub subcommand requires authentication AND the user's `~/.netrc` file contains an entry for machine
`github.com` THEN the username and password are loaded from that entry AND all API requests use HTTP Basic
authentication with the header `Authorization: Basic <base64(username:password)>` AND requests include
`Accept: application/vnd.github.v3+json` and `User-Agent: twig`

#### Scenario: Credentials not found on Unix

WHEN a GitHub subcommand requires authentication AND no entry for machine `github.com` exists in `~/.netrc` THEN the
command fails with an error indicating GitHub credentials were not found AND includes guidance to add credentials for
`github.com` to `~/.netrc`

#### Scenario: Credentials not found on Windows

WHEN a GitHub subcommand requires authentication AND no credentials are found for machine `github.com` THEN the command
fails with an error indicating GitHub credentials were not found AND includes guidance to run `twig creds setup`

### Requirement: Remote detection

#### Scenario: Extracting owner and repo from an HTTPS remote

WHEN a subcommand needs the repository's GitHub owner and name AND the `origin` remote URL matches
`github.com[/:]([^/]+)/([^/\.]+)` (e.g., `https://github.com/owner/repo.git`) THEN the owner and repo name are extracted
from the URL

#### Scenario: Extracting owner and repo from an SSH remote

WHEN a subcommand needs the repository's GitHub owner and name AND the `origin` remote URL is in SCP-style format (e.g.,
`git@github.com:owner/repo.git`) THEN the same regex extracts the owner and repo name

#### Scenario: Remote URL does not match GitHub

WHEN a subcommand needs the repository's GitHub owner and name AND the `origin` remote URL does not match the GitHub
pattern THEN the command fails with an error indicating the owner and repo could not be extracted from the remote URL

#### Scenario: No origin remote exists

WHEN a subcommand attempts to read the `origin` remote AND the repository has no remote named `origin` THEN the command
fails with an error indicating the `origin` remote was not found

### Requirement: Repository resolution for subcommands

Repository resolution follows the shared behavior defined in `repository-resolution/spec.md`. Subcommands that accept a
repository override use the `--repo` (or `-r`) flag.

### Requirement: Authentication check (`github check`)

#### Scenario: Successful authentication

WHEN the user runs `twig github check` AND credentials are valid THEN the command calls `test_connection()` followed by
`get_current_user()` AND prints a success message confirming authentication AND displays the user's username, name (if
present), and user ID

#### Scenario: Authentication failure

WHEN the user runs `twig github check` AND `test_connection()` returns false THEN the command prints an error indicating
authentication failed

#### Scenario: Connection error

WHEN the user runs `twig github check` AND `test_connection()` returns an error THEN the command prints an error
indicating authentication with GitHub failed, including the underlying error details

#### Scenario: User info retrieval failure

WHEN the user runs `twig github check` AND authentication succeeds but `get_current_user()` fails THEN the command
prints an error indicating user information could not be retrieved

### Requirement: CI/CD check display (`github checks`)

#### Scenario: Viewing checks for an explicit PR number

WHEN the user runs `twig github checks <PR_NUMBER>` THEN the command fetches the PR details to obtain the head commit
SHA AND fetches check runs for that commit AND displays a table with columns for check name, status, conclusion, and
start time AND lists detail URLs below the table

#### Scenario: Viewing checks for the current branch's PR

WHEN the user runs `twig github checks` without a PR number AND the current branch has an associated PR in
`.twig/state.json` THEN the command uses that PR number

#### Scenario: No associated PR and no argument

WHEN the user runs `twig github checks` without a PR number AND the current branch has no associated PR THEN the command
prints a warning indicating the branch has no associated PR AND prints guidance to link a PR with `twig github pr link`
or specify a PR number

#### Scenario: Invalid PR number argument

WHEN the user runs `twig github checks <value>` AND the value cannot be parsed as a u32 THEN the command prints an error
indicating the PR number is invalid

#### Scenario: Check status coloring in the table

WHEN the checks table is rendered THEN status values are colored: `completed` in green, `in_progress` in yellow,
`queued` in blue AND conclusion values are colored: `success` in green, `failure` in red, `cancelled` in yellow,
`timed_out` in red, `action_required` in yellow

#### Scenario: No checks found

WHEN the checks are fetched for a PR AND the response contains zero check runs THEN the command prints a message
indicating no checks were found for the PR

#### Scenario: Details URLs displayed

WHEN check runs include `details_url` fields THEN the command prints a "Details:" section with each check name and its
URL

### Requirement: CI/CD checks command alias

#### Scenario: Using the `ci` alias

WHEN the user runs `twig github ci` THEN it behaves identically to `twig github checks` with the same arguments

### Requirement: Open PR in browser (`github open`)

#### Scenario: Opening a PR by explicit number

WHEN the user runs `twig github open <PR_NUMBER>` THEN the command constructs the URL
`https://github.com/{owner}/{repo}/pull/{PR_NUMBER}` AND opens it in the system's default browser

#### Scenario: Opening the current branch's PR

WHEN the user runs `twig github open` without a PR number AND the current branch has an associated PR THEN the command
opens that PR's URL in the browser

#### Scenario: No associated PR

WHEN the user runs `twig github open` without a PR number AND the current branch has no associated PR THEN the command
prints a warning indicating the current branch has no associated GitHub PR AND prints guidance to link a PR with
`twig github pr link`

#### Scenario: Invalid PR number argument

WHEN the user runs `twig github open <value>` AND the value cannot be parsed as a u32 THEN the command prints an error
indicating the PR number is invalid

#### Scenario: Browser open failure is non-fatal

WHEN the command attempts to open a URL in the browser AND the browser launch fails THEN a warning is printed but the
command does not return an error

### Requirement: Link PR to branch (`github pr link`)

#### Scenario: Linking with a full PR URL

WHEN the user runs `twig github pr link <URL>` AND the URL contains `github.com` and `/pull/` THEN the URL is parsed to
extract the PR number AND the PR is fetched from GitHub for validation and title extraction

#### Scenario: Linking with a bare integer

WHEN the user runs `twig github pr link <NUMBER>` AND the value does not contain `github.com` or `/pull/` THEN the value
is parsed as a u32 PR number AND the PR is fetched from GitHub for validation and title extraction

#### Scenario: Creating a new link

WHEN the PR is validated AND the current branch has no existing metadata in `.twig/state.json` THEN a new
`BranchMetadata` entry is created with the `github_pr` field set THEN the command prints a success message indicating
the branch was linked to the PR, including the PR number and title

#### Scenario: Updating an existing link

WHEN the PR is validated AND the current branch already has metadata in `.twig/state.json` THEN the existing entry's
`github_pr` field is updated THEN the command prints a success message indicating the branch link was updated

#### Scenario: State save failure is non-fatal

WHEN the branch metadata is updated in memory but the `.twig/state.json` file cannot be saved THEN the command prints an
error indicating the state could not be saved AND returns Ok (does not propagate the error)

#### Scenario: Invalid PR URL

WHEN the user provides a URL containing `github.com` and `/pull/` AND the URL cannot be parsed (e.g., non-numeric PR
segment) THEN the command prints an error indicating the PR URL is invalid

#### Scenario: Invalid PR ID

WHEN the user provides a value that is not a URL AND the value cannot be parsed as a u32 THEN the command prints an
error indicating the PR ID is invalid

#### Scenario: No argument and no associated PR

WHEN the user runs `twig github pr link` without an argument AND the current branch has no associated PR THEN the
command prints an error indicating no PR URL or ID was provided and the current branch has no associated PR

#### Scenario: No argument with existing associated PR

WHEN the user runs `twig github pr link` without an argument AND the current branch has an associated PR number in state
THEN the command re-links using that PR number (fetches and validates it again)

### Requirement: List pull requests (`github pr list`)

#### Scenario: Default listing

WHEN the user runs `twig github pr list` THEN the command fetches up to 30 open pull requests (single page, no
pagination beyond the first page) AND displays a table with columns: PR #, Title, Author, State, Created

#### Scenario: Filtering by state

WHEN the user runs `twig github pr list --state <STATE>` AND the state is one of "open", "closed", or "all" THEN only
pull requests matching that state are fetched

#### Scenario: Limiting result count

WHEN the user runs `twig github pr list --limit <COUNT>` THEN at most `<COUNT>` pull requests are fetched in a single
page request

#### Scenario: Title truncation

WHEN a PR title exceeds 44 characters THEN the title is truncated to 44 characters in the table display

#### Scenario: State coloring in the table

WHEN the PR list table is rendered THEN "open" state values are colored green AND "closed" state values are colored red

#### Scenario: Date formatting

WHEN the PR list table is rendered THEN the Created column shows only the date portion (before the `T` in the ISO 8601
timestamp)

#### Scenario: No pull requests found

WHEN the API returns an empty list THEN the command prints a message indicating no matching pull requests were found for
the repository

### Requirement: List command alias

#### Scenario: Using the `ls` alias

WHEN the user runs `twig github pr ls` THEN it behaves identically to `twig github pr list` with the same arguments

### Requirement: PR status display (`github pr status`)

#### Scenario: Displaying PR status

WHEN the user runs `twig github pr status` AND the current branch has an associated PR THEN the command fetches the PR
details, reviews, and check runs AND displays: PR number, title, URL, state, created date, updated date

#### Scenario: Draft indicator

WHEN the fetched PR has `draft` set to `true` THEN the output includes "Draft: Yes" AND when `draft` is false or absent
THEN the draft line is omitted

#### Scenario: Mergeable information

WHEN the PR response includes a `mergeable` field THEN the output includes "Mergeable: Yes" or "Mergeable: No" AND when
`mergeable_state` is present, it is displayed as "Mergeable State: <value>"

#### Scenario: Reviews display with deduplication

WHEN the PR has reviews THEN the command groups reviews by user and shows only the latest review per user AND each
review line shows: timestamp, username, and state AND review states are colored: APPROVED in green, CHANGES_REQUESTED in
red, COMMENTED in yellow

#### Scenario: Checks display in status

WHEN the PR's head commit has check runs THEN the command displays each check's name and a formatted status/conclusion
string AND for completed checks, the conclusion is shown with coloring (success=green, failure=red, cancelled=yellow,
skipped=dim) AND for in-progress checks, the status itself is shown in yellow

#### Scenario: No associated PR

WHEN the user runs `twig github pr status` AND the current branch has no associated PR THEN the command prints a warning
indicating the branch has no associated PR AND prints guidance to link a PR with `twig github pr link`

#### Scenario: Status command always uses current repository

WHEN the user runs `twig github pr status` THEN the command always operates on the repository detected from the current
working directory AND does not accept a `--repo` flag

### Requirement: Status command alias

#### Scenario: Using the `st` alias

WHEN the user runs `twig github pr st` THEN it behaves identically to `twig github pr status` with the same arguments

### Requirement: API error handling

#### Scenario: Authentication failure (401/403)

WHEN any GitHub API request returns HTTP 401 or 403 THEN the error indicates authentication failed and suggests checking
GitHub credentials

#### Scenario: Pull request not found (404)

WHEN a `get_pull_request` or `get_pull_request_reviews` call returns HTTP 404 THEN the error indicates the pull request
was not found

#### Scenario: Commit not found (404) for check runs

WHEN a `get_check_runs` call returns HTTP 404 THEN the error indicates the commit was not found

#### Scenario: Unexpected HTTP status

WHEN any GitHub API request returns an unhandled HTTP status THEN the error includes the HTTP status code and the
response body text
