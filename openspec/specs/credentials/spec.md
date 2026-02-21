# Credentials

## Purpose

Manage authentication credentials for GitHub and Jira. Check if credentials are properly configured and set them up
interactively. Supports ~/.netrc file format and platform-specific credential stores (Unix keyring, Windows Credential
Manager).

**CLI surface:** `twig creds check`, `twig creds setup` **Crates:** `twig-core` (creds module, netrc parser, platform
backends), `twig-cli` (creds command module)

## Requirements

### Requirement: Credential check — netrc file existence

#### Scenario: No netrc file present

WHEN the user runs `twig creds check` AND no `~/.netrc` file exists THEN twig prints an error "No .netrc file found."
AND prints guidance to create a `.netrc` file at the expected path AND does not check individual service credentials

#### Scenario: Netrc file exists

WHEN the user runs `twig creds check` AND a `~/.netrc` file exists THEN twig proceeds to check file permissions, Jira
credentials, and GitHub credentials

### Requirement: Credential check — file permission verification (Unix)

#### Scenario: Secure permissions on Unix

WHEN the user runs `twig creds check` on a Unix system AND the `~/.netrc` file has mode 0600 (owner read/write only, no
group/other bits) THEN twig prints a success message ".netrc file has secure permissions."

#### Scenario: Insecure permissions on Unix

WHEN the user runs `twig creds check` on a Unix system AND the `~/.netrc` file has group or other permission bits set
(mode & 0o077 != 0) THEN twig prints a warning "Your .netrc file has insecure permissions." AND prints guidance to run
`chmod 600 <path>` to fix permissions

### Requirement: Credential check — file permission verification (Windows)

#### Scenario: Permission check on Windows

WHEN the user runs `twig creds check` on a Windows system AND the `~/.netrc` file exists THEN twig prints a warning
"Secure file permissions are not fully supported on Windows." AND prints a warning "Your .netrc file may not be properly
secured." AND suggests using Windows Credential Manager instead

### Requirement: Credential check — Jira credential detection

#### Scenario: Jira credentials found

WHEN the user runs `twig creds check` AND the `$JIRA_HOST` environment variable is set AND credentials exist in the
credential provider for the configured Jira host THEN twig prints a success message "Jira credentials found."

#### Scenario: Jira credentials not found

WHEN the user runs `twig creds check` AND the `$JIRA_HOST` environment variable is set AND no credentials exist for the
configured Jira host THEN twig prints a warning "No Jira credentials found." AND prints guidance to add credentials for
machine 'atlassian.net' to the `.netrc` file

#### Scenario: Jira host not configured

WHEN the user runs `twig creds check` AND the `$JIRA_HOST` environment variable is not set THEN twig prints an error
about being unable to get the Jira host

### Requirement: Credential check — GitHub credential detection

#### Scenario: GitHub credentials found

WHEN the user runs `twig creds check` AND credentials exist in the credential provider for machine `github.com` THEN
twig prints a success message "GitHub credentials found."

#### Scenario: GitHub credentials not found

WHEN the user runs `twig creds check` AND no credentials exist for machine `github.com` THEN twig prints a warning "No
GitHub credentials found." AND prints guidance to add credentials for machine 'github.com' to the `.netrc` file

### Requirement: Credential check — example format display

#### Scenario: Check command always displays example format

WHEN the user runs `twig creds check` AND the `~/.netrc` file exists THEN twig always prints an example `.netrc` format
block showing entries for both `atlassian.net` (login + password) and `github.com` (login + password)

### Requirement: Credential setup — welcome and platform guidance

#### Scenario: Setup wizard on Unix

WHEN the user runs `twig creds setup` on a Unix system THEN twig displays a welcome message for the credential setup
wizard AND informs the user that credentials will be stored in `~/.netrc` AND informs the user that file permissions
will be automatically set to 600

#### Scenario: Setup wizard on Windows

WHEN the user runs `twig creds setup` on a Windows system THEN twig displays a welcome message for the credential setup
wizard AND informs the user that credentials will be stored in Windows Credential Manager AND informs the user that
`~/.netrc` will be used as a fallback if it exists

### Requirement: Credential setup — existing netrc confirmation

#### Scenario: Netrc file already exists and user confirms

WHEN the user runs `twig creds setup` AND a `~/.netrc` file already exists AND the user responds 'y' to the confirmation
prompt THEN twig proceeds with the setup wizard to add/update credentials

#### Scenario: Netrc file already exists and user declines

WHEN the user runs `twig creds setup` AND a `~/.netrc` file already exists AND the user does not respond with a value
starting with 'y' (case-insensitive) THEN twig prints "Setup cancelled." AND exits without modifying any credentials

### Requirement: Credential setup — Jira credential collection

#### Scenario: Jira email is empty

WHEN the user runs `twig creds setup` AND the user provides an empty email when prompted for Jira credentials THEN twig
prints a warning "Email cannot be empty. Skipping Jira setup." AND informs the user they can run `twig creds setup`
again later AND proceeds to GitHub credential setup

#### Scenario: Jira API token is empty

WHEN the user runs `twig creds setup` AND the user provides a non-empty email AND the user provides an empty API token
THEN twig prints a warning "API token cannot be empty. Skipping Jira setup." AND informs the user they can run
`twig creds setup` again later AND proceeds to GitHub credential setup

#### Scenario: Jira domain is empty

WHEN the user runs `twig creds setup` AND the user provides a non-empty email and API token AND the user provides an
empty domain THEN twig prints a warning "Domain cannot be empty. Skipping Jira setup." AND informs the user they can run
`twig creds setup` again later AND proceeds to GitHub credential setup

#### Scenario: Jira domain URL normalization

WHEN the user runs `twig creds setup` AND the user provides a Jira domain without an `http` or `https` prefix THEN twig
prepends `https://` to the domain before validation

WHEN the user provides a Jira domain that starts with `http` THEN twig uses the domain as-is for validation

### Requirement: Credential setup — Jira credential validation

#### Scenario: Jira credentials validated successfully

WHEN the user runs `twig creds setup` AND the user provides email, API token, and domain AND the Jira API connection
test returns success (via `client.test_connection()` to `/rest/api/2/myself`) THEN twig prints "Jira credentials
validated successfully!" AND writes a netrc entry for machine `atlassian.net` with the provided email as login and API
token as password

#### Scenario: Jira credentials validation fails

WHEN the user runs `twig creds setup` AND the user provides email, API token, and domain AND the Jira API connection
test returns false THEN twig prints an error "Failed to validate Jira credentials." AND prints common troubleshooting
steps (check email, verify API token, check domain) AND suggests manually adding credentials to `.netrc` later AND does
NOT write any netrc entry for Jira

#### Scenario: Jira credentials validation network error

WHEN the user runs `twig creds setup` AND the user provides email, API token, and domain AND the Jira API connection
test returns an error THEN twig prints the error message AND suggests it might be a network issue or the Jira instance
might be unreachable AND suggests manually adding credentials to `.netrc` later AND does NOT write any netrc entry for
Jira

### Requirement: Credential setup — GitHub credential collection

#### Scenario: GitHub username is empty

WHEN the user runs `twig creds setup` AND the user provides an empty username when prompted for GitHub credentials THEN
twig prints a warning "Username cannot be empty. Skipping GitHub setup." AND informs the user they can run
`twig creds setup` again later

#### Scenario: GitHub Personal Access Token is empty

WHEN the user runs `twig creds setup` AND the user provides a non-empty username AND the user provides an empty Personal
Access Token THEN twig prints a warning "Personal Access Token cannot be empty. Skipping GitHub setup." AND informs the
user they can run `twig creds setup` again later

### Requirement: Credential setup — GitHub credential validation

#### Scenario: GitHub credentials validated successfully

WHEN the user runs `twig creds setup` AND the user provides username and Personal Access Token AND the GitHub API
connection test returns success (via `client.test_connection()` to `/user`) THEN twig prints "GitHub credentials
validated successfully!" AND writes a netrc entry for machine `github.com` with the provided username as login and token
as password

#### Scenario: GitHub credentials validation fails

WHEN the user runs `twig creds setup` AND the user provides username and Personal Access Token AND the GitHub API
connection test returns false THEN twig prints an error "Failed to validate GitHub credentials." AND prints common
troubleshooting steps (check username, verify token, check required scopes: repo, read:user) AND suggests manually
adding credentials to `.netrc` later AND does NOT write any netrc entry for GitHub

#### Scenario: GitHub credentials validation network error

WHEN the user runs `twig creds setup` AND the user provides username and Personal Access Token AND the GitHub API
connection test returns an error THEN twig prints the error message AND suggests it might be a network issue or GitHub
might be unreachable AND suggests manually adding credentials to `.netrc` later AND does NOT write any netrc entry for
GitHub

### Requirement: Credential setup — post-setup permissions

#### Scenario: Set secure permissions after setup on Unix

WHEN the user runs `twig creds setup` on a Unix system AND at least one credential was written to `~/.netrc` AND the
`~/.netrc` file exists at the end of setup THEN twig sets the file permissions to 0600 (owner read/write only) AND
prints "Set secure permissions on .netrc file (600)."

#### Scenario: Post-setup message on Windows

WHEN the user runs `twig creds setup` on a Windows system AND the `~/.netrc` file exists at the end of setup THEN twig
informs the user that the existing `.netrc` will be used as a fallback AND informs the user that Windows Credential
Manager will be the primary credential store

#### Scenario: Setup completion message

WHEN the user runs `twig creds setup` AND the wizard completes (regardless of which services were configured) THEN twig
prints "Credential setup complete!" AND suggests running `twig creds check` to verify credentials

### Requirement: Netrc file parsing

#### Scenario: Standard multi-line format

WHEN a `.netrc` file contains entries in standard multi-line format (machine, login, password on separate lines) AND a
credential lookup is performed for a matching machine name THEN the parser returns the corresponding login as username
and password as password

#### Scenario: Single-line format

WHEN a `.netrc` file contains entries with machine, login, and password all on the same line AND a credential lookup is
performed for a matching machine name THEN the parser returns the credentials correctly

#### Scenario: Mixed format

WHEN a `.netrc` file contains entries in a mix of single-line, multi-line, and partially split formats THEN the parser
correctly extracts credentials for all entries regardless of format

#### Scenario: Machine not found

WHEN a `.netrc` file is parsed for a machine name that does not exist in the file THEN the parser returns `None`

#### Scenario: Incomplete entry (missing password)

WHEN a `.netrc` file contains a machine entry with a login but no password THEN the parser does not return credentials
for that machine AND the parser considers the entry incomplete

#### Scenario: Incomplete entry (missing login)

WHEN a `.netrc` file contains a machine entry with a password but no login THEN the parser does not return credentials
for that machine

#### Scenario: Empty netrc file

WHEN a `.netrc` file is empty AND a credential lookup is performed THEN the parser returns `None`

#### Scenario: Netrc file does not exist

WHEN the `~/.netrc` file does not exist AND a credential lookup is performed via the Unix `NetrcCredentialProvider` THEN
the provider returns `None` without error

#### Scenario: Multiple machines in file

WHEN a `.netrc` file contains entries for multiple machines AND a credential lookup is performed for any specific
machine THEN the parser returns only the credentials for the requested machine

#### Scenario: Malformed lines are skipped

WHEN a `.netrc` file contains unrecognized tokens or comment-like lines THEN the parser skips unrecognized tokens AND
still correctly parses valid `machine`, `login`, and `password` entries

### Requirement: Netrc file writing

#### Scenario: Write entry to new file

WHEN a credential is written for a machine AND the `~/.netrc` file does not yet exist THEN the writer creates the file
AND writes the machine, login, and password in multi-line format AND sets secure file permissions (0600 on Unix)

#### Scenario: Append entry to existing file

WHEN a credential is written for a new machine AND the `~/.netrc` file already exists with other entries THEN the writer
appends the new entry to the end of the file AND preserves all existing entries

#### Scenario: Update existing entry

WHEN a credential is written for a machine that already has an entry in `~/.netrc` THEN the writer replaces the login
and password for that machine AND preserves all other machine entries unchanged

#### Scenario: Trailing newline handling on append

WHEN a credential is appended to an existing `~/.netrc` file AND the existing file does not end with a newline THEN the
writer adds a newline before the new entry to maintain proper formatting

### Requirement: Host normalization

#### Scenario: Normalize host with https prefix

WHEN a Jira host URL includes an `https://` prefix THEN the normalization strips the prefix and any trailing slash

#### Scenario: Normalize host with http prefix

WHEN a Jira host URL includes an `http://` prefix THEN the normalization strips the prefix and any trailing slash

#### Scenario: Normalize host without prefix

WHEN a Jira host URL has no protocol prefix THEN the normalization returns the host as-is (minus any trailing slash)

### Requirement: GitHub credential retrieval

#### Scenario: GitHub credentials found in provider

WHEN `get_github_credentials` is called AND the credential provider has credentials for machine `github.com` THEN it
returns the `Credentials` struct with username and password

#### Scenario: GitHub credentials not found on Unix

WHEN `get_github_credentials` is called on a Unix system AND no credentials exist for machine `github.com` THEN it
returns an error: "GitHub credentials not found in .netrc file. Please add credentials for machine 'github.com'."

#### Scenario: GitHub credentials not found on Windows

WHEN `get_github_credentials` is called on a Windows system AND no credentials exist for machine `github.com` in either
Windows Credential Manager or `.netrc` THEN it returns an error: "GitHub credentials not found. Please run 'twig creds
setup' to configure credentials for 'github.com'."

### Requirement: GitHub client creation from credentials

#### Scenario: Create GitHub client from netrc

WHEN `create_github_client_from_netrc` is called AND GitHub credentials exist for machine `github.com` THEN it returns
an authenticated `GitHubClient` using the username and password (PAT) from the credentials

#### Scenario: Create GitHub runtime and client

WHEN `create_github_runtime_and_client` is called AND GitHub credentials exist THEN it returns a tuple of a new Tokio
runtime and an authenticated `GitHubClient`

### Requirement: Jira credential retrieval with fallback

#### Scenario: Jira credentials found for exact host

WHEN `get_jira_credentials` is called with a Jira host AND the credential provider has credentials for the normalized
host (protocol and trailing slash stripped) THEN it returns the credentials for that exact host

#### Scenario: Jira credentials fallback to atlassian.net

WHEN `get_jira_credentials` is called with a Jira host AND no credentials exist for the normalized host AND credentials
exist for machine `atlassian.net` THEN it returns the credentials for `atlassian.net` as a fallback

#### Scenario: Jira credentials not found on Unix

WHEN `get_jira_credentials` is called on a Unix system AND no credentials exist for the normalized host AND no
credentials exist for `atlassian.net` THEN it returns an error: "Jira credentials not found in .netrc file. Please add
credentials for machine '<host>' or 'atlassian.net'."

#### Scenario: Jira credentials not found on Windows

WHEN `get_jira_credentials` is called on a Windows system AND no credentials exist for the normalized host AND no
credentials exist for `atlassian.net` THEN it returns an error: "Jira credentials not found. Please run 'twig creds
setup' to configure credentials for '<host>' or 'atlassian.net'."

### Requirement: Jira client creation from credentials

#### Scenario: Create Jira client from netrc

WHEN `create_jira_client_from_netrc` is called with a Jira host AND Jira credentials exist (either for the exact host or
via `atlassian.net` fallback) THEN it returns an authenticated `JiraClient` using the host URL, username, and password
(API token)

#### Scenario: Create Jira runtime and client

WHEN `create_jira_runtime_and_client` is called AND Jira credentials exist THEN it returns a tuple of a new Tokio
runtime and an authenticated `JiraClient`

### Requirement: Jira host resolution

#### Scenario: JIRA_HOST environment variable is set

WHEN `get_jira_host()` is called AND the `$JIRA_HOST` environment variable is set THEN it returns the value with a URL
scheme ensured (defaults to `https://` if no scheme is provided)

#### Scenario: JIRA_HOST environment variable is not set

WHEN `get_jira_host()` is called AND the `$JIRA_HOST` environment variable is not set THEN it returns an error: "Jira
host environment variable 'JIRA_HOST' not set"

### Requirement: Platform credential provider — Unix

#### Scenario: Unix provider reads from netrc

WHEN the Unix `NetrcCredentialProvider` is used THEN it reads credentials by parsing the `~/.netrc` file for the
requested machine name

#### Scenario: Unix provider stores to netrc with secure permissions

WHEN the Unix `NetrcCredentialProvider` stores credentials THEN it writes/updates the entry in `~/.netrc` AND sets file
permissions to 0600

### Requirement: Platform credential provider — Windows

#### Scenario: Windows provider checks Credential Manager first

WHEN the Windows `WindowsCredentialProvider` looks up credentials for a service THEN it first checks Windows Credential
Manager using target name `twig:<service>` AND if found, returns those credentials

#### Scenario: Windows provider falls back to netrc

WHEN the Windows `WindowsCredentialProvider` looks up credentials for a service AND no credentials are found in Windows
Credential Manager (or an error occurs) AND a `~/.netrc` file exists THEN it falls back to parsing the `.netrc` file for
the service

#### Scenario: Windows provider returns None when neither source has credentials

WHEN the Windows `WindowsCredentialProvider` looks up credentials for a service AND no credentials are found in Windows
Credential Manager AND no `~/.netrc` file exists THEN it returns `None`

#### Scenario: Windows provider stores to Credential Manager

WHEN the Windows `WindowsCredentialProvider` stores credentials for a service THEN it writes the credentials to Windows
Credential Manager using target name `twig:<service>` AND persistence is set to `CRED_PERSIST_LOCAL_MACHINE`

#### Scenario: Windows Credential Manager entry has empty username or password

WHEN the Windows `WindowsCredentialProvider` reads a credential from Windows Credential Manager AND either the username
or password is empty THEN it returns `None` for that credential (treats it as not found)

### Requirement: Platform credential provider selection

#### Scenario: Provider selection on Unix

WHEN `get_credential_provider` is called on a Unix system THEN it returns a `NetrcCredentialProvider` initialized with
the user's home directory

#### Scenario: Provider selection on Windows

WHEN `get_credential_provider` is called on a Windows system THEN it returns a `WindowsCredentialProvider` initialized
with the user's home directory

### Requirement: Credential data model

#### Scenario: Credentials struct

WHEN credentials are loaded from any provider THEN they are represented as a `Credentials` struct with `username`
(String) and `password` (String) fields

### Requirement: File permissions trait

#### Scenario: Unix secure permissions are mode 0600

WHEN `UnixFilePermissions::set_secure_permissions` is called on a file THEN the file mode is set to 0600 (owner
read/write only)

WHEN `UnixFilePermissions::has_secure_permissions` is called on a file THEN it returns `true` only if the file's mode
has no group or other permission bits set (mode & 0o077 == 0)

#### Scenario: Windows file permissions are no-ops

WHEN `WindowsFilePermissions::set_secure_permissions` is called THEN it returns `Ok(())` without modifying any file
permissions

WHEN `WindowsFilePermissions::has_secure_permissions` is called THEN it returns `true` if the file exists and is
readable, `false` otherwise
