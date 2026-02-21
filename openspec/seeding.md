# OpenSpec Seeding Tasks

Prioritized task list for seeding OpenSpec specs from existing twig functionality. Each task is self-contained — an
agent can pick any unblocked task and work independently.

**Spec format:** Each spec lives at `openspec/specs/<capability>/spec.md`. Use `### Requirement:` headings with
`#### Scenario:` blocks using **WHEN/THEN/AND** pattern. Group requirements by subcommand where applicable.

**How to work a task:** Read the listed source files, extract the actual behavior, and write requirements that describe
what the system does today. Don't invent new behavior — document what exists.

______________________________________________________________________

## Dependency Graph

```
P0: branch-dependencies ──┬──> P1: rebase
                          ├──> P1: cascade-rebase
                          ├──> P1: branch-tree
                          ├──> P1: branch-switching
                          └──> P2: branch-adoption

P0: credentials ──────────┬──> P2: github-integration
                          ├──> P2: jira-integration
                          └──> P2: auto-sync

P0: initialization           (no downstream blockers)
P1: commit-prefill           (independent)
P1: fixup-commits            (independent)
P2: repository-registry      (independent)
P2: stale-branch-cleanup     (independent)
P2: worktrees                (independent)
P3: plugin-system            (independent)
P3: mcp-server               (independent)
P3: self-management          (independent)
```

**Immediately parallelizable (11 tasks):** #4, #5, #6, #11, #12, #16, #18, #19, #20, #21, #22

______________________________________________________________________

## P0 — Foundation

These capabilities are depended on by other specs. Seed first.

______________________________________________________________________

### Task 1: branch-dependencies

**Spec file:** `openspec/specs/branch-dependencies/spec.md` **Unblocks:** rebase, cascade-rebase, branch-tree,
branch-switching, branch-adoption

#### Why This Is P0

This is the foundational data model. Every other capability (rebase, cascade, tree, adoption, switching) depends on
branch dependency relationships and root branches. Spec this first so other specs can reference it.

#### Source Files to Read

- `twig-cli/src/cli/branch.rs` — Clap structs and `handle_branch` dispatcher for all subcommands
- `twig-core/src/state.rs` — `BranchDependency`, `RootBranch`, `RepoState` (add/remove dependency, add/get root,
  indices)
- `twig-core/src/git/graph.rs` — `BranchGraph`, `BranchGraphBuilder` (how dependencies feed into the graph)

#### What to Capture

1. **Adding a dependency** (`twig branch depend <child> <parent>`) — creates parent→child edge in state, validation
   (branches must exist, no self-loops, no duplicates)
1. **Removing a dependency** (`twig branch rm-dep <child> <parent>`) — removes edge, error if doesn't exist
1. **Querying parent** (`twig branch parent [branch]`) — shows parent(s) of current or named branch
1. **Root branch management** — add root (`--default` flag), list roots, remove root, default root behavior
1. **State persistence** — dependencies and roots stored in `.twig/state.json`, survive across sessions
1. **Index consistency** — `dependency_children_index` and `dependency_parents_index` rebuilt on load

______________________________________________________________________

### Task 2: initialization

**Spec file:** `openspec/specs/initialization/spec.md` **Unblocks:** —

#### Why This Is P0

Initialization creates the directory structure and config files that every other capability depends on. Must be specced
before capabilities that reference config paths.

#### Source Files to Read

- `twig-cli/src/cli/config.rs` — `handle_init` function, what gets created
- `twig-core/src/config.rs` — `ConfigDirs` struct: `new()`, `init()`, directory paths (config_dir, data_dir, cache_dir,
  registry_path, repo_state_path, jira_config_path)
- `twig-core/src/state.rs` — `ensure_twig_internal_gitignore()`, initial Registry creation

#### What to Capture

1. **First-time init** (`twig init`) — creates XDG dirs, empty registry.json, success message
1. **Idempotent re-init** — running init again doesn't destroy existing data
1. **XDG compliance** — uses `directories::ProjectDirs` with qualifier `eddieland/twig`, respects
   XDG_CONFIG_HOME/XDG_DATA_HOME overrides
1. **Per-repo setup** — `.twig/` directory creation, `.twig/.gitignore` auto-generated to keep state out of version
   control
1. **Directory structure** — config_dir for settings, data_dir for registry, cache_dir for transient data

______________________________________________________________________

### Task 3: credentials

**Spec file:** `openspec/specs/credentials/spec.md` **Unblocks:** github-integration, jira-integration, auto-sync

#### Why This Is P0

GitHub and Jira integrations both depend on credentials. Spec this before those integration capabilities.

#### Source Files to Read

- `twig-cli/src/cli/creds.rs` — `handle_creds` (check/setup subcommands)
- `twig-core/src/creds/mod.rs` — `Credentials` trait, `get_credentials_for_host()`
- `twig-core/src/creds/netrc.rs` — `Netrc` struct, `parse_netrc()`, `get_machine_credentials()`
- `twig-core/src/creds/platform/unix.rs` and `windows.rs` — platform credential store backends
- `twig-gh/src/auth.rs` — `create_github_client_from_netrc()`, `get_github_credentials()`
- `twig-jira/src/auth.rs` — `create_jira_client_from_netrc()`, `get_jira_credentials()`

#### What to Capture

1. **Credential check** (`twig creds check`) — verifies GitHub and Jira credentials are configured, reports status
1. **Credential setup** (`twig creds setup`) — interactive flow to configure credentials
1. **Netrc parsing** — reads `~/.netrc`, matches by machine hostname, extracts login/password
1. **GitHub auth** — looks up `api.github.com` in netrc, uses as token-based auth
1. **Jira auth** — looks up Jira host in netrc, uses username + API token
1. **Platform stores** — Unix keyring and Windows Credential Manager as backends
1. **Graceful failure** — clear error messages when credentials missing or invalid

______________________________________________________________________

## P1 — Core Workflow

These are twig's differentiating features for stacked PR workflows.

______________________________________________________________________

### Task 4: rebase

**Spec file:** `openspec/specs/rebase/spec.md` **Blocked by:** branch-dependencies

#### Why This Is P1

Core workflow operation. Cascade-rebase builds on this, so spec rebase first.

#### Source Files to Read

- `twig-cli/src/cli/rebase.rs` — Clap struct, `handle_rebase` function
- Look for the rebase execution logic (likely in twig-cli or twig-core) — the actual `git rebase` invocation, parent
  resolution from dependency graph

#### What to Capture

1. **Basic rebase** (`twig rebase`) — rebases current branch onto its dependency-tree parent
1. **Parent resolution** — uses twig dependency graph (not git tracking branch) to find parent
1. **Up-to-date detection** — skips rebase if already up-to-date (unless `--force`)
1. **Force flag** — `--force` rebases even when up-to-date
1. **Graph preview** — `--show-graph` displays dependency graph before rebasing
1. **Autostash** — `--autostash` stashes uncommitted changes, rebases, pops stash
1. **Repo override** — `-r/--repo` targets a different repository
1. **Error handling** — rebase conflicts, missing parent, not in a repo

______________________________________________________________________

### Task 5: cascade-rebase

**Spec file:** `openspec/specs/cascade-rebase/spec.md` **Blocked by:** branch-dependencies

#### Why This Is P1

This is twig's killer feature — the primary differentiator for stacked PR workflows.

#### Source Files to Read

- `twig-cli/src/cli/cascade.rs` — Clap struct, `handle_cascade` function
- Look for cascade logic in twig-cli or twig-core — topological ordering, recursive rebase through children

#### What to Capture

1. **Cascade execution** (`twig cascade`) — rebases current branch, then cascades to all descendants in topological
   order
1. **Topological ordering** — children rebased after parents, siblings in deterministic order
1. **Max depth** — `--max-depth` limits how deep the cascade goes
1. **Preview mode** — `--preview` shows the rebase plan without executing
1. **Force flag** — `--force` rebases even if branches are up-to-date
1. **Graph preview** — `--show-graph` shows dependency graph before cascading
1. **Autostash** — `--autostash` handles uncommitted changes
1. **Conflict handling** — what happens when a rebase in the middle of the cascade conflicts
1. **Branch restoration** — returns to original branch after cascade completes

______________________________________________________________________

### Task 6: branch-tree

**Spec file:** `openspec/specs/branch-tree/spec.md` **Blocked by:** branch-dependencies

#### Why This Is P1

Primary visualization — how users understand their branch topology at a glance.

#### Source Files to Read

- `twig-cli/src/cli/tree.rs` — Clap struct, `handle_tree` function
- `twig-core/src/git/graph.rs` — `BranchGraph`, `BranchGraphBuilder`, `BranchNode`, `BranchTopology`, `BranchDivergence`
- `twig-core/src/git/tree.rs` — `determine_render_root()`, `find_orphaned_branches()`,
  `attach_orphans_to_default_root()`, `annotate_orphaned_branches()`, `filter_branch_graph()`
- `twig-core/src/git/renderer.rs` — `BranchTableRenderer`, `BranchTableSchema`, columns, styling

#### What to Capture

1. **Tree display** (`twig tree`) — shows branches in tree layout based on dependency graph
1. **Column layout** — branch name, linked Jira issue, GitHub PR, divergence (ahead/behind)
1. **Root selection** — priority: explicit override → default root → first candidate → current branch
1. **Orphan handling** — orphaned branches annotated, optionally attached to default root
1. **Max depth** — `--max-depth` limits tree depth
1. **Color modes** — ANSI colors (auto/yes/no), `--no-color` flag
1. **Current branch highlighting** — visual indicator for checked-out branch
1. **Repo override** — `-r/--repo` targets a different repository

______________________________________________________________________

### Task 7: branch-switching

**Spec file:** `openspec/specs/branch-switching/spec.md` **Blocked by:** branch-dependencies

#### Why This Is P1

Primary navigation — how users move between branches with automatic Jira/PR linking.

#### Source Files to Read

- `twig-cli/src/cli/switch.rs` — Clap struct, `handle_switch` function
- `twig-core/src/git/switch.rs` — `SwitchInput` enum, `detect_switch_input()`, `switch_from_input()`,
  `switch_to_branch_name()`, `switch_from_jira()`, `switch_from_pr()`, `ParentBranchOption`

#### What to Capture

1. **Switch by branch name** (`twig switch <name>`) — checks out existing branch
1. **Switch by Jira issue** (`twig switch PROJ-123`) — finds or creates branch linked to issue
1. **Switch by GitHub PR** (`twig switch 42` or URL) — finds branch for PR
1. **Input auto-detection** — `detect_switch_input()` identifies type from pattern
1. **Branch creation** — creates branch if doesn't exist (unless `--no-create`)
1. **Parent linking** — `-p/--parent` sets dependency (values: `current`, branch name, Jira key, `none`)
1. **Root switching** — `--root` switches to the dependency tree root of current branch
1. **Association storage** — stores Jira/PR links in `.twig/state.json` on switch
1. **Remote branch tracking** — checks out remote branches as local tracking branches

______________________________________________________________________

### Task 8: commit-prefill

**Spec file:** `openspec/specs/commit-prefill/spec.md` **Blocked by:** — (independent)

#### Why This Is P1

Daily workflow convenience — used on nearly every commit when working with Jira.

#### Source Files to Read

- `twig-cli/src/cli/commit.rs` — Clap struct, `handle_commit` function
- Look for commit logic — how it fetches Jira issue summary, formats message, detects duplicate for fixup

#### What to Capture

1. **Basic commit** (`twig commit`) — fetches Jira issue linked to current branch, uses key + summary as commit message
1. **Custom message** (`-m/--message`) — overrides Jira summary with custom text
1. **Prefix/suffix** (`-p/--prefix`, `-s/--suffix`) — prepend/append text to the generated message
1. **Fixup detection** — checks if an identical commit message already exists in history, offers fixup instead (unless
   `--no-fixup`)
1. **No Jira link** — graceful error when current branch has no linked Jira issue
1. **Staged changes required** — error when nothing is staged

______________________________________________________________________

### Task 9: fixup-commits

**Spec file:** `openspec/specs/fixup-commits/spec.md` **Blocked by:** — (independent)

#### Why This Is P1

Pairs with commit-prefill for the daily commit workflow in stacked PRs.

#### Source Files to Read

- `twig-cli/src/cli/fixup.rs` — Clap struct, `handle_fixup` function
- Look for fixup logic — commit listing, filtering, interactive selection, `git commit --fixup`

#### What to Capture

1. **Interactive selection** (`twig fixup`) — presents recent commits, user selects target for fixup
1. **Commit filtering** — `--limit` (default 20), `--days` (default 30), `--all-authors` (default: current user only)
1. **Fixup inclusion** — `--include-fixups` shows existing fixup! commits in the list
1. **Dry run** — `--dry-run` shows what would happen without creating the fixup
1. **Vim mode** — `--vim-mode` enables vim-style modal navigation in the selector
1. **Staged changes required** — error when nothing is staged
1. **Fixup commit format** — creates `fixup! <original message>` commit

______________________________________________________________________

## P2 — Integrations & Management

______________________________________________________________________

### Task 10: github-integration

**Spec file:** `openspec/specs/github-integration/spec.md` **Blocked by:** credentials

#### Why This Is P2

Important integration but depends on credentials spec (P0) being understood.

#### Source Files to Read

- `twig-cli/src/cli/github.rs` — Clap structs for all subcommands, handlers
- `twig-gh/src/client.rs` — `GitHubClient`
- `twig-gh/src/endpoints/pulls.rs` — `list_pull_requests()`, `get_pull_request()`, `list_pull_request_reviews()`,
  `get_pull_request_status()`
- `twig-gh/src/endpoints/checks.rs` — `get_check_runs()`, `get_check_suites()`
- `twig-gh/src/models.rs` — `GitHubPullRequest`, `PullRequestReview`, `CheckRun`, `PullRequestStatus`
- `twig-core/src/github.rs` — `GitHubPr::parse()`, `GitHubRepo::parse()`

#### What to Capture

1. **Auth check** (`twig github check`) — verifies GitHub credentials work
1. **PR link** (`twig github pr link [url_or_id]`) — links PR to current branch in state
1. **PR list** (`twig github pr list`) — lists PRs with state filter (open/closed/all), limit, repo override
1. **PR status** (`twig github pr status`) — shows PR for current branch with review status and CI checks
1. **CI checks** (`twig github checks [pr_number]`) — shows CI/CD check runs for a PR
1. **Open in browser** (`twig github open [pr_number]`) — opens PR URL in default browser
1. **Remote detection** — parses GitHub owner/repo from git remote URL (SSH and HTTPS)
1. **Current branch default** — commands default to the PR linked to the current branch

______________________________________________________________________

### Task 11: jira-integration

**Spec file:** `openspec/specs/jira-integration/spec.md` **Blocked by:** credentials

#### Why This Is P2

Important integration but depends on credentials spec (P0) being understood.

#### Source Files to Read

- `twig-cli/src/cli/jira.rs` — Clap structs for all subcommands, handlers
- `twig-jira/src/client.rs` — `JiraClient`
- `twig-jira/src/endpoints/issues.rs` — `get_issue()`, `list_issues()`
- `twig-jira/src/endpoints/transitions.rs` — `get_transitions()`, `transition_issue()`
- `twig-jira/src/models.rs` — `Issue`, `IssueFields`, `Transition`
- `twig-core/src/jira_parser.rs` — `JiraTicketParser`, `JiraParsingMode`, `JiraParsingConfig`

#### What to Capture

1. **View issue** (`twig jira view [key]`) — displays issue details (summary, status, assignee, description)
1. **Open in browser** (`twig jira open [key]`) — opens issue URL in default browser
1. **Create branch** (`twig jira create-branch <key>`) — creates branch named from issue, links it, optional
   `--with-worktree`
1. **Link branch** (`twig jira link-branch [key] [branch]`) — links existing branch to Jira issue in state
1. **Transition** (`twig jira transition [key] [transition]`) — moves issue through workflow states (interactive
   selection if not specified)
1. **Config** (`twig jira config`) — configure parsing mode (strict/flexible), `--show` to display current config
1. **Issue key detection** — regex-based matching of PROJ-123 patterns, configurable project keys
1. **Current branch default** — commands default to the Jira issue linked to the current branch

______________________________________________________________________

### Task 12: auto-sync

**Spec file:** `openspec/specs/auto-sync/spec.md` **Blocked by:** credentials

#### Why This Is P2

Bridges branches to integrations — depends on understanding GitHub and Jira linking patterns.

#### Source Files to Read

- `twig-cli/src/cli/sync.rs` — Clap struct, `handle_sync` function
- Look for sync logic — how it scans branches, detects Jira keys in branch names, finds GitHub PRs from remote tracking

#### What to Capture

1. **Full sync** (`twig sync`) — scans all branches, detects Jira issues and GitHub PRs, stores associations
1. **Jira detection** — extracts issue keys from branch names using configured parser (strict/flexible mode)
1. **GitHub detection** — finds PRs by matching branch names to open PRs
1. **Dry run** — `--dry-run` shows what would be linked without making changes
1. **Force mode** — `--force` updates existing associations that differ from detected patterns
1. **Selective skip** — `--no-jira` skips Jira detection, `--no-github` skips GitHub detection
1. **Repo override** — `-r/--repo` targets a different repository
1. **Idempotency** — running sync twice produces the same result

______________________________________________________________________

### Task 13: repository-registry

**Spec file:** `openspec/specs/repository-registry/spec.md` **Blocked by:** — (independent)

#### Why This Is P2

Multi-repo management is a key feature but not foundational for the stacked PR workflow.

#### Source Files to Read

- `twig-cli/src/cli/git.rs` — Clap structs for all subcommands, handlers (add, remove, list, exec, fetch,
  stale-branches)
- `twig-core/src/state.rs` — `Registry` struct, `Repository` struct, load/save/add/remove/list/update_fetch_time

#### What to Capture

1. **Add repo** (`twig git add [path]`) — registers repo in global registry, defaults to CWD
1. **Remove repo** (`twig git rm [path]`) — unregisters repo, defaults to CWD
1. **List repos** (`twig git list`) — shows all registered repos with names and paths
1. **Execute command** (`twig git exec <command>`) — runs git command in repo(s), `-a/--all` for all registered repos
1. **Fetch** (`twig git fetch`) — fetches updates, `-a/--all` for all repos, tracks last fetch time
1. **Registry persistence** — stored at `${XDG_DATA_HOME}/twig/registry.json`
1. **Duplicate prevention** — can't add same repo twice
1. **Path resolution** — handles relative paths, symlinks, worktree→main repo resolution

> **Note:** Stale branches subcommand is covered separately in the `stale-branch-cleanup` capability.

______________________________________________________________________

### Task 14: branch-adoption

**Spec file:** `openspec/specs/branch-adoption/spec.md` **Blocked by:** branch-dependencies

#### Why This Is P2

Orphan management is important for graph hygiene but not part of the core daily workflow.

#### Source Files to Read

- `twig-cli/src/cli/adopt.rs` — Clap struct, `handle_adopt` function
- `twig-core/src/git/tree.rs` — `find_orphaned_branches()`, `attach_orphans_to_default_root()`
- Look for adoption mode logic — auto, default-root, branch modes

#### What to Capture

1. **Auto mode** (`twig adopt --mode auto`) — heuristically determines best parent for each orphan
1. **Default-root mode** (`twig adopt --mode default-root`) — adopts all orphans under the default root branch
1. **Branch mode** (`twig adopt --mode branch --parent <branch>`) — adopts all orphans under a specific branch
1. **Preview tree** — shows proposed adoption as a tree before confirming
1. **Confirmation** — prompts before applying (unless `-y/--yes`)
1. **Max depth** — `--max-depth` limits preview tree depth
1. **No-color** — `--no-color` disables colored output in preview
1. **Repo override** — `-r/--repo` targets a different repository

______________________________________________________________________

### Task 15: stale-branch-cleanup

**Spec file:** `openspec/specs/stale-branch-cleanup/spec.md` **Blocked by:** — (independent)

#### Why This Is P2

Maintenance capability — keeps the branch list clean over time.

#### Source Files to Read

- `twig-cli/src/cli/git.rs` — stale-branches subcommand struct and handler
- `twig-core/src/state.rs` — `RepoState::evict_stale_branches()`, `EvictionStats`
- `plugins/twig-prune/src/` — the prune plugin (merged PR / completed Jira detection, interactive deletion)

#### What to Capture

1. **List stale branches** (`twig git stale-branches`) — shows branches not updated in N days (default 30)
1. **Days threshold** (`-d/--days`) — configurable staleness threshold
1. **Interactive prune** (`-p/--prune`) — interactive mode to select and delete stale branches
1. **JSON output** (`--json`) — machine-readable output for scripting
1. **State eviction** — `evict_stale_branches()` removes deleted branches from state (metadata, dependencies, indices)
1. **twig-prune plugin** — detects branches with merged GitHub PRs or completed Jira issues, offers deletion with
   `--dry-run` and `--skip-prompts`
1. **Safe deletion** — never deletes current branch or root branches

______________________________________________________________________

### Task 16: worktrees

**Spec file:** `openspec/specs/worktrees/spec.md` **Blocked by:** — (independent)

#### Why This Is P2

Parallel development support — complements the stacked workflow but not core to it.

#### Source Files to Read

- `twig-cli/src/cli/worktree.rs` — Clap structs for create/list/clean, handlers
- `twig-core/src/state.rs` — `Worktree` struct, `RepoState::add_worktree()` / `remove_worktree()`
- `twig-core/src/git/detection.rs` — `resolve_to_main_repo_path()` (worktree→main repo resolution)

#### What to Capture

1. **Create worktree** (`twig worktree create <branch>`) — creates git worktree for branch, records in state
1. **List worktrees** (`twig worktree list`) — shows all active worktrees for the repo
1. **Clean worktrees** (`twig worktree clean`) — removes stale/orphaned worktrees
1. **State tracking** — worktrees stored in `.twig/state.json`
1. **Main repo resolution** — when running from a worktree, resolves back to main repo for state access
1. **Jira integration** — `twig jira create-branch --with-worktree` creates branch + worktree together
1. **Repo override** — `-r/--repo` targets a different repository

______________________________________________________________________

## P3 — Supporting

______________________________________________________________________

### Task 17: plugin-system

**Spec file:** `openspec/specs/plugin-system/spec.md` **Blocked by:** — (independent)

#### Why This Is P3

Extensibility mechanism — important for architecture but used by few users directly.

#### Source Files to Read

- `twig-core/src/plugin.rs` — `PluginContext` struct, `discover()`, env var handling
- `twig-cli/src/cli/self_cmd.rs` — `plugins` subcommand (discovery listing)
- `twig-cli/src/cli/mod.rs` — external command dispatch (how `twig <plugin-name>` resolves to `twig-<name>` binary)
- `plugins/twig-flow/` and `plugins/twig-prune/` — reference implementations

#### What to Capture

1. **Discovery** (`twig self plugins`) — finds `twig-*` binaries on PATH, lists them
1. **Execution** (`twig <name>`) — dispatches to `twig-<name>` binary with remaining args
1. **Context passing** — sets TWIG_CONFIG_DIR, TWIG_DATA_DIR, TWIG_CURRENT_REPO, TWIG_CURRENT_BRANCH, TWIG_VERSION,
   TWIG_COLORS, TWIG_NO_LINKS, TWIG_VERBOSITY env vars
1. **Plugin context discovery** — `PluginContext::discover()` reads env vars with filesystem fallback
1. **Plugin-specific dirs** — `plugin_config_dir(name)` and `plugin_data_dir(name)` for per-plugin storage
1. **Library usage** — plugins can depend on twig-core as a library crate

______________________________________________________________________

### Task 18: mcp-server

**Spec file:** `openspec/specs/mcp-server/spec.md` **Blocked by:** — (independent)

#### Why This Is P3

AI integration — valuable but not part of the core Git workflow.

#### Source Files to Read

- `twig-mcp/src/server.rs` — all 12 tool handlers with `#[tool_router]` / `#[tool_handler]`
- `twig-mcp/src/context.rs` — `ServerContext` (repo detection, lazy GitHub/Jira client init)
- `twig-mcp/src/types.rs` — response types, `ToolResponse<T>`
- `twig-mcp/src/tools/local.rs` — parameter types for 7 local tools
- `twig-mcp/src/tools/github.rs` — parameter types for 3 GitHub tools
- `twig-mcp/src/tools/jira.rs` — parameter types for 2 Jira tools
- `docs/specs/20260220_twig-mcp-server.md` — existing detailed spec (can be migrated)

#### What to Capture

1. **Local tools** — get_current_branch, get_branch_metadata, get_branch_tree, get_branch_stack, list_branches,
   list_repositories, get_worktrees
1. **GitHub tools** — get_pull_request, get_pr_status, list_pull_requests
1. **Jira tools** — get_jira_issue, list_jira_issues
1. **Transport** — stdio-based MCP communication via rmcp
1. **Tool annotations** — all tools marked `read_only_hint: true`, `idempotent_hint: true`
1. **Lazy auth** — GitHub/Jira clients initialized on first use via `OnceCell`
1. **Error handling** — graceful errors when repo not detected or credentials missing

> **Note:** There's already a detailed spec at `docs/specs/20260220_twig-mcp-server.md` — migrate relevant content to
> OpenSpec format.

______________________________________________________________________

### Task 19: self-management

**Spec file:** `openspec/specs/self-management/spec.md` **Blocked by:** — (independent)

#### Why This Is P3

Maintenance utilities — important for usability but not core workflow.

#### Source Files to Read

- `twig-cli/src/cli/self_cmd.rs` — Clap structs and handlers for update, diagnose, completion, plugins subcommands
- Look for update logic — GitHub release fetching, binary replacement
- Look for diagnose logic — what system checks are performed

#### What to Capture

1. **Self-update** (`twig self update`) — downloads latest release from GitHub, replaces binary, `--force` reinstalls
   even if current
1. **Plugin updates** — `twig self update flow/prune/mcp` installs or updates specific plugins
1. **Diagnostics** (`twig self diagnose`) — checks: git version, credentials, config dirs, registry, etc.
1. **Shell completions** (`twig self completion <shell>`) — generates completions for bash/zsh/fish/powershell
1. **Plugin discovery** (`twig self plugins`) — lists installed twig-\* plugins found on PATH
