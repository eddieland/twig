# Credentials

## Purpose

Manage authentication credentials for GitHub and Jira. Check if credentials are properly configured and set them up
interactively. Supports ~/.netrc file format and platform-specific credential stores (Unix keyring, Windows Credential
Manager).

**CLI surface:** `twig creds check`, `twig creds setup` **Crates:** `twig-core` (creds module, netrc parser, platform
backends), `twig-cli` (creds command module)

## CLI Commands

### Requirement: Credential check

#### Scenario: No netrc file present

WHEN the user runs `twig creds check` AND no `~/.netrc` file exists THEN twig reports that no .netrc file was found AND
provides guidance on creating one AND does not check individual service credentials

#### Scenario: Netrc file exists

WHEN the user runs `twig creds check` AND a `~/.netrc` file exists THEN twig checks file permissions, Jira credentials,
and GitHub credentials AND displays an example `.netrc` format block showing entries for `atlassian.net` and
`github.com`

#### Scenario: File permission verification

WHEN file permissions are checked on Unix AND the file has mode 0600 THEN twig reports secure permissions AND when the
file has group or other bits set THEN twig warns about insecure permissions and suggests `chmod 600`

WHEN file permissions are checked on Windows THEN twig warns that secure file permissions are not fully supported and
suggests using Windows Credential Manager

#### Scenario: Service credential detection

WHEN twig checks for Jira credentials AND `$JIRA_HOST` is set THEN it reports whether credentials were found for the
configured host AND when `$JIRA_HOST` is not set THEN it reports an error about the missing host variable

WHEN twig checks for GitHub credentials THEN it reports whether credentials were found for machine `github.com`

### Requirement: Credential setup wizard

#### Scenario: Platform-specific welcome guidance

WHEN the user runs `twig creds setup` THEN twig displays a welcome message AND on Unix, informs the user that
credentials will be stored in `~/.netrc` with permissions set to 600 AND on Windows, informs the user that Windows
Credential Manager will be the primary store with `~/.netrc` as fallback

#### Scenario: Existing netrc confirmation

WHEN `~/.netrc` already exists THEN twig prompts the user for confirmation before proceeding AND if the user declines,
the wizard exits without modifying credentials

#### Scenario: Empty required fields skip the service

WHEN any required field (email, API token, domain for Jira; username, PAT for GitHub) is left empty THEN twig warns that
the field cannot be empty, skips setup for that service, and informs the user they can retry later

#### Scenario: Jira domain URL normalization

WHEN the user provides a Jira domain without a protocol prefix THEN twig prepends `https://` before validation AND when
the domain starts with `http` THEN twig uses it as-is

#### Scenario: Credential validation (common behavior)

WHEN valid credentials are provided for a service AND the API connection test succeeds THEN twig reports successful
validation AND writes the credentials to the netrc file (machine `atlassian.net` for Jira, machine `github.com` for
GitHub)

WHEN the API connection test fails (returns false or a network error) THEN twig reports the failure with troubleshooting
guidance AND does NOT write credentials AND suggests manual `.netrc` configuration

#### Scenario: Post-setup permissions and completion

WHEN the wizard completes on Unix AND credentials were written THEN twig sets `~/.netrc` permissions to 0600 AND on
Windows, twig informs the user about the Credential Manager / netrc fallback arrangement AND on all platforms, twig
suggests running `twig creds check` to verify

## Core: Netrc Parser (`twig-core`)

### Requirement: Netrc file parsing

#### Scenario: Format flexibility

WHEN a `.netrc` file contains entries in standard multi-line, single-line, or mixed formats THEN the parser correctly
extracts machine, login, and password for all entries AND unrecognized tokens are skipped without error

#### Scenario: Machine not found or incomplete entry

WHEN a lookup is performed for a machine not in the file THEN the parser returns `None` AND when a machine entry is
missing either login or password THEN the parser treats it as incomplete and does not return credentials

#### Scenario: No netrc file

WHEN `~/.netrc` does not exist THEN the provider returns `None` without error

### Requirement: Netrc file writing

#### Scenario: Writing credentials

WHEN a credential is written for a new machine THEN the entry is appended (creating the file if needed) with secure
permissions (0600 on Unix) AND when a credential is written for an existing machine THEN the entry is updated in-place,
preserving other entries

#### Scenario: Trailing newline handling

WHEN appending to an existing file that does not end with a newline THEN a newline is added before the new entry

## Core: Host Normalization (`twig-core`)

### Requirement: Jira host normalization

#### Scenario: Protocol and trailing slash stripping

WHEN a Jira host URL includes `https://` or `http://` THEN the normalization strips the protocol prefix and any trailing
slash AND when no prefix is present THEN the host is returned as-is (minus any trailing slash)

## Core: Credential Retrieval (`twig-core`, `twig-gh`, `twig-jira`)

### Requirement: GitHub credential retrieval

#### Scenario: Credentials found

WHEN `get_github_credentials` is called AND the provider has credentials for machine `github.com` THEN it returns a
`Credentials` struct with username and password

#### Scenario: Credentials not found

WHEN no credentials exist for `github.com` THEN an error is returned directing the user to add credentials (on Unix, via
`.netrc`; on Windows, via `twig creds setup`)

### Requirement: Jira credential retrieval with fallback

#### Scenario: Exact host match

WHEN `get_jira_credentials` is called AND credentials exist for the normalized host THEN those credentials are returned

#### Scenario: Fallback to atlassian.net

WHEN no credentials exist for the normalized host AND credentials exist for `atlassian.net` THEN the `atlassian.net`
credentials are returned

#### Scenario: No credentials found

WHEN no credentials exist for either the normalized host or `atlassian.net` THEN an error is returned directing the user
to add credentials (on Unix, via `.netrc`; on Windows, via `twig creds setup`)

### Requirement: Client creation from credentials

#### Scenario: GitHub client

WHEN `create_github_client_from_netrc` is called AND credentials exist THEN it returns an authenticated `GitHubClient`

#### Scenario: Jira client

WHEN `create_jira_client_from_netrc` is called AND credentials exist THEN it returns an authenticated `JiraClient` using
the host URL and credentials

#### Scenario: Runtime and client pair

WHEN `create_*_runtime_and_client` is called THEN it returns a Tokio runtime paired with the authenticated client

### Requirement: Jira host resolution

#### Scenario: JIRA_HOST set

WHEN `get_jira_host()` is called AND `$JIRA_HOST` is set THEN it returns the value with `https://` prepended if no
scheme is present

#### Scenario: JIRA_HOST not set

WHEN `$JIRA_HOST` is not set THEN an error is returned indicating the environment variable is missing

## Core: Platform Credential Providers (`twig-core`)

### Requirement: Provider selection

#### Scenario: Platform dispatch

WHEN `get_credential_provider` is called on Unix THEN it returns a `NetrcCredentialProvider` AND on Windows it returns a
`WindowsCredentialProvider` AND both are initialized with the user's home directory

### Requirement: Unix provider (NetrcCredentialProvider)

#### Scenario: Read and store

WHEN the Unix provider looks up credentials THEN it parses `~/.netrc` for the requested machine AND when it stores
credentials THEN it writes/updates the entry and sets file permissions to 0600

### Requirement: Windows provider (WindowsCredentialProvider)

#### Scenario: Lookup with fallback

WHEN the Windows provider looks up credentials THEN it first checks Windows Credential Manager using target name
`twig:<service>` AND if not found (or error occurs), falls back to parsing `~/.netrc` AND if neither source has
credentials, returns `None`

#### Scenario: Store to Credential Manager

WHEN the Windows provider stores credentials THEN it writes to Windows Credential Manager with persistence set to
`CRED_PERSIST_LOCAL_MACHINE`

#### Scenario: Empty credentials treated as missing

WHEN a Credential Manager entry has an empty username or password THEN the provider returns `None`

### Requirement: File permissions

#### Scenario: Unix permissions

WHEN `set_secure_permissions` is called on Unix THEN file mode is set to 0600 AND `has_secure_permissions` returns
`true` only if no group or other bits are set

#### Scenario: Windows permissions are no-ops

WHEN `set_secure_permissions` is called on Windows THEN it is a no-op AND `has_secure_permissions` returns `true` if the
file exists and is readable

### Requirement: Credential data model

#### Scenario: Credentials struct

WHEN credentials are loaded from any provider THEN they are represented as a `Credentials` struct with `username`
(String) and `password` (String) fields
