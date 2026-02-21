# Auto Sync

## Purpose

Scan branches and automatically detect and link them to Jira issues and GitHub PRs based on naming conventions and
remote tracking. Supports dry-run mode to preview changes, force mode to update existing associations, and selective
skipping of Jira or GitHub detection.

**CLI surface:** `twig sync`, flags: `--dry-run`, `--force`, `--no-jira`, `--no-github`, `-r` **Crates:** `twig-core`
(state, jira_parser, github), `twig-gh`, `twig-jira`, `twig-cli` (sync command module)

## Requirements

### Requirement: Repository resolution

#### Scenario: Auto-detecting the repository from the working directory

WHEN the user runs `twig sync` without `-r` THEN the repository is detected by traversing from the current working
directory upward using `detect_repository` AND if no repository is found, the command fails with a "No repository
specified and not in a git repository" error

#### Scenario: Overriding the repository path with `-r`

WHEN the user runs `twig sync -r <path>` THEN the command operates on the repository located at `<path>` instead of
auto-detecting from the working directory AND if the path does not exist, the command fails with a "Repository path does
not exist" error

#### Scenario: Repository cannot be opened

WHEN the repository path is resolved (via `-r` or auto-detection) AND `git2::Repository::open` fails THEN the command
fails with a "Failed to open git repository at <path>" error

### Requirement: Branch enumeration

#### Scenario: Collecting local branches

WHEN the sync command begins processing THEN all local branches are enumerated using `git2` with `BranchType::Local` AND
branches named "HEAD" or containing "origin/" are excluded from processing

#### Scenario: No local branches found

WHEN the repository has no local branches THEN the command prints "No local branches found to sync" AND exits
successfully without making any changes

### Requirement: Stale branch eviction

#### Scenario: Branches exist in state but not locally

WHEN the sync command loads repository state AND some branch metadata entries reference branches that no longer exist
locally THEN those stale entries and their orphaned dependencies are removed from state AND an informational message is
printed: "Cleaned up N stale branch entries and M orphaned dependencies" AND the eviction is persisted to disk even in
dry-run mode

### Requirement: Jira issue detection from branch names

#### Scenario: Branch name starts with issue key followed by slash

WHEN a branch is named like `PROJ-123/feature-name` THEN the Jira issue key `PROJ-123` is detected

#### Scenario: Branch name starts with issue key followed by hyphen

WHEN a branch is named like `PROJ-123-feature-name` THEN the Jira issue key `PROJ-123` is detected

#### Scenario: Issue key appears after a slash

WHEN a branch is named like `feature/PROJ-456-description` THEN the Jira issue key `PROJ-456` is detected

#### Scenario: Issue key appears in the middle separated by hyphens

WHEN a branch is named like `feature-ABC-789-description` THEN the Jira issue key `ABC-789` is detected

#### Scenario: Branch name is exactly an issue key

WHEN a branch is named like `PROJ-123` THEN the Jira issue key `PROJ-123` is detected

#### Scenario: Issue key appears after a trailing slash

WHEN a branch is named like `feature/PROJ-123` THEN the Jira issue key `PROJ-123` is detected

#### Scenario: Issue key project prefix must be at least two uppercase letters

WHEN a branch name contains a pattern like `P-123` (single uppercase letter prefix) THEN no Jira issue key is detected
AND WHEN a branch name contains a lowercase pattern like `proj-123` THEN no Jira issue key is detected

#### Scenario: Branch name has no recognizable Jira pattern

WHEN a branch is named like `feature-branch` or `main` THEN no Jira issue key is detected AND the branch is not flagged
as an error

### Requirement: GitHub PR detection from branch names

#### Scenario: Fetching open PRs by head branch

WHEN GitHub detection is active AND the origin remote URL can be parsed into an owner/repo pair THEN the GitHub API is
queried for pull requests matching each branch's name as the head branch AND if an open PR exists, its number is stored
AND if no open PR exists but closed PRs do, the most recent PR number is stored

#### Scenario: Origin remote URL cannot be resolved

WHEN GitHub detection is active AND the origin remote URL is missing or cannot be parsed into an owner/repo pair THEN
GitHub PR detection is skipped entirely with a warning: "Skipping GitHub PR detection because the origin remote URL is
missing or invalid" AND the sync continues with Jira detection only

#### Scenario: GitHub API call fails for a branch

WHEN the GitHub API request for a specific branch fails THEN no PR is associated with that branch AND the error is
silently ignored AND the sync continues processing remaining branches

#### Scenario: Parallel GitHub PR detection

WHEN GitHub PRs are being detected for multiple branches THEN all API requests are dispatched concurrently using async
tasks AND results are collected as each task completes

### Requirement: Skipping detection with flags

#### Scenario: Skipping Jira detection with `--no-jira`

WHEN the user runs `twig sync --no-jira` THEN Jira issue detection is skipped for all branches AND GitHub PR detection
proceeds normally (unless also disabled)

#### Scenario: Skipping GitHub detection with `--no-github`

WHEN the user runs `twig sync --no-github` THEN no GitHub client is created AND GitHub PR detection is skipped for all
branches AND Jira issue detection proceeds normally (unless also disabled)

#### Scenario: Skipping both Jira and GitHub detection

WHEN the user runs `twig sync --no-jira --no-github` THEN no detection is performed AND no new associations are created
AND stale branch eviction still occurs

### Requirement: New association creation

#### Scenario: Branch has detected patterns and no existing association

WHEN a branch has a detected Jira issue key, a detected GitHub PR number, or both AND the branch has no existing
metadata in the repository state THEN a new `BranchMetadata` entry is created with the detected values AND the entry's
`created_at` timestamp is set to the current UTC time in RFC 3339 format

#### Scenario: Branch has no detected patterns and no existing association

WHEN a branch has no detected Jira issue key and no detected GitHub PR number AND the branch has no existing metadata
THEN the branch is added to the unlinked branches list AND no metadata entry is created

### Requirement: Existing association handling without `--force`

#### Scenario: Existing association matches detected values

WHEN a branch already has metadata AND the detected Jira issue key matches the stored value AND the detected GitHub PR
number matches the stored value THEN no update is performed AND the branch is not reported as changed

#### Scenario: Existing association has missing fields that can be filled

WHEN a branch already has metadata AND the existing metadata has a missing Jira issue or missing GitHub PR AND the sync
detects the missing value THEN the metadata is updated to include the newly detected value AND this is not treated as a
conflict

#### Scenario: Existing association conflicts with detected values

WHEN a branch already has metadata AND the detected Jira issue key or GitHub PR number differs from the stored value AND
`--force` is not set THEN the conflict is reported with both existing and detected values AND the user is advised to
"Use --force to update conflicting associations" AND the metadata is not modified

#### Scenario: Branch has existing association but no patterns detected

WHEN a branch already has metadata AND no Jira issue key or GitHub PR is detected from the branch name THEN the existing
metadata is left unchanged

### Requirement: Force mode

#### Scenario: Force updating conflicting associations

WHEN the user runs `twig sync --force` AND a branch's detected values conflict with its existing stored values THEN the
metadata is overwritten with the detected values AND the update is reported in the summary as an updated association

### Requirement: Dry-run mode

#### Scenario: Dry-run previews new associations

WHEN the user runs `twig sync --dry-run` THEN the command prints "Running in dry-run mode - no changes will be made" AND
new associations are reported with "Would create" instead of "Creating" AND no new metadata is persisted to the state
file

#### Scenario: Dry-run previews updates to existing associations

WHEN the user runs `twig sync --dry-run` AND existing associations would be updated THEN updates are reported with
"Would update" instead of "Updating" AND no metadata modifications are persisted to the state file

#### Scenario: Dry-run still evicts stale branches

WHEN the user runs `twig sync --dry-run` AND stale branch entries exist in the state THEN the stale entries are evicted
and saved to disk even in dry-run mode because eviction is background cleanup

### Requirement: Sync summary output

#### Scenario: New associations detected

WHEN sync completes with new associations THEN a success message is printed: "Creating N new associations:" (or "Would
create" in dry-run) AND each association is listed with the branch name and its detected Jira issue and/or PR number

#### Scenario: Associations updated

WHEN sync completes with updated associations THEN a success message is printed: "Updating N existing associations:" (or
"Would update" in dry-run) AND each update shows the branch name with the old and new values for changed fields

#### Scenario: Conflicts found

WHEN sync completes with conflicting associations THEN a warning is printed: "Found N conflicting associations:" AND
each conflict shows the branch name with existing and detected values for the conflicting fields AND an informational
message is printed: "Use --force to update conflicting associations"

#### Scenario: Unlinked branches found

WHEN sync completes with branches that have no detectable patterns THEN an informational message is printed: "Found N
branches without detectable patterns:" AND each unlinked branch name is listed AND guidance is printed for manual
linking via `twig jira branch link` and `twig github pr link`

#### Scenario: All branches already linked

WHEN sync completes AND no new associations, updates, or conflicts are found THEN a success message is printed: "All
branches are already properly linked!"

### Requirement: State persistence

#### Scenario: Saving changes after sync

WHEN sync completes (not in dry-run mode) AND new or updated associations were created OR stale branch eviction occurred
THEN the updated state is saved to `.twig/state.json` AND a success message is printed: "Successfully saved branch
associations"

#### Scenario: No changes to save

WHEN sync completes AND no new or updated associations were created AND no stale branch eviction occurred THEN the state
file is not written

### Requirement: Idempotency

#### Scenario: Running sync twice produces the same result

WHEN sync is run AND then run again immediately without any changes to branches or remote PRs THEN the second run
produces no new associations, no updates, and no conflicts AND the summary reports "All branches are already properly
linked!"

### Requirement: GitHub client creation

#### Scenario: GitHub client created from netrc credentials

WHEN GitHub detection is active (`--no-github` not set) THEN a GitHub client is created using credentials from the
user's `~/.netrc` file via `create_github_client_from_netrc`

#### Scenario: GitHub client creation fails

WHEN GitHub detection is active AND the GitHub client cannot be created (e.g., missing netrc credentials) THEN the
command fails with an error from the client creation step
