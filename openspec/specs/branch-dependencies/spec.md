# Branch Dependencies

## Purpose

Manage custom parent/child relationships between branches and designate root branches that anchor dependency trees. This
is the foundational data model for twig's stacked workflow — all rebase, cascade, tree, and adoption features depend on
these relationships.

**CLI surface:** `twig branch depend`, `twig branch remove-dep`, `twig branch parent`,
`twig branch root add/list/remove` **Crates:** `twig-core` (state, graph), `twig-cli` (branch command module)

## Requirements

### Requirement: Adding a branch dependency

#### Scenario: Successfully adding a dependency between two branches

WHEN the user runs `twig branch depend <child> <parent>` THEN a dependency relationship is created with `<parent>` as
the parent of `<child>` AND the dependency is assigned a unique UUID and a `created_at` timestamp AND the dependency is
persisted to `.twig/state.json` AND the in-memory `dependency_children_index` and `dependency_parents_index` are rebuilt
AND the CLI prints a success message: `Added dependency: <child> -> <parent>`

#### Scenario: Adding a duplicate dependency

WHEN the user runs `twig branch depend <child> <parent>` AND a dependency from `<child>` to `<parent>` already exists
THEN the command fails with the error: `Dependency from '<child>' to '<parent>' already exists` AND no new dependency is
created AND the state file is not modified

#### Scenario: Adding a dependency that would create a direct cycle

WHEN the user runs `twig branch depend A B` AND a dependency from `B` to `A` already exists (B is a child of A) THEN the
command fails with the error: `Adding dependency from 'A' to 'B' would create a circular dependency` AND no new
dependency is created

#### Scenario: Adding a dependency that would create a transitive cycle

WHEN the user runs `twig branch depend A C` AND dependencies exist forming a chain `C -> B -> A` (C depends on B, B
depends on A) THEN the command fails with the error:
`Adding dependency from 'A' to 'C' would create a circular dependency` AND no new dependency is created

#### Scenario: Self-loop is rejected via cycle detection

WHEN the user runs `twig branch depend X X` THEN the command fails with the error:
`Adding dependency from 'X' to 'X' would create a circular dependency` AND no new dependency is created

#### Scenario: Using the dot alias for the current branch

WHEN the user runs `twig branch depend . <parent>` from a branch named `feature` THEN `.` is resolved to `feature` AND
the dependency is created with `feature` as the child

WHEN the user runs `twig branch depend <child> .` from a branch named `main` THEN `.` is resolved to `main` AND the
dependency is created with `main` as the parent

#### Scenario: Specifying a repository path

WHEN the user runs `twig branch depend <child> <parent> -r <path>` THEN the dependency is created in the repository at
`<path>` instead of the auto-detected repository

#### Scenario: No Git branch existence validation

WHEN the user runs `twig branch depend <child> <parent>` THEN the dependency is created regardless of whether `<child>`
or `<parent>` exist as Git branches AND no Git repository lookup is performed to validate branch names

### Requirement: Removing a branch dependency

#### Scenario: Successfully removing an existing dependency

WHEN the user runs `twig branch remove-dep <child> <parent>` (or `twig branch rm-dep <child> <parent>`) AND a dependency
from `<child>` to `<parent>` exists THEN the dependency is removed from the dependencies list AND the in-memory indices
are rebuilt AND the state file is updated AND the CLI prints: `Removed dependency: <child> -> <parent>`

#### Scenario: Removing a non-existent dependency

WHEN the user runs `twig branch remove-dep <child> <parent>` AND no dependency from `<child>` to `<parent>` exists THEN
the CLI prints a warning: `Dependency <child> -> <parent> not found` AND the command returns successfully (no error) AND
the state file is not modified

#### Scenario: Using the dot alias for the current branch

WHEN the user runs `twig branch remove-dep . <parent>` from a branch named `feature` THEN `.` is resolved to `feature`
before looking up the dependency

#### Scenario: Specifying a repository path

WHEN the user runs `twig branch remove-dep <child> <parent> -r <path>` THEN the removal operates on the repository at
`<path>`

### Requirement: Querying parent branches

#### Scenario: Branch has a single twig-defined parent

WHEN the user runs `twig branch parent [branch]` AND the branch has exactly one parent dependency defined THEN the CLI
prints: `Parent branch of '<branch>': <parent>`

#### Scenario: Branch has multiple twig-defined parents

WHEN the user runs `twig branch parent [branch]` AND the branch has more than one parent dependency defined THEN the CLI
prints: `Parent branches of '<branch>':` AND lists each parent on its own line, indented with two spaces

#### Scenario: Branch has no twig parent but has a Git upstream

WHEN the user runs `twig branch parent [branch]` AND the branch has no twig-defined parent dependencies AND the branch
has a Git upstream tracking branch configured THEN the CLI prints:
`No twig parent defined for '<branch>', but Git upstream is: <upstream>`

#### Scenario: Branch has no twig parent and no Git upstream

WHEN the user runs `twig branch parent [branch]` AND the branch has no twig-defined parent dependencies AND the branch
has no Git upstream tracking branch THEN the CLI prints: `No parent branches defined for '<branch>'`

#### Scenario: Defaulting to the current branch

WHEN the user runs `twig branch parent` without specifying a branch name THEN the current HEAD branch is used as the
target branch

#### Scenario: Using the dot alias

WHEN the user runs `twig branch parent .` THEN `.` is resolved to the current branch name

#### Scenario: Specifying a repository path

WHEN the user runs `twig branch parent [branch] -r <path>` THEN the query operates on the repository at `<path>`

### Requirement: Adding a root branch

#### Scenario: Adding a new root branch without default flag

WHEN the user runs `twig branch root add <branch>` THEN the branch is added to the root branches list with `is_default`
set to `false` AND it is assigned a unique UUID and a `created_at` timestamp AND the state file is updated AND the CLI
prints: `Added <branch> as root branch`

#### Scenario: Adding a new root branch as default

WHEN the user runs `twig branch root add <branch> --default` THEN the branch is added to the root branches list with
`is_default` set to `true` AND all previously default root branches have their `is_default` flag cleared AND the state
file is updated AND the CLI prints: `Added <branch> as default root branch`

#### Scenario: Re-adding an existing root branch without default flag

WHEN the user runs `twig branch root add <branch>` AND `<branch>` is already a root branch THEN no new entry is created
(idempotent) AND the existing root's `is_default` flag is not changed AND the state file is updated

#### Scenario: Promoting an existing root branch to default

WHEN the user runs `twig branch root add <branch> --default` AND `<branch>` is already a root branch but not the default
THEN `<branch>` becomes the default root AND all other roots have their `is_default` flag cleared AND the state file is
updated

#### Scenario: Specifying a repository path

WHEN the user runs `twig branch root add <branch> -r <path>` THEN the root is added in the repository at `<path>`

### Requirement: Listing root branches

#### Scenario: Listing when root branches exist

WHEN the user runs `twig branch root list` (or `twig branch root ls`) AND one or more root branches are defined THEN the
CLI prints `Root branches:` followed by each root branch name on its own line, indented with two spaces AND the default
root branch (if any) has ` (default)` appended to its name

#### Scenario: Listing when no root branches are defined

WHEN the user runs `twig branch root list` AND no root branches are defined THEN the CLI prints:
`No root branches defined`

#### Scenario: Specifying a repository path

WHEN the user runs `twig branch root list -r <path>` THEN the listing operates on the repository at `<path>`

### Requirement: Removing a root branch

#### Scenario: Successfully removing an existing root branch

WHEN the user runs `twig branch root remove <branch>` (or `twig branch root rm <branch>`) AND `<branch>` is currently a
root branch THEN the root branch entry is removed from the list AND if it was the default root, no other root is
promoted to default automatically AND the state file is updated AND the CLI prints:
`Removed <branch> from root branches`

#### Scenario: Removing a non-existent root branch

WHEN the user runs `twig branch root remove <branch>` AND `<branch>` is not in the root branches list THEN the CLI
prints a warning: `Root branch <branch> not found` AND the command returns successfully (no error) AND the state file is
not modified

#### Scenario: Specifying a repository path

WHEN the user runs `twig branch root remove <branch> -r <path>` THEN the removal operates on the repository at `<path>`

### Requirement: State persistence and index consistency

#### Scenario: Dependencies and roots survive across sessions

WHEN dependencies and root branches are added to `.twig/state.json` AND a new command loads the state from disk THEN all
previously saved dependencies and root branches are present AND the `dependency_children_index` and
`dependency_parents_index` are rebuilt on load

#### Scenario: Indices are not serialized

WHEN the state is saved to `.twig/state.json` THEN the `dependency_children_index`, `dependency_parents_index`,
`branch_to_jira_index`, `jira_to_branch_index`, and `pr_to_branch_index` fields are excluded from the JSON (marked
`#[serde(skip)]`) AND they are rebuilt from the canonical data whenever the state is loaded

#### Scenario: Index rebuild produces correct mappings

WHEN indices are rebuilt from the dependencies list THEN `dependency_children_index` maps each parent branch name to a
list of its child branch names AND `dependency_parents_index` maps each child branch name to a list of its parent branch
names

#### Scenario: .twig directory is created automatically

WHEN the state is saved and the `.twig/` directory does not yet exist THEN the directory is created AND a
`.twig/.gitignore` file is written containing `*` to exclude all twig metadata from version control

#### Scenario: .twig/.gitignore is idempotent

WHEN the `.twig/.gitignore` already exists and contains the `*` rule THEN saving state again does not duplicate the rule

### Requirement: Branch dependency data model

#### Scenario: BranchDependency fields

WHEN a dependency is created THEN it contains: `id` (UUID v4), `child` (String), `parent` (String), `created_at`
(DateTime<Utc>)

#### Scenario: RootBranch fields

WHEN a root branch is created THEN it contains: `id` (UUID v4), `branch` (String), `is_default` (bool), `created_at`
(DateTime<Utc>)

#### Scenario: Only one default root at a time

WHEN a root branch is set as default (via `--default` flag) THEN all other root branches have `is_default` set to
`false` AND exactly zero or one root branch has `is_default` set to `true` at any given time

### Requirement: Graph integration of dependencies

#### Scenario: Dependencies create edges in the branch graph

WHEN the `BranchGraphBuilder` builds a graph with `include_declared_dependencies` enabled (the default) AND dependencies
exist in the repo state AND both the child and parent branches exist as Git branches THEN an edge is created from the
parent node to the child node AND the child's `primary_parent` is set to the first declared parent AND any additional
parents are stored as `secondary_parents` AND the parent's `children` list includes the child

#### Scenario: Dependencies referencing non-existent Git branches are silently skipped

WHEN a dependency references a child or parent branch that does not exist in the Git repository THEN no edge is created
for that dependency AND no error is raised

#### Scenario: Graph builder can disable dependency integration

WHEN the `BranchGraphBuilder` is configured with `with_declared_dependencies(false)` THEN no dependency edges are
created regardless of what is in the repo state

#### Scenario: Root branches become graph root candidates

WHEN root branches are defined in the repo state AND those branches exist as Git branches THEN they are included in the
graph's `root_candidates` list

#### Scenario: Fallback root candidates when no roots are defined

WHEN no root branches are defined in the repo state THEN the currently checked-out branch (HEAD) is used as the root
candidate AND if HEAD is unavailable, the first branch alphabetically is used

#### Scenario: Orphan branches can be auto-parented to the default root

WHEN the `BranchGraphBuilder` is configured with `with_orphan_parenting(true)` AND a default root branch exists THEN
branches with no `primary_parent` (and that are not themselves root branches) are attached as children of the default
root AND edges are created from the default root to each orphan AND the orphans' `primary_parent` is set to the default
root

#### Scenario: Divergence is calculated against the primary parent

WHEN a branch has a `primary_parent` set via dependencies THEN the ahead/behind counts are calculated between the branch
and its primary parent AND the result is stored in the branch node's `divergence` metadata

#### Scenario: Orphans get divergence against the default root

WHEN a branch has no `primary_parent` AND a default root branch exists AND the branch is not the default root itself
THEN the ahead/behind counts are calculated between the branch and the default root

#### Scenario: The default root has no divergence

WHEN a branch is the default root AND it has no primary parent dependency THEN no divergence is calculated (divergence
is `None`)

### Requirement: Dependency tree traversal

#### Scenario: Finding the root of a dependency tree

WHEN `find_dependency_tree_root` is called for a branch THEN it traverses up the parent chain until it finds a branch
with no parents AND returns that branch name

#### Scenario: Branch with no dependencies is its own root

WHEN `find_dependency_tree_root` is called for a branch that has no parent dependencies THEN the branch itself is
returned as the root

#### Scenario: Traversal follows the first parent in multi-parent cases

WHEN a branch has multiple parents THEN `find_dependency_tree_root` follows only the first parent at each step

### Requirement: Bulk dependency removal

#### Scenario: Removing all dependencies for a branch

WHEN `remove_all_dependencies_for_branch` is called with a branch name THEN all dependencies where the branch appears as
either a child or a parent are removed AND the indices are rebuilt AND the count of removed dependencies is returned

### Requirement: Stale branch eviction of dependencies

#### Scenario: Dependencies are evicted when the child branch no longer exists locally

WHEN `evict_stale_branches` is called with a set of locally existing branches AND a dependency's child branch is not in
the local set and is not a root branch THEN that dependency is removed

#### Scenario: Dependencies are preserved when only the parent branch is gone

WHEN a dependency's parent branch does not exist locally AND the dependency's child branch does exist locally (or is a
root branch) THEN the dependency is not removed

#### Scenario: Root branches protect dependencies from eviction

WHEN a dependency's child branch is a root branch AND that branch does not exist locally THEN the dependency is
preserved (root branches are never evicted)
