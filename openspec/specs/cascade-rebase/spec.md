# Cascade Rebase

## Purpose

Rebase the current branch and then automatically cascade the rebase to all descendant branches in the dependency tree.
This is the core operation for maintaining stacked PRs — when a base branch changes, all branches built on top of it are
updated in topological order.

**CLI surface:** `twig cascade` (alias `casc`), flags: `--max-depth`, `--force`, `--show-graph`, `--autostash`,
`--preview`, `-r` **Crates:** `twig-core` (git ops, graph, state), `twig-cli` (cascade command module)

## Requirements

### Requirement: Repository resolution

#### Scenario: Auto-detect repository from current directory

WHEN the user runs `twig cascade` without the `-r` flag THEN the repository is detected by walking up from the current
working directory AND if no Git repository is found, the command fails with "Not in a git repository"

#### Scenario: Explicit repository path override

WHEN the user runs `twig cascade -r <path>` THEN the command operates on the repository at `<path>` instead of the
auto-detected repository

### Requirement: HEAD must be a branch

#### Scenario: Detached HEAD is rejected

WHEN the user runs `twig cascade` AND HEAD is not a branch (e.g., detached HEAD state) THEN the command fails with "HEAD
is not a branch. Cannot cascade rebase."

### Requirement: Dependency tree discovery

#### Scenario: Descendants are discovered from the dependency graph

WHEN the cascade begins THEN the current branch's descendants are discovered by querying the RepoState dependency graph
(children, grandchildren, etc.) using breadth-first traversal AND each descendant is visited at most once (cycle-safe
via visited set)

#### Scenario: No child branches found

WHEN the current branch has no children in the dependency graph THEN the command prints a warning "No child branches
found for the current branch." AND exits successfully without performing any rebase

#### Scenario: No local branches found

WHEN the repository has no branches tracked in the dependency resolver THEN the command prints a warning "No local
branches found." AND exits successfully

### Requirement: Topological rebase ordering

#### Scenario: Parents are rebased before children

WHEN the rebase order is determined THEN a topological sort is performed on the descendant subgraph AND parent branches
appear before their children in the rebase order AND a branch is never rebased until all of its ancestor branches
(within the descendant set) have been rebased

#### Scenario: Direct children of the starting branch are processed first

WHEN the topological sort begins THEN the immediate children of the starting branch are visited first AND then remaining
unvisited branches are processed in declaration order

#### Scenario: Cycles in the dependency graph are tolerated

WHEN a cycle is detected during topological sorting THEN the cyclic node is skipped (not revisited) AND the sort
completes with the remaining branches

### Requirement: Max depth limiting

#### Scenario: Depth is not limited by default

WHEN the user runs `twig cascade` without `--max-depth` THEN all descendants at any depth are included in the cascade

#### Scenario: Limiting cascade depth

WHEN the user runs `twig cascade --max-depth <N>` THEN only descendants up to depth N are included AND depth 1 means
immediate children only, depth 2 means children and grandchildren, etc. AND branches beyond the specified depth are not
rebased

### Requirement: Preview mode

#### Scenario: Preview shows the plan without executing

WHEN the user runs `twig cascade --preview` THEN the command displays the number of branches that would be rebased AND
for each branch, it prints the branch name and its parent (in the format `<branch> onto <parent>`) AND the dependency
tree is displayed AND no actual rebase is performed AND the command exits successfully

### Requirement: Show graph before cascade

#### Scenario: Dependency tree is displayed before rebasing

WHEN the user runs `twig cascade --show-graph` THEN the full branch dependency tree is rendered using TreeRenderer
before any rebasing begins AND root branches are displayed as tree roots AND orphaned branches (no dependencies defined)
are listed separately AND the cascade proceeds normally after displaying the graph

### Requirement: Cascade rebase execution

#### Scenario: Each descendant is rebased onto its parent

WHEN the cascade executes THEN for each branch in topological order, the command checks out the branch AND then runs
`git rebase <parent>` AND if the branch has multiple parents, it rebases onto each parent in the order returned by
`get_dependency_parents` with each subsequent rebase operating on the branch tip produced by the previous rebase
(cumulative, not from the original tip)

#### Scenario: Successful rebase

WHEN `git rebase <parent>` succeeds for a branch THEN a success message is printed ("Successfully rebased <branch> onto
<parent>") AND the cascade continues to the next branch

#### Scenario: Branch is already up-to-date (no force)

WHEN `git rebase <parent>` reports "up to date" AND `--force` is not set THEN the command prints an info message
("<branch> is already up-to-date with <parent>") AND the cascade continues to the next branch without re-rebasing

#### Scenario: Branch with no known parents is skipped

WHEN a branch in the rebase order has no parents in the dependency graph THEN a warning is printed ("No parent branches
found for <branch>, skipping") AND the cascade continues to the next branch

#### Scenario: Checkout failure skips the branch

WHEN the `git checkout <branch>` command does not output "Switched to branch" or "Already on" THEN an error is printed
AND the branch is skipped AND the cascade continues to the next branch

### Requirement: Force rebase

#### Scenario: Force rebase when up-to-date

WHEN the user runs `twig cascade --force` AND a branch is already up-to-date with its parent THEN the command runs
`git rebase --force-rebase <parent>` instead of skipping AND on success, prints "Successfully force-rebased <branch>
onto <parent>"

#### Scenario: Force rebase failure

WHEN `git rebase --force-rebase` fails (non-conflict) THEN an error is printed AND the branch is skipped AND the cascade
continues to the next branch

### Requirement: Autostash

#### Scenario: Autostash passes through to git rebase

WHEN the user runs `twig cascade --autostash` THEN every `git rebase` invocation includes the `--autostash` flag AND
this applies to both normal and force rebases AND Git automatically stashes uncommitted changes before the rebase and
pops them after

### Requirement: Conflict handling

The interactive conflict resolution prompt and its four options ("Continue", "Abort to original", "Abort stay here",
"Skip") are defined in `rebase/spec.md` under "Interactive conflict resolution". The cascade command uses the same
prompt and mechanics. The scenarios below document only where the cascade's behavior diverges from standalone rebase.

#### Scenario: "Continue" resumes the cascade

WHEN the user selects "Continue" during a cascade conflict THEN `git rebase --continue` is executed AND the cascade
proceeds to the next branch in topological order (in standalone rebase, the command simply finishes)

#### Scenario: "Abort to original" terminates the cascade

WHEN the user selects "Abort to original" during a cascade conflict THEN `git rebase --abort` is executed AND the
original branch (the branch the user was on when the cascade started) is checked out AND the cascade terminates
immediately — no further branches are rebased

#### Scenario: "Abort stay here" skips only the current branch

WHEN the user selects "Abort stay here" during a cascade conflict THEN `git rebase --abort` is executed AND the cascade
continues to the next branch (in standalone rebase, the command returns immediately)

#### Scenario: "Skip" continues the cascade

WHEN the user selects "Skip" during a cascade conflict THEN `git rebase --skip` is executed AND the cascade continues to
the next branch

### Requirement: Rebase error handling

#### Scenario: Non-conflict rebase error continues cascade

WHEN `git rebase` fails with a non-conflict error (exit code non-zero, no "CONFLICT" in output) THEN an error message is
printed AND the cascade continues to the next branch rather than aborting the entire operation

### Requirement: Branch restoration after cascade

#### Scenario: Original branch is restored on completion

WHEN the cascade completes (all branches processed or no early termination) THEN the command checks out the original
branch (the branch the user was on when the cascade started) AND prints "Cascading rebase completed successfully"

#### Scenario: Original branch is restored on abort-to-original

WHEN the user selects "Abort to original" during conflict resolution THEN the original branch is checked out AND the
cascade terminates AND the command returns successfully (no error)

### Requirement: Graph display in preview and show-graph

#### Scenario: Tree uses user-defined roots and dependencies

WHEN the dependency tree is displayed (via `--show-graph` or `--preview`) THEN the tree is built from user-defined
dependencies in RepoState AND root branches are determined from user-configured roots AND orphaned branches (no
dependencies, not a root) are listed separately AND the TreeRenderer is used for consistent visual output
