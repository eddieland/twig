# Branch Tree

## Purpose

Visualize the branch dependency graph as a formatted tree/table in the terminal. Displays branch names, linked Jira
issues, GitHub PRs, divergence counts, and orphan annotations in a readable columnar layout with ANSI color support.

**CLI surface:** `twig tree` (alias `t`), flags: `--max-depth`, `--no-color`, `-r` **Crates:** `twig-core` (graph,
renderer, tree algorithms), `twig-cli` (tree command module)

## CLI Command

### Requirement: Tree command invocation

The `twig tree` command (alias `t`) renders the branch dependency tree for the current or specified repository.

#### Scenario: Displaying the tree for the current repository

WHEN the user runs `twig tree` THEN the command detects the current git repository AND loads the repository state from
`.twig/state.json` AND resolves user-defined branch dependencies AND renders the branch tree to stdout

#### Scenario: Targeting a different repository with `-r`

WHEN the user runs `twig tree -r <path>` THEN the command operates on the repository at `<path>` instead of
auto-detecting from the current working directory

#### Scenario: No local branches found

WHEN the user runs `twig tree` AND the repository has no local branches THEN a warning is printed: "No local branches
found." AND the command exits successfully

#### Scenario: No user-defined dependencies or root branches

WHEN the user runs `twig tree` AND the repository state has no user-defined dependencies AND no root branches are
configured THEN an informational message is displayed explaining how to get started AND the message suggests
`twig branch root add`, `twig branch depend`, and `twig branch list` commands AND the command exits successfully without
rendering a tree

#### Scenario: Dependencies exist but no root branches

WHEN the user runs `twig tree` AND user-defined dependencies exist AND no root branches are configured THEN a warning is
displayed: "Found user-defined dependencies but no root branches." AND the available branches are listed AND the user is
prompted to designate a root branch via `twig branch root add`

### Requirement: Max depth limiting

#### Scenario: Limiting tree depth with `--max-depth`

WHEN the user runs `twig tree --max-depth <N>` (or `-d <N>`) THEN branches at depth greater than `<N>` are not rendered
AND the tree is truncated at the specified depth AND root branches are at depth 0

#### Scenario: No max depth specified

WHEN the user runs `twig tree` without `--max-depth` THEN all branches in the dependency tree are rendered regardless of
depth

### Requirement: Color control

#### Scenario: Disabling color with `--no-color`

WHEN the user runs `twig tree --no-color` THEN the output contains no ANSI color escape codes AND branch names,
metadata, and tree connectors are rendered as plain text

#### Scenario: Default color behavior

WHEN the user runs `twig tree` without `--no-color` THEN the current branch name and marker are rendered in bright green
bold AND tree connector glyphs are rendered in dimmed bright black AND Jira issue labels are rendered in cyan AND GitHub
PR annotations are rendered in yellow AND placeholder values for missing metadata are rendered in dimmed bright black
AND the `(current)` marker uses green bold styling

## Tree Construction

### Requirement: Branch node discovery

The tree is built from local git branches combined with user-defined dependency metadata from `.twig/state.json`.

#### Scenario: Collecting local branches

WHEN the tree is built THEN all local git branches are enumerated AND each branch is recorded with its name, head commit
OID, commit summary, author, and timestamp AND the `is_current` flag is set for the branch that is HEAD

#### Scenario: Applying branch metadata from state

WHEN a branch has metadata in the repository state THEN Jira issue keys are attached as labels AND GitHub PR numbers are
attached as annotations with the key `twig.pr`

### Requirement: Dependency resolution

#### Scenario: Building parent-child relationships from user-defined dependencies

WHEN user-defined dependencies exist in the repository state THEN for each dependency where both parent and child
branches exist locally, a parent-child relationship is established AND the first parent encountered becomes the primary
parent AND additional parents become secondary parents AND children lists are sorted alphabetically

#### Scenario: Both parent and child must exist locally

WHEN a dependency references a branch that does not exist as a local git branch THEN that dependency is silently skipped
AND no error is raised

### Requirement: Root branch selection

Root branches anchor the top of the rendered tree. The selection uses a priority chain.

#### Scenario: Configured root branches used as roots

WHEN root branches are configured in the repository state THEN all configured root branches that exist locally are used
as tree roots AND they are rendered in the order they appear in the state

#### Scenario: Fallback to current branch when no roots configured

WHEN no root branches are configured AND no explicit root override is provided THEN the currently checked-out branch is
used as the tree root

#### Scenario: Fallback to first branch when no roots and no HEAD

WHEN no root branches are configured AND no current branch can be determined THEN the first branch alphabetically in the
graph is used as the tree root

### Requirement: Orphaned branch handling

An orphaned branch is one that has no configured parent dependencies and is not marked as a root branch.

#### Scenario: Attaching orphans to the default root

WHEN the default root branch is configured AND orphan parenting is enabled THEN branches without parent dependencies
that are not themselves root branches are attached as children of the default root AND their primary parent is set to
the default root AND edges are created from the default root to each orphan AND the default root's children list is
updated and sorted

#### Scenario: Orphaned branches listed after the tree

WHEN the tree is rendered AND orphaned branches exist that were not attached to a root THEN the orphaned branches are
listed below the tree under a heading AND the user is prompted with commands to organize them via `twig branch root add`
or `twig branch depend`

#### Scenario: Annotating orphaned branches

WHEN a branch is identified as orphaned THEN a `twig.orphan` annotation flag is set to `true` on that branch AND the
renderer appends a dagger symbol (`†`) after the branch name

### Requirement: Divergence calculation

#### Scenario: Divergence against primary parent

WHEN a branch has a primary parent AND both branches exist locally THEN the ahead/behind commit counts are calculated
using `git graph_ahead_behind` AND the result is stored as a `BranchDivergence` on the branch metadata

#### Scenario: Divergence for orphans against default root

WHEN a branch has no primary parent AND a default root branch is configured AND the branch is not the default root
itself THEN the divergence is calculated against the default root branch

#### Scenario: No divergence for the default root itself

WHEN the branch is the default root THEN no divergence is calculated AND the divergence field remains `None`

#### Scenario: Divergence result caching

WHEN divergence is calculated for multiple branches THEN previously computed ahead/behind results are cached by OID pair
AND duplicate git operations are avoided

## Tree Rendering (TreeRenderer)

### Requirement: Tree structure rendering

The `TreeRenderer` renders branches in a hierarchical tree format using Unicode box-drawing characters.

#### Scenario: Root branch displayed at top level

WHEN a root branch is rendered THEN it appears at depth 0 with no tree connector prefix

#### Scenario: Child branches indented with connectors

WHEN a child branch is rendered THEN it is indented relative to its parent AND non-last children use `├── ` as their
connector AND the last child uses `└── ` as its connector AND continuation lines for deeper siblings use `│   ` for
non-last parents and `    ` for last parents

#### Scenario: Current branch highlighted

WHEN the checked-out branch is rendered AND colors are enabled THEN the branch name is displayed in green bold AND
`(current)` is appended after the branch name AND when colors are disabled, the `(current)` text is appended without
styling

#### Scenario: Visited branches are not re-rendered

WHEN a branch has already been rendered in the current traversal (e.g., diamond dependency) THEN it is skipped to
prevent duplicate output

### Requirement: Multiple root rendering

#### Scenario: Multiple root branches rendered sequentially

WHEN multiple root branches are configured THEN each root's subtree is rendered in sequence AND a delimiter (newline) is
inserted between separate root trees

### Requirement: Metadata columns in tree output

The tree renderer displays aligned metadata columns alongside the tree structure.

#### Scenario: Jira issue displayed in brackets

WHEN a branch has linked Jira metadata THEN the Jira issue key is displayed in square brackets (e.g., `[PROJ-123]`) AND
it is aligned to a calculated column position based on the maximum tree width AND when colors are enabled, the Jira
issue is rendered in cyan

#### Scenario: GitHub PR displayed in brackets

WHEN a branch has a linked GitHub PR THEN the PR number is displayed as `[PR#<number>]` AND it is positioned after the
Jira column AND when colors are enabled, the PR number is rendered in yellow

#### Scenario: Cross-references for multi-parent branches

WHEN a branch has multiple parents THEN an `[also: <other-parents>]` annotation is displayed after the PR column AND
this only appears for parents not already shown in the current tree path AND when colors are enabled, the
cross-reference text is dimmed

#### Scenario: Missing metadata has no column rendered

WHEN a branch has no Jira issue AND no GitHub PR AND no cross-references THEN no metadata columns are appended after the
branch name

## Table Rendering (BranchTableRenderer)

### Requirement: Columnar table layout

The `BranchTableRenderer` renders a `BranchGraph` as a tree-aligned columnar table with configurable schema.

#### Scenario: Default schema columns

WHEN the default schema is used THEN three columns are rendered: "Branch" (min width 8), "Story" (min width 8), and "PR"
(min width 6) AND columns are separated by 2 spaces

#### Scenario: Header row rendering

WHEN `show_header` is enabled (the default) THEN a header row is rendered above the data rows AND headers are styled in
bright white bold when colors are enabled

#### Scenario: Header row suppression

WHEN `show_header` is set to `false` THEN no header row is rendered AND data rows begin immediately

#### Scenario: Column width auto-sizing

WHEN the table is rendered THEN each column width is the maximum of: its configured `min_width`, its header text width,
and the widest cell value in that column AND cells are right-padded with spaces to align columns

### Requirement: Branch column rendering

The Branch column must be the first column in any schema.

#### Scenario: Current branch indicator

WHEN the branch is the checked-out branch AND colors are enabled THEN a `*` marker is prefixed to the branch name AND
both the marker and name are styled in bright green bold

#### Scenario: Highlighted branch styling

WHEN a branch is in the highlighted set AND it is not the current branch AND colors are enabled THEN the branch name is
styled in yellow bold

#### Scenario: Orphan marker in branch column

WHEN a branch has the `twig.orphan` annotation set to `true` THEN a dagger symbol (`†`) is appended after the branch
name AND when colors are enabled, the dagger is styled in yellow

#### Scenario: Divergence counts appended to branch name

WHEN a branch has divergence metadata THEN the ahead/behind counts are appended in the format `(+N/-M)` AND when colors
are enabled, the ahead count is green when non-zero and dimmed when zero AND the behind count is red when non-zero and
dimmed when zero

#### Scenario: Tree connectors in branch column

WHEN a branch is rendered at depth > 0 THEN tree connector prefixes (`├─ `, `└─ `, `│  `) are prepended to the branch
name AND when connector dimming is enabled, connectors are styled in bright black

### Requirement: Story column rendering

#### Scenario: Branch has a label (Jira issue)

WHEN the branch has one or more labels THEN the first label is displayed in the Story column AND when colors are
enabled, it is styled in cyan

#### Scenario: Branch has no label

WHEN the branch has no labels THEN the placeholder value (`--` by default) is displayed AND when colors are enabled and
placeholder dimming is active, the placeholder is styled in dimmed bright black

### Requirement: PR column rendering

#### Scenario: Branch has a PR annotation

WHEN the branch has a `twig.pr` annotation THEN its value is displayed in the PR column AND when colors are enabled, it
is styled in yellow

#### Scenario: Branch has no PR annotation

WHEN the branch has no `twig.pr` annotation THEN the placeholder value is displayed

### Requirement: Custom schema support

#### Scenario: Additional annotation columns

WHEN a custom schema includes annotation columns with arbitrary keys THEN the renderer displays the annotation value for
each branch if present AND uses the placeholder for missing values AND non-PR annotations are styled in magenta when
colors are enabled

#### Scenario: Custom placeholder text

WHEN a schema is configured with a custom placeholder (e.g., `"---"`) THEN all missing values use that placeholder
instead of the default `"--"`

#### Scenario: Custom column spacing

WHEN a schema is configured with custom column spacing THEN columns are separated by the specified number of spaces
instead of the default 2

### Requirement: Schema validation

#### Scenario: Empty schema rejected

WHEN a schema with zero columns is used THEN the renderer returns an `EmptySchema` error

#### Scenario: Branch column must be first

WHEN the first column in the schema is not of kind `Branch` THEN the renderer returns a `MissingBranchColumn` error

## Tree Filtering

### Requirement: Pattern-based branch filtering

#### Scenario: Filtering branches by name pattern

WHEN a filter pattern is applied to a branch graph THEN only branches whose names contain the pattern (case-insensitive)
are included AND all ancestor branches up to the root are also included to preserve tree structure AND non-matching
branches that are not ancestors of matches are excluded AND edges between excluded branches are removed

#### Scenario: No branches match the filter

WHEN a filter pattern matches no branches THEN `None` is returned AND no filtered graph is produced

#### Scenario: Filtered children pruned from retained branches

WHEN a branch is retained in the filtered graph THEN its children list is pruned to include only children that are also
in the filtered set

## Render Root Selection

### Requirement: Determining the render root

The render root determines which branch is used as the starting point for tree display.

#### Scenario: Explicit override takes highest priority

WHEN an explicit override branch is provided AND it exists in the graph THEN it is used as the render root

#### Scenario: Configured default root is second priority

WHEN no explicit override is provided AND a default root is configured in the repository state AND it exists in the
graph THEN the configured default root is used

#### Scenario: First root candidate is third priority

WHEN no explicit override is provided AND no configured default root exists AND the graph has root candidates THEN the
first root candidate is used

#### Scenario: Current branch is fourth priority

WHEN no explicit override is provided AND no configured default root exists AND no root candidates exist AND a current
branch exists in the graph THEN the current branch is used

#### Scenario: First graph node is last resort

WHEN no other selection criteria are met AND the graph is non-empty THEN the first branch in iteration order is used as
the render root

## Summary and Guidance

### Requirement: Post-tree summary

#### Scenario: No linked issues or PRs

WHEN the tree is rendered AND no branches have linked Jira issues or GitHub PRs THEN an informational message is
displayed suggesting `twig jira branch link` and `twig github pr link` commands

#### Scenario: Some branches have linked metadata

WHEN the tree is rendered AND at least one branch has a linked Jira issue or GitHub PR THEN no integration guidance
message is displayed
