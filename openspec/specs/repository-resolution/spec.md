# Repository Resolution

## Purpose

Detect and open the git repository that a command operates on. Most twig commands need a repository context and share
the same two-step resolution: auto-detect from the working directory, or accept an explicit path override via a CLI
flag. This spec defines the canonical behavior; individual command specs reference it rather than re-describing the
mechanics.

**Crates:** `twig-core` (git/detection, git/repository), `twig-cli` (per-command handler modules)

## Requirements

### Requirement: Auto-detecting the repository from the working directory

#### Scenario: Repository found by traversal

WHEN a command that requires a repository context is run without a repository path flag THEN the repository is detected
by calling `detect_repository()`, which delegates to `git2::Repository::discover` and walks from the current working
directory upward until a `.git` directory is found AND the resolved path is used for all subsequent operations

#### Scenario: No repository found

WHEN `detect_repository()` finds no `.git` directory at or above the current working directory THEN the command fails
with an error indicating the user is not in a git repository

### Requirement: Overriding the repository path with a CLI flag

Individual commands expose this override under different flag names (`-r`, `--repo`, or both). The flag name is
documented in each command's own spec.

#### Scenario: Valid path provided

WHEN the user passes a repository path flag THEN the command uses that path instead of auto-detecting from the working
directory

#### Scenario: Path cannot be opened as a git repository

WHEN the provided path does not contain a valid git repository (e.g., the path does not exist or is not a git root) THEN
the command fails with an error indicating the repository could not be opened at the given path

### Requirement: Opening the repository

#### Scenario: Repository opens successfully

WHEN the repository path is resolved (by auto-detection or explicit flag) AND `git2::Repository::open` succeeds THEN the
command proceeds with the opened repository

#### Scenario: Repository open fails

WHEN `git2::Repository::open` fails on the resolved path THEN the command fails with an error indicating the repository
could not be opened at that path
