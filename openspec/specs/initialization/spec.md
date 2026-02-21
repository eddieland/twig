# Initialization

## Purpose

Set up twig's local configuration files and directories for first-time use. Creates XDG-compliant config/data/cache
directories, initializes an empty global registry, and ensures the per-repo `.twig/` directory is git-ignored.

**CLI surface:** `twig init` **Crates:** `twig-core` (config::ConfigDirs, state), `twig-cli` (config command module)

## Requirements

### Requirement: XDG-compliant directory resolution

#### Scenario: Resolving project directories using the directories crate

WHEN `ConfigDirs::new()` is called THEN it uses `directories::ProjectDirs::from("eddieland", "", "twig")` to resolve
platform-appropriate directories AND on Linux the config directory resolves to `$XDG_CONFIG_HOME/twig` (default
`~/.config/twig`) AND on Linux the data directory resolves to `$XDG_DATA_HOME/twig` (default `~/.local/share/twig`) AND
on Linux the cache directory resolves to `$XDG_CACHE_HOME/twig` (default `~/.cache/twig`)

#### Scenario: Respecting XDG environment variable overrides

WHEN `XDG_CONFIG_HOME` is set to a custom path THEN the config directory uses that custom path instead of `~/.config`
AND when `XDG_DATA_HOME` is set to a custom path THEN the data directory uses that custom path instead of
`~/.local/share`

#### Scenario: ProjectDirs resolution fails

WHEN `ProjectDirs::from()` returns `None` (e.g., no valid home directory) THEN `ConfigDirs::new()` returns an error with
the message "Failed to determine project directories"

### Requirement: First-time initialization

#### Scenario: Creating directories and registry on first run

WHEN the user runs `twig init` for the first time AND no twig directories exist THEN twig creates the XDG-compliant
config directory (and all parent directories) AND creates the XDG-compliant data directory (and all parent directories)
AND creates the XDG-compliant cache directory (and all parent directories) AND creates an empty registry file at
`<data_dir>/registry.json` with contents `[]` AND prints a success message "Initialized twig configuration directories:"
AND prints the config directory path AND prints the data directory path

#### Scenario: Cache directory is optional

WHEN `ConfigDirs` is constructed THEN `cache_dir` is stored as `Option<PathBuf>` AND `cache_dir()` returns
`Option<&PathBuf>` AND if `cache_dir` is `Some`, the `init()` method creates it AND if `cache_dir` is `None`, the
`init()` method skips cache directory creation without error

### Requirement: Idempotent re-initialization

#### Scenario: Running init when directories already exist

WHEN the user runs `twig init` a second time AND the config, data, and cache directories already exist THEN the
directories are not destroyed or recreated (fs::create_dir_all is a no-op for existing directories) AND the success
message is printed again

#### Scenario: Running init when registry already has data

WHEN the user runs `twig init` AND `<data_dir>/registry.json` already exists with repository entries THEN the existing
registry file is not overwritten AND the existing repository data is preserved

### Requirement: Registry file initialization

#### Scenario: Creating an empty registry

WHEN `ConfigDirs::init()` runs AND no `registry.json` file exists in the data directory THEN it writes `[]` (an empty
JSON array) to `<data_dir>/registry.json`

#### Scenario: Preserving existing registry

WHEN `ConfigDirs::init()` runs AND `registry.json` already exists in the data directory THEN the file is not modified
(existence check with `!registry_path.exists()`)

#### Scenario: Loading a non-existent registry

WHEN `Registry::load()` is called AND the registry file does not exist at the expected path THEN it returns an empty
`Registry` with zero repositories (does not error)

### Requirement: Directory path accessors

#### Scenario: Registry path

WHEN `ConfigDirs::registry_path()` is called THEN it returns `<data_dir>/registry.json`

#### Scenario: Repo state directory

WHEN `ConfigDirs::repo_state_dir(repo_path)` is called THEN it returns `<repo_path>/.twig`

#### Scenario: Repo state file path

WHEN `ConfigDirs::repo_state_path(repo_path)` is called THEN it returns `<repo_path>/.twig/state.json`

#### Scenario: Jira config path

WHEN `ConfigDirs::jira_config_path()` is called THEN it returns `<config_dir>/jira.toml`

### Requirement: Per-repo .twig directory setup

#### Scenario: Creating the .twig directory and gitignore on first state save

WHEN `RepoState::save()` is called for a repository AND the `.twig/` directory does not exist THEN it creates the
`.twig/` directory (and any parent directories) AND calls `ensure_twig_internal_gitignore()` to create the gitignore

#### Scenario: Creating the .twig directory on git add

WHEN a repository is added to the registry via `twig git add` THEN `ensure_twig_internal_gitignore()` is called for the
repository path

### Requirement: Internal gitignore for .twig directory

#### Scenario: Creating .twig/.gitignore on first encounter

WHEN `ensure_twig_internal_gitignore()` is called AND the `.twig/` directory does not exist THEN it creates the `.twig/`
directory AND writes `*\n` to `.twig/.gitignore`

#### Scenario: Updating .twig/.gitignore when catch-all rule is missing

WHEN `ensure_twig_internal_gitignore()` is called AND `.twig/.gitignore` exists but does not contain a line with just
`*` THEN it overwrites the file with `*\n`

#### Scenario: Skipping update when .twig/.gitignore already has catch-all rule

WHEN `ensure_twig_internal_gitignore()` is called AND `.twig/.gitignore` exists and contains a line that trims to `*`
THEN it returns without modifying the file

#### Scenario: Idempotent gitignore creation

WHEN `ensure_twig_internal_gitignore()` is called multiple times THEN the `.twig/.gitignore` file contains exactly one
`*` entry (no duplicates)

#### Scenario: Root .gitignore is never modified

WHEN `ensure_twig_internal_gitignore()` is called for a repository AND the repository has an existing root `.gitignore`
file THEN the root `.gitignore` file is not modified AND only `.twig/.gitignore` is created or updated

### Requirement: Init command has no flags or arguments

#### Scenario: Command definition

WHEN `twig init` is invoked THEN it accepts no flags, options, or positional arguments AND it is defined as a unit
variant `Init` in the `Commands` enum (no associated struct)

### Requirement: Init error handling

#### Scenario: Config directory creation fails

WHEN `twig init` is called AND creating the config directory fails (e.g., permission denied) THEN an error is returned
with the message "Failed to create config directory"

#### Scenario: Data directory creation fails

WHEN `twig init` is called AND creating the data directory fails THEN an error is returned with the message "Failed to
create data directory"

#### Scenario: Cache directory creation fails

WHEN `twig init` is called AND creating the cache directory fails THEN an error is returned with the message "Failed to
create cache directory"

#### Scenario: Registry file creation fails

WHEN `twig init` is called AND writing the empty registry file fails THEN an error is returned with the message "Failed
to create empty registry file"
