# Branch Dependencies

## Purpose

Manage custom parent/child relationships between branches and designate root branches that anchor dependency trees. This
is the foundational data model for twig's stacked workflow — all rebase, cascade, tree, and adoption features depend on
these relationships.

**CLI surface:** `twig branch depend`, `twig branch remove-dep`, `twig branch parent`,
`twig branch root add/list/remove` **Crates:** `twig-core` (state, graph), `twig-cli` (branch command module)

## CLI Commands

### Requirement: Common command behavior

All `twig branch` subcommands share these behaviors:

#### Scenario: Dot alias resolves to the current branch

WHEN any branch argument is specified as `.` THEN it is resolved to the name of the currently checked-out branch before
the command executes

#### Scenario: Repository path override

WHEN `-r <path>` is provided THEN the command operates on the repository at `<path>` instead of the auto-detected
repository

### Requirement: Adding a branch dependency

#### Scenario: Successfully adding a dependency

WHEN the user runs `twig branch depend <child> <parent>` THEN a dependency relationship is created with `<parent>` as
the parent of `<child>` AND the dependency is assigned a unique UUID and a `created_at` timestamp AND the dependency is
persisted to `.twig/state.json` AND the in-memory indices are rebuilt AND the CLI reports that the dependency was added

#### Scenario: Duplicate dependency is rejected

WHEN the user runs `twig branch depend <child> <parent>` AND a dependency from `<child>` to `<parent>` already exists
THEN the command fails with an error indicating the dependency already exists AND no new dependency is created

#### Scenario: Cycle detection prevents circular dependencies

WHEN the user runs `twig branch depend <child> <parent>` AND adding this dependency would create a cycle (direct,
transitive, or self-loop) THEN the command fails with an error indicating a circular dependency would be created AND no
new dependency is created

#### Scenario: No Git branch existence validation

WHEN the user runs `twig branch depend <child> <parent>` THEN the dependency is created regardless of whether `<child>`
or `<parent>` exist as Git branches

### Requirement: Removing a branch dependency

#### Scenario: Successfully removing an existing dependency

WHEN the user runs `twig branch remove-dep <child> <parent>` (alias `rm-dep`) AND a dependency from `<child>` to
`<parent>` exists THEN the dependency is removed AND the in-memory indices are rebuilt AND the state file is updated AND
the CLI reports the removal

#### Scenario: Removing a non-existent dependency

WHEN the user runs `twig branch remove-dep <child> <parent>` AND no such dependency exists THEN the CLI prints a warning
that the dependency was not found AND the command returns successfully (no error) AND the state file is not modified

### Requirement: Querying parent branches

#### Scenario: Branch has one or more twig-defined parents

WHEN the user runs `twig branch parent [branch]` AND the branch has twig-defined parent dependencies THEN the CLI
displays the parent branch name(s)

#### Scenario: No twig parent but Git upstream exists

WHEN the user runs `twig branch parent [branch]` AND no twig-defined parents exist AND the branch has a Git upstream
tracking branch THEN the CLI indicates no twig parent is defined and shows the Git upstream

#### Scenario: No twig parent and no Git upstream

WHEN the user runs `twig branch parent [branch]` AND no twig-defined parents exist AND no Git upstream is configured
THEN the CLI indicates no parent branches are defined

#### Scenario: Defaulting to the current branch

WHEN the user runs `twig branch parent` without specifying a branch name THEN the current HEAD branch is used

### Requirement: Root branch management

#### Scenario: Adding a root branch

WHEN the user runs `twig branch root add <branch>` THEN the branch is added to the root branches list with `is_default`
set to `false` AND it is assigned a unique UUID and `created_at` timestamp AND the state file is updated

#### Scenario: Adding a root branch as default

WHEN the user runs `twig branch root add <branch> --default` THEN the branch is added (or promoted) as the default root
AND all other roots have their `is_default` flag cleared

#### Scenario: Re-adding an existing root branch is idempotent

WHEN the user runs `twig branch root add <branch>` AND `<branch>` is already a root branch THEN no new entry is created
AND the existing root's `is_default` flag is unchanged

#### Scenario: Listing root branches

WHEN the user runs `twig branch root list` (alias `ls`) THEN the CLI displays all root branches, indicating which (if
any) is the default AND if no roots are defined, the CLI indicates that

#### Scenario: Removing a root branch

WHEN the user runs `twig branch root remove <branch>` (alias `rm`) AND `<branch>` is a root branch THEN the entry is
removed AND if it was the default, no other root is auto-promoted AND the state file is updated

#### Scenario: Removing a non-existent root branch

WHEN the user runs `twig branch root remove <branch>` AND `<branch>` is not a root branch THEN the CLI prints a warning
AND the command returns successfully

## Core State (`twig-core`)

### Requirement: Branch dependency data model

#### Scenario: BranchDependency fields

WHEN a dependency is created THEN it contains: `id` (UUID v4), `child` (String), `parent` (String), `created_at`
(DateTime<Utc>)

#### Scenario: RootBranch fields

WHEN a root branch is created THEN it contains: `id` (UUID v4), `branch` (String), `is_default` (bool), `created_at`
(DateTime<Utc>)

#### Scenario: At most one default root

WHEN a root branch is set as default THEN all other root branches have `is_default` cleared AND exactly zero or one root
has `is_default` set to `true` at any time

### Requirement: State persistence and index consistency

#### Scenario: Dependencies and roots survive across sessions

WHEN dependencies and root branches are saved to `.twig/state.json` AND a new command loads the state THEN all data is
present AND the `dependency_children_index` and `dependency_parents_index` are rebuilt on load

#### Scenario: Indices are transient (not serialized)

WHEN the state is saved THEN the index fields (`dependency_children_index`, `dependency_parents_index`,
`branch_to_jira_index`, `jira_to_branch_index`, `pr_to_branch_index`) are excluded from JSON AND they are rebuilt from
canonical data on each load

#### Scenario: Index rebuild produces correct mappings

WHEN indices are rebuilt THEN `dependency_children_index` maps each parent to its children AND
`dependency_parents_index` maps each child to its parents

#### Scenario: .twig directory is auto-created with gitignore

WHEN the state is saved and `.twig/` does not exist THEN the directory is created AND `.twig/.gitignore` is written with
a catch-all `*` rule to exclude twig metadata from version control AND repeated saves do not duplicate the rule

### Requirement: Dependency tree traversal

#### Scenario: Finding the root of a dependency tree

WHEN `find_dependency_tree_root` is called for a branch THEN it traverses up the parent chain (following the first
parent at each step) until it finds a branch with no parents AND returns that branch name

#### Scenario: Branch with no dependencies is its own root

WHEN `find_dependency_tree_root` is called for a branch with no parent dependencies THEN the branch itself is returned

### Requirement: Bulk dependency removal

#### Scenario: Removing all dependencies for a branch

WHEN `remove_all_dependencies_for_branch` is called THEN all dependencies where the branch appears as either child or
parent are removed AND the indices are rebuilt AND the count of removed dependencies is returned

### Requirement: Stale branch eviction

#### Scenario: Eviction removes dependencies for missing child branches

WHEN `evict_stale_branches` is called with the set of locally existing branches THEN dependencies whose child branch
does not exist locally (and is not a root branch) are removed AND dependencies are preserved if only the parent is
missing AND root branches are never evicted

## Graph Integration (`twig-core`)

### Requirement: Dependencies create graph edges

#### Scenario: Declared dependencies become graph edges

WHEN the `BranchGraphBuilder` builds with declared dependencies enabled (the default) AND both child and parent branches
exist as Git branches THEN an edge is created from parent to child AND the child's `primary_parent` is set to its first
declared parent AND additional parents are stored as `secondary_parents`

#### Scenario: Non-existent branches are silently skipped

WHEN a dependency references a branch that does not exist in Git THEN no edge is created AND no error is raised

#### Scenario: Dependency integration can be disabled

WHEN `BranchGraphBuilder` is configured with `with_declared_dependencies(false)` THEN no dependency edges are created

### Requirement: Root branch graph integration

#### Scenario: Root branches become root candidates

WHEN root branches are defined and exist as Git branches THEN they are included in the graph's `root_candidates` list

#### Scenario: Fallback when no roots are defined

WHEN no root branches are defined THEN HEAD is used as the root candidate AND if HEAD is unavailable, the first branch
alphabetically is used

### Requirement: Orphan handling and divergence

#### Scenario: Auto-parenting orphans to the default root

WHEN `with_orphan_parenting(true)` is set AND a default root exists THEN branches with no `primary_parent` (and not
themselves roots) are attached as children of the default root

#### Scenario: Divergence calculation

WHEN a branch has a `primary_parent` THEN ahead/behind counts are calculated against it AND when a branch has no parent
but a default root exists, divergence is calculated against the default root AND the default root itself has no
divergence
