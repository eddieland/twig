# Jira Integration

## Purpose

Work with Jira issues from the terminal. View issue details, create branches from issues, link existing branches to
issues, transition issues through workflow states, open issues in the browser, and configure Jira parsing modes.
Credentials sourced from ~/.netrc, host from JIRA_HOST env var or config.

**CLI surface:** `twig jira view/open/create-branch/link-branch/transition/config` **Crates:** `twig-jira` (client,
endpoints, models), `twig-core` (jira_parser, state, utils), `twig-cli` (jira command module)

## Requirements

### Requirement: Jira issue key parsing and normalization

#### Scenario: Parsing in strict mode

WHEN the Jira parsing mode is set to Strict AND the user provides an issue key THEN only the `PROJECT-NUMBER` format is
accepted (uppercase letters, hyphen, digits) with a minimum project code length of 2 characters AND any other format is
rejected with "Invalid Jira issue key format: '{key}'"

#### Scenario: Parsing in flexible mode

WHEN the Jira parsing mode is set to Flexible (the default) AND the user provides an issue key THEN the formats
`ME-1234`, `me-1234`, `ME1234`, and `me1234` are all accepted with a minimum project code length of 2 characters AND the
key is normalized to uppercase with a hyphen (e.g., `me1234` becomes `ME-1234`)

#### Scenario: Extracting issue keys from commit messages in strict mode

WHEN the parsing mode is Strict AND a commit message is examined for a Jira issue key THEN only the pattern
`^([A-Z]{2,}-\d+):` is matched (uppercase project code, hyphen, digits, followed by a colon)

#### Scenario: Extracting issue keys from commit messages in flexible mode

WHEN the parsing mode is Flexible AND a commit message is examined for a Jira issue key THEN the pattern
`^([A-Za-z]{2,}[-]?\d+):` is matched (mixed-case project code, optional hyphen, digits, followed by a colon) AND the
extracted key is normalized to uppercase with a hyphen

#### Scenario: Parser configuration loading

WHEN a Jira subcommand is executed THEN the parser configuration is loaded from `${XDG_CONFIG_HOME}/twig/jira.toml` AND
if the file does not exist or cannot be loaded, the default Flexible mode is used

### Requirement: Current branch Jira issue default

#### Scenario: Resolving the issue key from the current branch

WHEN a subcommand accepts an optional issue key AND no issue key is provided THEN the command detects the current
repository, determines the current branch name, loads `.twig/state.json`, and returns the `jira_issue` field from the
branch's metadata AND if the branch has no associated Jira issue, the command prints "No Jira issue key provided and
current branch has no associated Jira issue"

#### Scenario: Not in a git repository when resolving the current branch

WHEN the current branch's Jira issue is needed AND the working directory is not inside a git repository THEN the command
fails with a "Not in a Git repository" error

### Requirement: Viewing a Jira issue

#### Scenario: Viewing an issue with an explicit key

WHEN the user runs `twig jira view <ISSUE_KEY>` THEN the issue key is parsed and normalized AND the issue is fetched
from the Jira API AND the command displays the issue's key, summary, status, and description (if available)

#### Scenario: Viewing an issue using the current branch default

WHEN the user runs `twig jira view` without an issue key THEN the issue key is resolved from the current branch's
metadata AND the issue is displayed as if the key had been provided explicitly

#### Scenario: Viewing an issue with an invalid key format

WHEN the user runs `twig jira view <ISSUE_KEY>` AND the key does not match the configured parsing mode THEN the command
prints "Invalid Jira issue key format: '{key}'"

#### Scenario: Viewing an issue that fails to fetch

WHEN the user runs `twig jira view <ISSUE_KEY>` AND the Jira API request fails THEN the command prints "Failed to fetch
issue {issue_key}: {error}"

### Requirement: Opening a Jira issue in the browser

#### Scenario: Opening an issue with an explicit key

WHEN the user runs `twig jira open <ISSUE_KEY>` THEN the issue key is parsed and normalized AND the Jira host is
resolved AND the URL `{jira_host}/browse/{issue_key}` is constructed AND the URL is opened in the default browser

#### Scenario: Opening an issue using the current branch default

WHEN the user runs `twig jira open` without an issue key THEN the issue key is resolved from the current branch's
metadata AND the issue is opened as if the key had been provided explicitly

#### Scenario: Opening an issue with no linked issue on the current branch

WHEN the user runs `twig jira open` without an issue key AND the current branch has no associated Jira issue THEN the
command prints "Current branch has no associated Jira issue" AND prints guidance: "Link an issue with: twig jira
link-branch <issue-key>"

#### Scenario: Opening an issue when the Jira host is not configured

WHEN the user runs `twig jira open` AND the Jira host cannot be resolved (JIRA_HOST env var not set) THEN the command
prints "Jira host not configured: {error}" AND prints guidance: "Set up Jira credentials with: twig creds jira"

### Requirement: Creating a branch from a Jira issue

#### Scenario: Creating a branch without a worktree

WHEN the user runs `twig jira create-branch <ISSUE_KEY>` THEN the issue key is parsed and normalized AND the issue is
fetched from the Jira API to obtain its summary AND a branch name is generated from the issue key and summary using
`generate_branch_name_from_issue` with stop word filtering enabled AND the branch is created from HEAD via
`repo.branch(name, commit, false)` AND the branch metadata is stored in `.twig/state.json` with the `jira_issue` field
AND the command prints "Creating branch: {name}" followed by "Created branch '{name}'"

#### Scenario: Creating a branch with a worktree

WHEN the user runs `twig jira create-branch <ISSUE_KEY> --with-worktree` THEN the branch name is generated as normal AND
a git worktree is created via `create_worktree(repo_path, branch_name)` instead of a regular branch AND the command
prints "Created worktree for branch '{name}'"

#### Scenario: Creating a branch with an invalid issue key

WHEN the user runs `twig jira create-branch <ISSUE_KEY>` AND the key does not match the configured parsing mode THEN the
command prints "Invalid Jira issue key format: '{key}'" AND no branch is created

#### Scenario: Creating a branch when the issue fetch fails

WHEN the user runs `twig jira create-branch <ISSUE_KEY>` AND the Jira API request to fetch the issue fails THEN the
command prints "Failed to fetch issue {issue_key}: {error}" AND no branch is created

#### Scenario: Branch name generation from issue

WHEN a branch name is generated from a Jira issue THEN the format is `{ISSUE_KEY}/{sanitized-summary}` where the summary
has stop words filtered and special characters sanitized AND if the sanitized summary is empty, the branch name is just
the issue key

### Requirement: Linking a branch to a Jira issue

#### Scenario: Linking the current branch with an explicit issue key

WHEN the user runs `twig jira link-branch <ISSUE_KEY>` without a branch name THEN the issue key is parsed and normalized
AND the issue is verified to exist via the Jira API AND the current branch is determined from HEAD AND the branch
metadata in `.twig/state.json` is updated with the `jira_issue` field AND the command prints "Associated branch
'{branch}' with Jira issue {issue_key}"

#### Scenario: Linking a specific branch with an explicit issue key

WHEN the user runs `twig jira link-branch <ISSUE_KEY> <BRANCH>` THEN the specified branch is verified to exist as a
local branch AND the branch metadata is updated as normal

#### Scenario: Linking when the branch does not exist

WHEN the user runs `twig jira link-branch <ISSUE_KEY> <BRANCH>` AND the specified branch does not exist locally THEN the
command prints "Branch '{name}' not found"

#### Scenario: Linking when the branch is already linked to the same issue

WHEN the user runs `twig jira link-branch <ISSUE_KEY>` AND the target branch is already associated with the same issue
key THEN the command prints "Branch '{branch}' is already associated with issue {key}" as an informational message AND
no state change occurs

#### Scenario: Linking when the branch is already linked to a different issue

WHEN the user runs `twig jira link-branch <ISSUE_KEY>` AND the target branch is already associated with a different
issue THEN the command prints a warning that the branch is already associated with the existing issue and is being
updated AND the metadata is overwritten with the new issue key

#### Scenario: Linking with no issue key and no branch default

WHEN the user runs `twig jira link-branch` without an issue key AND the current branch has no associated Jira issue THEN
the command prints "No Jira issue key provided and current branch has no associated Jira issue"

#### Scenario: Linking when HEAD is detached

WHEN the user runs `twig jira link-branch` without a branch name AND HEAD is detached (not on a branch) THEN the command
prints "Not currently on a branch"

### Requirement: Transitioning a Jira issue

#### Scenario: Transitioning with an explicit transition name

WHEN the user runs `twig jira transition <ISSUE_KEY> <TRANSITION>` THEN the available transitions are fetched from the
Jira API AND the specified transition is matched by name (case-insensitive) or by ID AND the transition is executed via
the API AND the command prints "Successfully transitioned issue {key} to '{transition_name}'"

#### Scenario: Transitioning using the current branch default

WHEN the user runs `twig jira transition` without an issue key THEN the issue key is resolved from the current branch's
metadata AND the transition proceeds as if the key had been provided explicitly

#### Scenario: Listing available transitions when no transition is specified

WHEN the user runs `twig jira transition <ISSUE_KEY>` without a transition name THEN the available transitions are
fetched from the Jira API AND each transition is printed with its name and ID AND no transition is performed

#### Scenario: No transitions available

WHEN the user runs `twig jira transition <ISSUE_KEY>` without a transition name AND no transitions are available for the
issue THEN the command prints "No transitions available for this issue."

#### Scenario: Transition name not found

WHEN the user runs `twig jira transition <ISSUE_KEY> <TRANSITION>` AND no transition matches the provided name or ID
THEN the command prints "Transition '{transition_name}' not found for issue {issue_key}" AND lists the available
transitions

#### Scenario: Transition API call fails

WHEN the user runs `twig jira transition <ISSUE_KEY> <TRANSITION>` AND the Jira API call to execute the transition fails
THEN the command prints "Failed to transition issue: {error}"

### Requirement: Configuring Jira parsing settings

#### Scenario: Showing the current configuration

WHEN the user runs `twig jira config --show` THEN the current Jira parsing mode is loaded from
`${XDG_CONFIG_HOME}/twig/jira.toml` AND the command prints "Current Jira configuration:" followed by the parsing mode

#### Scenario: Setting the parsing mode

WHEN the user runs `twig jira config --mode <strict|flexible>` THEN the parsing mode is updated in
`${XDG_CONFIG_HOME}/twig/jira.toml` AND the command prints "Jira parsing mode set to: {mode}"

#### Scenario: Running config with no flags

WHEN the user runs `twig jira config` without `--show` or `--mode` THEN the command prints "Please specify --mode or
--show"

### Requirement: Credential and host resolution

#### Scenario: Resolving the Jira host from the environment variable

WHEN a Jira subcommand needs the Jira host THEN the `JIRA_HOST` environment variable is checked first AND if set, the
value is used as the base URL (with `https://` prepended if no scheme is present)

#### Scenario: Jira host environment variable is not set

WHEN a Jira subcommand needs the Jira host AND the `JIRA_HOST` environment variable is not set THEN the command fails
with "Jira host environment variable 'JIRA_HOST' not set"

#### Scenario: Jira credentials resolution

WHEN a Jira API call is made THEN credentials are read from `~/.netrc` for the configured Jira host AND if no
credentials are found for the specific host, a fallback lookup for `atlassian.net` is attempted AND credentials are used
for Basic authentication (username + API token)
