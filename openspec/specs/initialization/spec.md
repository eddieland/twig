# Initialization

## Purpose

Set up twig's local configuration files and directories for first-time use. Creates XDG-compliant config/data/cache
directories, initializes an empty global registry, and ensures the per-repo `.twig/` directory is git-ignored.

**CLI surface:** `twig init` **Crates:** `twig-core` (config::ConfigDirs, state), `twig-cli` (config command module)

## CLI Command

### Requirement: First-time initialization

#### Scenario: Creating directories and registry on first run

WHEN the user runs `twig init` for the first time AND no twig directories exist THEN twig creates the XDG-compliant
config, data, and cache directories (including parent directories) AND creates an empty registry file at
`<data_dir>/registry.json` with an empty JSON array AND prints a success message showing the config and data directory
paths

#### Scenario: Init accepts no flags or arguments

WHEN `twig init` is invoked THEN it accepts no flags, options, or positional arguments

### Requirement: Idempotent re-initialization

#### Scenario: Running init when directories already exist

WHEN the user runs `twig init` again AND the directories already exist THEN the directories are not destroyed or
recreated AND the success message is printed again

#### Scenario: Running init when registry already has data

WHEN the user runs `twig init` AND `<data_dir>/registry.json` already exists with repository entries THEN the existing
registry file is not overwritten AND existing data is preserved

### Requirement: Init error handling

#### Scenario: Directory or registry creation fails

WHEN `twig init` is called AND creating any directory (config, data, or cache) or the registry file fails THEN an error
is returned identifying which resource could not be created

## Core Config (`twig-core`)

### Requirement: XDG-compliant directory resolution

#### Scenario: Resolving project directories

WHEN `ConfigDirs::new()` is called THEN it uses `directories::ProjectDirs::from("eddieland", "", "twig")` to resolve
platform-appropriate directories AND on Linux the directories default to `~/.config/twig`, `~/.local/share/twig`, and
`~/.cache/twig`

#### Scenario: Respecting XDG environment variable overrides

WHEN `XDG_CONFIG_HOME` or `XDG_DATA_HOME` is set to a custom path THEN the corresponding directory uses that path
instead of the default

#### Scenario: ProjectDirs resolution fails

WHEN `ProjectDirs::from()` returns `None` THEN `ConfigDirs::new()` returns an error indicating project directories could
not be determined

#### Scenario: Cache directory is optional

WHEN `ConfigDirs` is constructed THEN `cache_dir` is stored as `Option<PathBuf>` AND if `None`, init skips cache
directory creation without error

### Requirement: Registry file initialization

#### Scenario: Creating an empty registry

WHEN `ConfigDirs::init()` runs AND no `registry.json` exists THEN it writes an empty JSON array to
`<data_dir>/registry.json`

#### Scenario: Preserving existing registry

WHEN `ConfigDirs::init()` runs AND `registry.json` already exists THEN the file is not modified

#### Scenario: Loading a non-existent registry

WHEN `Registry::load()` is called AND the registry file does not exist THEN it returns an empty `Registry` with zero
repositories (does not error)

### Requirement: Directory path accessors

#### Scenario: Standard path conventions

WHEN path accessors are called THEN `registry_path()` returns `<data_dir>/registry.json` AND `repo_state_dir(repo_path)`
returns `<repo_path>/.twig` AND `repo_state_path(repo_path)` returns `<repo_path>/.twig/state.json` AND
`jira_config_path()` returns `<config_dir>/jira.toml`

### Requirement: Per-repo .twig directory and gitignore

#### Scenario: Auto-creation on state save or git add

WHEN `RepoState::save()` is called or a repository is added via `twig git add` THEN `ensure_twig_internal_gitignore()`
is called for the repository path

#### Scenario: Gitignore creation and idempotency

WHEN `ensure_twig_internal_gitignore()` is called THEN it creates `.twig/` if missing AND writes a catch-all `*` rule to
`.twig/.gitignore` AND if the rule already exists, the file is not modified (no duplicates) AND the repository's root
`.gitignore` is never modified
