# Self Management

## Purpose

Twig maintenance utilities: update twig and its plugins to the latest release from GitHub, run system diagnostics to
verify the environment, generate shell completions for bash/zsh/fish/powershell, and discover installed plugins.

**CLI surface:** `twig self update` (alias `upgrade`), `twig self diagnose` (alias `diag`), `twig self completion`,
`twig self plugins` **Crates:** `twig-cli` (self_cmd module)

## Self Update

### Requirement: Updating twig to the latest release

#### Scenario: Newer version available

WHEN the user runs `twig self update` AND the current version differs from the latest GitHub release tag THEN the
command prints "Checking for updates (current version X.Y.Z)…" AND fetches the latest release metadata from
`https://api.github.com/repos/eddieland/twig/releases/latest` AND downloads the platform-appropriate archive AND
extracts the binary AND replaces the currently running twig executable AND cleans up temporary staging files AND prints
"Twig has been updated to version X.Y.Z."

#### Scenario: Already on the latest version

WHEN the user runs `twig self update` without `--force` AND the current version matches the latest GitHub release tag
THEN the command prints "You're already running the latest version of Twig." AND exits successfully without downloading

#### Scenario: Force reinstall

WHEN the user runs `twig self update --force` THEN the version comparison check is skipped AND the update workflow
proceeds unconditionally

### Requirement: Platform detection for asset selection

#### Scenario: Linux platform

WHEN `twig self update` selects a release asset on Linux THEN the OS markers are `["linux"]` AND the architecture
markers are `["x86_64", "amd64"]` for x86_64 or `["aarch64", "arm64"]` for aarch64 AND the archive extension is
`.tar.gz` AND the binary name is `twig`

#### Scenario: macOS platform

WHEN `twig self update` selects a release asset on macOS THEN the OS markers are `["macos", "darwin"]` AND the
architecture markers match the current architecture AND universal builds are accepted for any architecture AND the
archive extension is `.tar.gz` AND the binary name is `twig`

#### Scenario: Windows platform

WHEN `twig self update` selects a release asset on Windows THEN the OS markers are `["windows"]` AND the archive
extension is `.zip` AND the binary name is `twig.exe`

#### Scenario: Unsupported platform

WHEN the current operating system is not Linux, macOS, or Windows THEN the command returns an error "Unsupported
operating system: <os>"

### Requirement: Asset matching rules

#### Scenario: Matching release assets to the current platform

WHEN matching a release asset THEN the command performs case-insensitive matching on the archive extension, product name,
OS marker (substring match), and architecture marker (substring match) AND the product name is derived from the asset
filename by stripping the `.exe` suffix on Windows

### Requirement: Archive extraction

#### Scenario: Extracting from tar.gz on Unix

WHEN the downloaded archive is `.tar.gz` THEN the command decompresses and iterates through tar entries searching for
the binary by filename AND unpacks the binary to the staging directory AND sets executable permissions (mode `0o755`) AND
if the binary is not found in the archive, returns an error "Binary <name> not found in archive"

#### Scenario: Extracting from zip on Windows

WHEN the downloaded archive is `.zip` THEN the command opens the zip archive and iterates through entries searching for
the binary by filename AND extracts the binary to the staging directory AND if the binary is not found, returns an error
"Binary <name> not found in archive"

### Requirement: Binary installation on Unix

#### Scenario: Direct copy and atomic rename

WHEN installing a new binary on Unix THEN the command copies the new binary to a UUID-named staging file in the target
directory AND sets permissions to `0o755` AND attempts an atomic rename to the current executable path

#### Scenario: Permission denied during installation

WHEN the atomic rename fails with a permission error OR the initial copy fails with permission denied THEN the command
falls back to `sudo install -m 755 <new_binary> <current_exe>`

### Requirement: Binary installation on Windows

#### Scenario: Deferred installation via PowerShell helper

WHEN installing a new binary on Windows THEN the command tests write permissions in the target directory AND generates a
PowerShell helper script that waits for the parent twig process to exit, moves the staged binary to the target location,
and cleans up AND if the directory requires elevated permissions, launches the helper via `Start-Process PowerShell -Verb
RunAs` AND returns an `InstallOutcome::Deferred` indicating the installation will complete after twig exits

### Requirement: Version tag handling

#### Scenario: Cleaning version tags

WHEN comparing versions THEN GitHub release tag names have leading `v` characters stripped (e.g., `v0.5.2` becomes
`0.5.2`) AND the internal version from `CARGO_PKG_VERSION` does not include a `v` prefix

## Plugin Updates

### Requirement: Updating plugins via self update

#### Scenario: Installing or updating a plugin

WHEN the user runs `twig self update flow`, `twig self update prune`, or `twig self update mcp` THEN the command fetches
the latest release from GitHub AND searches for an asset matching the plugin binary name for the current platform AND
downloads, extracts, and installs the plugin binary to the same directory as the running twig executable AND prints
"Twig <plugin-name> <version> is installed at <path>."

#### Scenario: Plugin already up to date

WHEN the user runs `twig self update <plugin>` without `--force` AND the installed plugin version matches the latest
release THEN the command prints "<Plugin-Name> plugin is already up to date." AND exits without downloading

#### Scenario: Plugin not available for platform

WHEN the GitHub release does not have an asset matching the plugin binary name for the current platform THEN the command
returns an error "No <plugin-name> plugin asset available for this platform"

### Requirement: Plugin version detection

#### Scenario: Extracting version from plugin binary

WHEN checking if an installed plugin is up to date THEN the command executes the plugin with `--version` AND parses
stdout to extract a version number by finding the first whitespace-separated token starting with an ASCII digit AND
trims non-version characters from the ends (e.g., `twig-flow 0.2.3\n` extracts `0.2.3`)

#### Scenario: Plugin not installed or version unreadable

WHEN the plugin does not exist on PATH or `--version` execution fails THEN the version is considered missing AND a
reinstall is triggered

### Requirement: Plugin install location

#### Scenario: Placing plugin in twig's directory

WHEN installing a plugin THEN the binary is placed in the same directory as the running twig executable AND if that
directory is not on PATH, a warning is printed: "The plugin was installed to <path> which is not on your PATH."

## Diagnostics

### Requirement: System diagnostics report

#### Scenario: Running diagnostics

WHEN the user runs `twig self diagnose` (alias `twig self diag`) THEN the command prints a comprehensive diagnostic
report covering system information, configuration directories, credentials, git configuration, tracked repositories, and
dependencies AND if any section encounters an error, it continues with remaining sections AND prints "Diagnostics
complete!" upon finishing

### Requirement: System information section

#### Scenario: Displaying system details

WHEN diagnostics runs the system information section THEN it prints the operating system name and architecture, the home
directory path (or "Not found"), the current working directory, and the shell name from `$SHELL` (or "Not detected")

### Requirement: Configuration directories section

#### Scenario: Displaying directory status

WHEN diagnostics runs the configuration directories section THEN for each of config, data, and cache directories it
prints the path AND appends "(not created yet)" if the directory does not exist AND for the data directory, checks
whether `registry.json` exists and prints "Registry: Found" or "Registry: Not found" AND if the cache directory is not
configured, prints "Cache: Not configured"

### Requirement: Credentials section

#### Scenario: Checking netrc file

WHEN diagnostics runs the credentials section THEN it checks whether `~/.netrc` exists AND on Unix, checks file
permissions and prints "Secure (600)" if mode is exactly `0o600` or a warning with the actual permissions if not AND on
non-Unix platforms, prints "Unable to check on this platform"

#### Scenario: Checking Jira credentials

WHEN diagnostics checks Jira credentials THEN it reads the Jira host configuration AND verifies that a netrc entry
exists for the host AND prints "Jira credentials: Found", "Jira credentials: Not found", or "Jira credentials: Error -
<error>"

#### Scenario: Checking GitHub credentials

WHEN diagnostics checks GitHub credentials THEN it verifies that a netrc entry exists for GitHub AND prints "GitHub
credentials: Found", "GitHub credentials: Not found", or "GitHub credentials: Error - <error>"

### Requirement: Git configuration section

#### Scenario: Checking git installation and config

WHEN diagnostics runs the git configuration section THEN it checks git version via `git --version` AND checks global
`user.name` and `user.email` settings AND prints each value or "Not configured" / "Not found or not executable" as
appropriate

### Requirement: Tracked repositories section

#### Scenario: Displaying registered repositories

WHEN diagnostics runs the tracked repositories section THEN it attempts to load the registry AND displays the list of
registered repositories AND prints "No repositories tracked" if the registry is empty or not found AND prints the error
if the registry cannot be read

### Requirement: Dependencies section

#### Scenario: Checking SSH and network connectivity

WHEN diagnostics runs the dependencies section THEN it checks SSH version via `ssh -V` AND tests network connectivity to
GitHub (`ping -c 1 -W 3 github.com`) and Atlassian (`ping -c 1 -W 3 atlassian.net`) AND prints "Reachable",
"Unreachable", or "Unable to test (ping not available)" for each host

## Shell Completions

### Requirement: Generating shell completion scripts

#### Scenario: Generating completions for a supported shell

WHEN the user runs `twig self completion <shell>` where `<shell>` is one of `bash`, `zsh`, `fish`, or `powershell` THEN
the command generates a shell-specific completion script using clap_complete AND writes the script to stdout AND the
script provides tab completion for all twig subcommands, flags, and options

#### Scenario: Invalid shell argument

WHEN the user provides an unsupported shell name THEN clap validation rejects the input with an error message listing
the valid options

## Command Aliases

### Requirement: Subcommand aliases

#### Scenario: Using aliases

WHEN the user runs `twig self upgrade` THEN it behaves identically to `twig self update` AND when the user runs
`twig self diag` THEN it behaves identically to `twig self diagnose` AND when the user runs `twig self list-plugins`
THEN it behaves identically to `twig self plugins`
