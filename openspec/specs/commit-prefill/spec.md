# Commit Prefill

## Purpose

Create git commits with messages automatically prefilled from the Jira issue linked to the current branch. Formats
commit messages with the issue key and summary, with optional prefix/suffix customization. Detects duplicate commit
messages and offers fixup instead.

**CLI surface:** `twig commit`, flags: `-m/--message`, `-p/--prefix`, `-s/--suffix`, `--no-fixup` **Crates:**
`twig-core` (state, utils), `twig-jira` (issue fetch), `twig-cli` (commit command module)

## Requirements

### Requirement: Repository and branch validation

#### Scenario: Not in a git repository

WHEN the user runs `twig commit` AND the current directory is not inside a git repository THEN twig exits with an error
indicating "Not in a git repository"

#### Scenario: No Jira issue linked to current branch

WHEN the user runs `twig commit` AND the current branch has no Jira issue associated in the repo state THEN twig prints
an error "No Jira issue associated with the current branch." AND prints guidance to link a Jira issue with
`twig jira branch link <issue-key>` AND exits without creating a commit

### Requirement: Jira issue resolution

#### Scenario: Jira host not configured

WHEN the user runs `twig commit` AND the `$JIRA_HOST` environment variable is not set THEN twig exits with an error
about the missing environment variable

#### Scenario: Jira issue fetch failure

WHEN the user runs `twig commit` AND the Jira API call to fetch the issue fails (network error, invalid credentials, or
issue not found) THEN twig exits with an error "Failed to fetch Jira issue {ISSUE-KEY}"

#### Scenario: Successful Jira issue fetch

WHEN the user runs `twig commit` AND the current branch has a linked Jira issue AND the Jira API successfully returns
the issue details THEN twig uses the issue key and the `fields.summary` from the API response to generate the commit
message

### Requirement: Commit message generation (default)

#### Scenario: Default message format

WHEN the user runs `twig commit` without `-m`, `-p`, or `-s` flags THEN twig generates a commit message in the format
`{ISSUE-KEY}: {Issue summary}` where the issue key and summary come from the linked Jira issue

### Requirement: Custom message override (`-m/--message`)

#### Scenario: Custom message replaces summary

WHEN the user runs `twig commit -m "Custom text"` THEN twig generates a commit message in the format
`{ISSUE-KEY}: Custom text` AND the Jira issue summary is not used in the message

#### Scenario: Custom message ignores prefix and suffix

WHEN the user runs `twig commit -m "Custom text" -p "WIP" -s "[ci skip]"` THEN twig generates a commit message in the
format `{ISSUE-KEY}: Custom text` AND the prefix and suffix flags are ignored

### Requirement: Prefix (`-p/--prefix`)

#### Scenario: Prefix added between key and summary

WHEN the user runs `twig commit -p "WIP"` THEN twig generates a commit message in the format
`{ISSUE-KEY}: WIP {Issue summary}` where the prefix is inserted after the key separator and before the issue summary
with a space separating them

### Requirement: Suffix (`-s/--suffix`)

#### Scenario: Suffix appended to message

WHEN the user runs `twig commit -s "[ci skip]"` THEN twig generates a commit message in the format
`{ISSUE-KEY}: {Issue summary} [ci skip]` where the suffix is appended after the issue summary with a space separating
them

### Requirement: Prefix and suffix combined

#### Scenario: Both prefix and suffix applied

WHEN the user runs `twig commit -p "WIP" -s "[ci skip]"` THEN twig generates a commit message in the format
`{ISSUE-KEY}: WIP {Issue summary} [ci skip]`

### Requirement: Duplicate commit detection and fixup

#### Scenario: No duplicate found

WHEN the user runs `twig commit` AND no commit in the last 20 commits of the current branch has an identical subject
line THEN twig creates a normal commit with `git commit -m "{message}"`

#### Scenario: Duplicate found and user accepts fixup

WHEN the user runs `twig commit` AND a commit with an identical subject line exists in the last 20 commits THEN twig
prints a warning "A commit with the message '{message}' already exists in recent history." AND prompts the user with
"Create a fixup commit instead? \[y/N\]: " AND when the user responds "y" or "yes" (case-insensitive) THEN twig finds
the short hash of the matching commit from the last 20 commits AND creates a fixup commit with
`git commit --fixup {hash}`

#### Scenario: Duplicate found and user declines fixup

WHEN the user runs `twig commit` AND a commit with an identical subject line exists in the last 20 commits AND the user
responds with anything other than "y" or "yes" to the fixup prompt THEN twig creates a normal commit with
`git commit -m "{message}"`

#### Scenario: Duplicate detection searches last 20 commits

WHEN twig checks for duplicate commit messages THEN it runs `git log --pretty=format:%s -n 20` AND compares each subject
line for exact string equality with the generated message

#### Scenario: Fixup commit hash resolution

WHEN twig creates a fixup commit THEN it runs `git log --pretty=format:%h %s -n 20` to retrieve short hashes AND finds
the first commit whose subject line exactly matches the generated message AND uses that short hash as the fixup target

### Requirement: Disable fixup detection (`--no-fixup`)

#### Scenario: Fixup detection skipped

WHEN the user runs `twig commit --no-fixup` THEN twig creates a normal commit with `git commit -m "{message}"` without
checking recent history for duplicates AND without prompting the user

### Requirement: Commit execution

#### Scenario: Successful normal commit

WHEN `git commit -m "{message}"` succeeds THEN twig prints an info line "Creating commit with message: '{message}'" AND
prints a success message "Commit created successfully." AND displays the git commit output

#### Scenario: Failed normal commit (e.g., nothing staged)

WHEN `git commit -m "{message}"` fails (non-zero exit code) THEN twig prints an error "Failed to create commit." AND
displays git's stderr output AND exits with an error

#### Scenario: Successful fixup commit

WHEN `git commit --fixup {hash}` succeeds THEN twig prints a success message "Fixup commit created successfully." AND
the git output is logged at debug verbosity level

#### Scenario: Failed fixup commit

WHEN `git commit --fixup {hash}` fails THEN twig prints an error "Failed to create fixup commit." AND the git stderr is
logged at warn verbosity level AND twig exits with an error
