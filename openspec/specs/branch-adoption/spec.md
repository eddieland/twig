# Branch Adoption

## Purpose

Automatically detect orphaned branches (those without parent dependencies in the twig graph) and re-parent them to a
chosen parent branch. Supports auto, default-root, and explicit branch adoption modes with interactive preview and
confirmation.

**CLI surface:** `twig adopt`, flags: `--mode`, `--parent`, `-y/--yes`, `-d/--max-depth`, `--no-color` **Crates:**
`twig-core` (tree algorithms, state), `twig-cli` (adopt command module)

## Requirements

### Requirement: Repository resolution

#### Scenario: Auto-detecting the repository from the working directory

WHEN the user runs `twig adopt` without `--repo` THEN the repository is detected by traversing from the current working
directory upward using `detect_repository` AND if no repository is found, the command fails with a "Not in a git
repository" error

#### Scenario: Overriding the repository path with `--repo`

WHEN the user runs `twig adopt --repo <path>` THEN the command operates on the repository located at `<path>` instead of
auto-detecting from the working directory AND if the repository cannot be opened, the command fails with a "Failed to
open git repository at <path>" error

### Requirement: Orphan detection

#### Scenario: Identifying orphaned branches

WHEN the user runs `twig adopt` THEN the command resolves user-defined dependencies (excluding the default root
attachment) via `resolve_user_dependencies_without_default_root` AND builds the tree structure via
`build_tree_from_user_dependencies` AND any branch that has no parent dependencies AND is not marked as a root branch is
classified as orphaned

#### Scenario: No orphaned branches exist

WHEN the user runs `twig adopt` AND no orphaned branches are found THEN the command prints "No orphaned branches found.
Nothing to adopt." AND exits successfully without prompting or modifying state

### Requirement: Mode selection

#### Scenario: Default mode is auto

WHEN the user runs `twig adopt` without `--mode` or `--parent` THEN the adoption mode defaults to `auto`

#### Scenario: Explicit mode selection

WHEN the user runs `twig adopt --mode <mode>` THEN the command uses the specified mode (`auto`, `default-root`, or
`branch`) for building the adoption plan

#### Scenario: Providing `--parent` implies branch mode

WHEN the user runs `twig adopt --parent <branch>` without specifying `--mode` THEN the mode is automatically set to
`branch` AND the specified branch is used as the target parent

### Requirement: Auto mode adoption

#### Scenario: Generating adoption plan from Git ancestry

WHEN the mode is `auto` THEN `AutoDependencyDiscovery::suggest_dependencies` analyzes Git commit ancestry to generate
`DependencySuggestion` entries with confidence scores (0.0 to 1.0) AND for each orphaned branch, the suggestion with the
highest confidence is selected as the adoption target

#### Scenario: Auto mode falls back to suggested root for unmatched branches

WHEN the mode is `auto` AND one or more orphaned branches have no auto-generated suggestions THEN the command falls back
to the default root (or a heuristically suggested root via `get_or_suggest_default_root`) for those branches AND if a
fallback root is available, the plan entry uses the reason "Fallback to suggested root"

#### Scenario: Auto mode warns when some branches cannot be matched

WHEN the mode is `auto` AND the number of plan entries is fewer than the number of orphaned branches THEN the command
prints a warning: "Some orphaned branches could not be matched automatically."

### Requirement: Default-root mode adoption

#### Scenario: Attaching all orphans to the default root

WHEN the mode is `default-root` AND a default root branch is configured THEN all orphaned branches are added to the
adoption plan with the configured default root as their parent AND each plan entry uses the reason "Attach to default
root"

#### Scenario: No default root configured

WHEN the mode is `default-root` AND no default root branch is configured THEN the command fails with the error: "No
default root is configured. Set one with 'twig branch root add <branch> --default'."

### Requirement: Branch mode adoption

#### Scenario: Attaching all orphans to a specific parent

WHEN the mode is `branch` AND `--parent <branch>` is provided AND the parent branch exists locally THEN all orphaned
branches are added to the adoption plan with the specified branch as their parent AND each plan entry uses the reason
"Attach to specified parent"

#### Scenario: Branch mode without `--parent`

WHEN the mode is `branch` AND `--parent` is not provided THEN the command fails with the error: "--parent must be
provided when using --mode branch"

#### Scenario: Specified parent branch does not exist locally

WHEN the mode is `branch` AND the `--parent` branch does not exist in the resolved branch nodes THEN the command fails
with the error: "Parent branch '<branch>' does not exist locally."

### Requirement: Empty adoption plan

#### Scenario: No suggestions generated for any orphaned branch

WHEN the adoption plan is empty after mode processing (e.g., auto mode produced no suggestions and no fallback root is
available) THEN the command prints a warning: "No adoption suggestions could be generated for the orphaned branches."
AND exits successfully without prompting or modifying state

### Requirement: Plan display

#### Scenario: Displaying the adoption plan before preview

WHEN adoption plan entries exist THEN the command prints "Adoption plan (proposed dependencies):" followed by each entry
formatted as " <child> -> <parent> (<reason>)" AND this display occurs before the preview tree is rendered

### Requirement: Preview tree

#### Scenario: Rendering the proposed tree structure

WHEN adoption plan entries exist THEN a clone of the repository state is created AND the plan is applied to the cloned
state AND the resulting tree is rendered with the header "Proposed tree (no changes made yet):" AND the tree respects
`--max-depth` (limiting display depth) and `--no-color` (disabling colored output)

#### Scenario: No root branches in preview state

WHEN the adoption plan is applied to the preview state AND no root branches are found THEN the command prints a warning:
"No root branches available after adoption plan." AND continues to the confirmation step

#### Scenario: Remaining orphans after adoption plan

WHEN the preview tree is rendered AND some branches remain orphaned after the plan is applied THEN the command prints
"Remaining orphaned branches after adoption plan:" followed by each remaining orphan listed with a bullet point

### Requirement: Confirmation prompt

#### Scenario: Prompting for confirmation

WHEN the adoption plan and preview are displayed AND `--yes` is not specified THEN the command prompts the user with
"Apply this adoption plan? \[y/N\]: " AND waits for input from stdin

#### Scenario: User confirms the adoption plan

WHEN the user responds with "y" or "yes" (case-insensitive) THEN the command proceeds to apply the plan to the real
repository state

#### Scenario: User declines the adoption plan

WHEN the user responds with any value other than "y" or "yes" (including empty input) THEN the command prints "Aborted
without making changes." AND exits successfully without modifying state

#### Scenario: Skipping confirmation with `--yes`

WHEN the user runs `twig adopt --yes` THEN the confirmation prompt is skipped AND the adoption plan is applied
immediately after the preview

### Requirement: Plan application and persistence

#### Scenario: Applying the adoption plan to repository state

WHEN the user confirms the adoption plan (or `--yes` is specified) THEN for each plan entry,
`repo_state.add_dependency(child, parent)` is called AND the updated state is saved to disk via `repo_state.save()` AND
the command prints "Adoption complete. Branch relationships updated."

#### Scenario: Duplicate dependency detected during application

WHEN `add_dependency` is called for a child-parent pair that already exists in the dependency graph THEN the operation
fails with the error: "Dependency from '<child>' to '<parent>' already exists"

#### Scenario: Circular dependency detected during application

WHEN `add_dependency` is called AND adding the dependency would create a cycle in the dependency graph THEN the
operation fails with the error: "Adding dependency from '<child>' to '<parent>' would create a circular dependency"
