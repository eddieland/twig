# Twig Short-Term Improvement Plan

This plan focuses on core branch-centric flows that most Twig users rely on: `twig switch`, Jira / GitHub driven branch creation, cascading rebases, and the dependency tree view. Each task is scoped for a single contributor and aims to land within a few hundred lines of Rust across the CLI and shared core crates.

## Priority Overview

| Priority | Task | Estimated Effort | Rough LoC | Primary Areas |
| --- | --- | --- | --- | --- |
| P0 | Fix parent-aware branch creation in `twig switch` | 2-3 days | 120 | `twig-cli/src/cli/switch.rs`, `twig-core/src/state.rs` |
| P0 | Use upstream PR data when creating branches from GitHub | 2-3 days | 150 | `twig-cli/src/cli/switch.rs`, `twig-gh` |
| P1 | Enrich `twig tree` with branch health signals | 3 days | 180 | `twig-cli/src/cli/tree.rs`, `twig-core/src/tree_renderer.rs` |
| P1 | Add cascade preflight & progress reporting | 2 days | 130 | `twig-cli/src/cli/cascade.rs` |
| P2 | Improve switch input resolution & guidance | 2 days | 100 | `twig-cli/src/cli/switch.rs`, `twig-core::jira_parser` |

---

## P0 — Fix parent-aware branch creation in `twig switch`

**Problem.** When `twig switch` creates a branch (directly or via Jira/PR flows) and a parent is provided, the code still snapshots the current HEAD instead of the parent branch. The helper `create_and_switch_to_branch` always branches from the current checkout before adding a dependency, which produces incorrect ancestry and forces an immediate manual rebase. 【F:twig-cli/src/cli/switch.rs†L408-L450】

**Value.** Ensures dependency metadata aligns with actual Git history, eliminating an entire class of surprise rebases immediately after branch creation and restoring parity with argit.

**Plan.**

1. Resolve the target commit before branch creation:
   - If a parent branch is specified, look up its tip (locally or by fetching the remote) and use it as the base commit.
   - Fall back to HEAD when no parent is provided.
2. Validate the parent exists; if it does not, emit a clear error suggesting `twig branch depend` or `twig branch root add` instead of silently creating the branch.
3. Update `create_and_switch_to_branch` to accept a `BranchBase` enum (e.g., `Head`, `Parent(String)`) and handle checkout of the base commit before branching.
4. Extend repo-state updates to only persist dependencies after the branch materializes successfully.
5. Add tests covering: parent present, parent missing, Jira flow creating a branch, and GitHub flow with parent overrides.

**Deliverables.** Updated branching helper, CLI logic, and integration tests in `tests/` that assert the new branch points at the parent tip when provided.

---

## P0 — Use upstream PR data when creating branches from GitHub

**Problem.** `create_branch_from_github_pr` generates a new branch from the current HEAD and merely names it after the PR, ignoring the PR's actual head commit. This breaks expectations when hopping onto review branches because the new branch does not contain the PR changes. 【F:twig-cli/src/cli/switch.rs†L552-L597】

**Value.** Allows reviewers to jump straight into the PR context with the correct commits and upstream tracking information. Also enables repeatable re-sync (`git pull`) without manual fetch/fetch-ref gymnastics.

**Plan.**

1. Extend `twig-gh` to expose helpers for fetching PR head refs (e.g., via `pulls/{id}` or GraphQL) and optionally copying remote URLs.
2. Update `create_branch_from_github_pr` to:
   - Fetch the PR's head ref into `refs/remotes/twig/pr/<id>` (support forks by honoring the PR head repository URL).
   - Create a local branch that tracks the fetched ref.
   - Optionally configure upstream tracking (so `git status` and `git pull` work out-of-the-box).
3. Handle forks where the PR head repo differs from `origin` by adding temporary remotes or using `git fetch <url> <ref>`.
4. Persist GitHub metadata exactly as today after creating the branch.
5. Write integration coverage using `twig-test-utils` to simulate a repository with a fake PR payload and assert that the branch tip matches the PR head SHA.

**Deliverables.** Updated GitHub client, enhanced switch flow, tests verifying SHA alignment, and documentation snippets in `README`/`USER_STORIES` if needed.

---

## P1 — Enrich `twig tree` with branch health signals

**Problem.** The tree command displays dependency structure but omits actionable status like ahead/behind counts, stale branches, or linked PR/Jira states. Users must pivot to separate commands to understand which branches need action, limiting the tree's usefulness as a situational dashboard. 【F:twig-cli/src/cli/tree.rs†L1-L116】【F:twig-core/src/tree_renderer.rs†L1-L120】

**Value.** Surface key health indicators alongside the topology so users can decide what to rebase, prune, or update without leaving the tree view.

**Plan.**

1. Gather Git status per branch (ahead/behind versus upstream, last commit age) and merge it into `BranchNode` metadata.
2. Augment the renderer to append inline badges (e.g., `↑2 ↓1`, `stale 5d`, `PR #123`) with color-coded severity.
3. Respect `--no-color` and terminal width constraints by truncating or stacking metadata lines when necessary.
4. Provide an opt-out flag (e.g., `--simple`) to keep current behavior for scripts.
5. Add unit tests for formatting and integration tests validating output snapshots using `insta`.

**Deliverables.** Extended branch analysis utilities, renderer updates, CLI flag handling, and snapshots demonstrating the enriched output.

---

## P1 — Add cascade preflight & progress reporting

**Problem.** `twig cascade` proceeds immediately, even if the working tree is dirty, dependencies are missing, or a branch will be skipped. Users only discover issues mid-run, sometimes after partially rebasing. Additionally, progress output is minimal, making it hard to monitor long cascades. 【F:twig-cli/src/cli/cascade.rs†L20-L205】【F:twig-cli/src/cli/cascade.rs†L360-L520】

**Value.** Fail fast on invalid setups and provide better visibility into rebase progress, reducing recovery time from half-completed cascades.

**Plan.**

1. Introduce a `preflight` phase:
   - Verify a clean working tree (unless `--autostash`).
   - Check that every branch in the dependency chain exists locally and has reachable parents.
   - Summarize planned rebase order and prompt for confirmation.
2. Capture per-branch timings and emit progress updates (e.g., `Rebased 3/8: feature/foo`).
3. When conflicts abort the cascade, surface a resume hint (`twig cascade --resume`) by writing interim state to `.twig/state.json`.
4. Add structured logging/tracing for verbose runs, aligning with the existing tracing setup.
5. Expand tests around graph ordering and new CLI flags; mock git command execution in unit tests for determinism.

**Deliverables.** Enhanced cascade workflow, optional confirmation/resume flags, updated tests ensuring preflight guards and progress output.

---

## P2 — Improve switch input resolution & guidance

**Problem.** When `twig switch` cannot resolve an input (typoed Jira key, missing branch), the user only receives a generic error. There is no discovery workflow to suggest nearby matches or show known Jira associations, despite the registry already tracking branch metadata. 【F:twig-cli/src/cli/switch.rs†L94-L166】【F:twig-cli/src/cli/switch.rs†L320-L371】

**Value.** Smooths the primary entry point into Twig by catching input mistakes early and offering recovery paths, reducing time spent spelunking through `twig tree` or `git branch` listings.

**Plan.**

1. Extend input detection to gather candidate Jira issues (using parser normalization) and branch names (fuzzy match via `skim`/`fuzzy-matcher`).
2. When detection fails, launch an interactive selector listing:
   - Closest Jira keys from registry metadata.
   - Branches with similar names.
   - Recent branches sorted by activity.
3. Provide a non-interactive fallback (`--no-interactive`) that prints suggestions instead of prompting.
4. Cache the last successful selection to help subsequent invocations (store in repo state).
5. Add unit tests for suggestion ranking and CLI snapshot tests (interactive prompt text) guarded by feature flags.

**Deliverables.** Updated switch command UX, suggestion engine utilities, tests validating suggestion lists, and documentation updates in command help text.
