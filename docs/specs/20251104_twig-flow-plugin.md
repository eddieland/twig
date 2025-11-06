# 20251104 Twig Flow Plugin Specification

## Purpose

- Define the architecture and feature scope for `twig flow`, a Rust-based plugin that serves as the canonical example for building Twig plugins.
- Capture how the plugin will provide argit-like branch tree visualization and branch switching workflows, aligning with existing Twig CLI conventions.
- Enumerate integration points and reusable components that must live in core crates (not `twig-cli`), ensuring long-term maintainability and reuse.
- Highlight discovery, documentation, and testing tasks required before implementation so that future agents can execute efficiently.

## Guiding Constraints

- The plugin ships out-of-tree relative to the default Twig binary; it is built within this repository but not bundled by default.
- Shared functionality (branch graph modeling, navigation helpers, shared Git operations) belongs in `twig-core` or another reusable crate, not `twig-cli`.
- Plugin should use existing Twig plugin infrastructure (registration, discovery, execution) without bespoke hacks.
- `twig flow` must mirror the ergonomics of `argit` for branch tree browsing and switching but respect Twig UX conventions (output helpers, logging, configuration loading).
- Command should be resilient when no Git repository or Twig registry exists, providing actionable error messaging.
- Support for Jira ticket-based branch lookup must reuse existing Twig core helpers for parsing ticket keys and metadata.
- Keep IO dependencies minimal; avoid introducing heavy crates unless necessary (prefer existing Git abstraction within project or lightweight wrappers).
- Treat the implementation as a flagship example: intentionally over-document and over-polish the plugin so future authors can follow it as the canonical pattern.

## Target Capabilities

1. **Branch Tree Visualization (`twig flow`)**
   - Render the current repository's branch graph (local branches) similar to argit's tree view, highlighting current branch and parent-child relationships.
  - Provide flags for choosing the root of the visualization (`--root`) and limiting depth or subtree focus via `--parent` semantics, automatically checking out the resolved branch before rendering the tree. These tree-selection flags are mutually exclusive and the CLI should surface a clear error when multiple are supplied.
   - Integrate with Twig output styling, optionally using ASCII/Unicode connectors consistent with CLI guidelines.

2. **Branch Switching (`twig flow <target>`)
   - Accept a positional argument that mirrors `twig switch` semantics: switch to existing branch, create new branch, or resolve via Jira ticket.
   - Reuse shared branch resolution logic extracted into core modules so `twig switch` and `twig flow` share behavior.
   - Support dry-run/confirmation flows if required by existing `twig switch` UX.

3. **Plugin Example & Documentation**
   - Provide comprehensive inline documentation and accompanying README snippet describing plugin structure, intended as canonical reference for plugin authors.
   - Include integration tests demonstrating plugin invocation, branch tree output snapshot(s), and branch switching behavior.

4. **Configuration & Extensibility Hooks**
   - Respect Twig configuration directories for storing plugin-specific settings (if any) via `twig-core::config` APIs.
   - Expose extension points for future enhancements (e.g., filtering branches, including remote branches, coloring, Jira metadata overlays).

## Context & Existing Assets

- Existing commands (`twig switch`, `twig git tree`, etc.) should be audited to understand existing Git abstractions; any reusable pieces must migrate into `twig-core` or a new shared crate (e.g., `twig-git`).
- Plugin discovery currently lives in `twig-cli/src/plugin.rs`; confirm how optional plugins are compiled and invoked, ensuring `twig flow` follows the same structure.
- Jira integration logic is located in `twig-core` / `twig-jira`; branch naming conventions for tickets should align with existing parsing utilities.
- `MIGRATING_FROM_ARGIT.md` may contain workflows similar to `argit`; leverage to match feature parity.

## Pre-Implementation Research Targets

- **Branch Graph Rendering Inspirations**: Review argit's output examples (local installation or documentation) to capture tree layout conventions (e.g., connectors, depth limits, color usage).
- **Existing Twig Git Helpers**: Map all functions currently handling branch enumeration, checkout, and creation inside `twig-cli` to determine what migrates into `twig-core`.
- **Plugin Lifecycle Hooks**: Understand how optional plugins are enabled/disabled at runtime, including manifest updates, build feature flags, and distribution packaging.
- **State & Config Interaction**: Inspect how Twig stores branch metadata today (if any) and whether additional schema updates are required.
- **Jira & GitHub Dependencies**: Identify any existing abstractions that translate Jira tickets to branch names to avoid duplication.
- **Testing Infrastructure**: Evaluate `twig-test-utils` for creating temporary Git repositories and how to extend them for complex branch graphs.

## Architecture Overview

### High-Level Flow

1. **Command Dispatch**
   - When user runs `twig flow`, plugin entrypoint is invoked.
   - CLI arguments parsed using plugin-specific Clap definitions (likely via `twig-core` plugin support macros or manual Clap integration).

2. **Mode Selection**
   - No positional argument → branch tree visualization mode.
   - Positional argument provided → branch switching mode, deferring to shared switch engine.

3. **Core Services**
   - Git repository inspection using shared Git service (to be extracted from existing CLI or added to `twig-core`).
   - Branch metadata model representing parent-child relationships, derived from commits or stored Twig state.
   - Config & state retrieval via `twig_core::config::ConfigDirs` and `twig_core::state::Registry` if needed.

4. **Output**
   - Structured output through `twig_core::output` helpers for success/info/warning messages.
   - Provide colorized tree if terminal supports; otherwise degrade gracefully.

### Modules & Responsibilities

| Module | Responsibility | Notes |
| --- | --- | --- |
| `plugins/twig-flow/src/lib.rs` | Plugin registration, Clap integration, high-level routing | Should mirror other plugin examples. |
| `plugins/twig-flow/src/tree.rs` | Branch graph construction, formatting, rendering | Contains logic for `--root` and `--parent` filters. |
| `plugins/twig-flow/src/switch.rs` | Branch resolution & switching interface | Delegates to shared core functions extracted from `twig switch`. |
| `twig-core/src/git/graph.rs` (new) | Core branch graph utilities (commit traversal, ancestry) | Reusable for other commands needing branch topology. |
| `twig-core/src/git/switch.rs` (refactor) | Common branch switch engine | Used by both CLI and plugin. |
| `docs/plugins/twig-flow.md` (proposed) | Human-readable plugin guide | Ensures canonical example status. |

## Data & Domain Modeling

- **Branch Node**
  - Fields: `name`, `commit_id`, `parent`, `children`, `is_current`, optional metadata (Jira ticket key, last updated).
  - Should support building from Git references; consider caching in Twig repo state for performance.
- **Tree Rendering Options**
  - `root`: branch name or commit hash to anchor tree.
  - `parent`: immediate ancestor filter; mutually exclusive with `root`; providing both (or multiple tree-selection flags) is an error that Clap must detect and report before any checkout occurs.
  - `max_depth` (potential future option) for limiting traversal.
- **Switch Request**
  - Input variants: explicit branch name, Jira ticket key, create-new flag.
  - Output: operation performed (checkout existing, create+checkout, aborted) plus messages.

## CLI Behavior Specifications

### `twig flow`

- Default mode lists branch tree.
- Flags:
  - `--root <branch>`: switch to the target branch, then show the tree rooted at that branch.
  - `--parent <branch>`: switch to the selected parent branch before rendering its subtree (e.g., to view siblings or direct descendants).
  - Tree-selection flags (`--root`, `--parent`, future variants) belong to a Clap `ArgGroup` so that specifying more than one surfaces an immediate error and prevents any checkout side effects.
  - `--show-remotes`: future extension; note in backlog.
  - `--format json`: optional future; not in initial scope unless easy to provide.
- Output should highlight current branch (e.g., `* main`).
- Should handle no branches scenario (empty repo) gracefully.

### `twig flow <target>`

- Accepts same target syntax as `twig switch` (branch name, `owner/branch`, Jira key, `feature/foo`).
- Reuses `twig switch` fallback rules (prompt to create branch if missing, apply naming templates for Jira keys).
- Accept plugin-specific options if needed (e.g., `--no-track`).

## Subagent Execution Plan

### Task Backlog

| Priority | Task | Definition of Done | Notes | Status |
| -------- | ---- | ------------------ | ----- | ------ |
| P0 | Audit existing Twig Git/branch utilities and document reusable components. | Summary document listing candidate functions/types and proposed extraction path. | Focus on `twig-cli/src/git.rs`, `twig-cli/src/cli/git.rs`, `twig-core` modules. | ✅ Completed – see "Git Utility Audit" section |
| P0 | Define plugin crate scaffolding & build integration. | Plugin compiles as optional crate with minimal main function & Clap wiring. | Determine placement under `plugins/` or `twig-flow/`. Update workspace manifests. | ✅ Completed – plugin crate scaffolded under `plugins/twig-flow` |
| P0 | Design branch graph data structures in `twig-core`. | Spec and initial interfaces ready for implementation. | Consider performance implications for large repos. | ✅ Completed – branch graph domain models and builder scaffolding added under `twig-core/src/git/graph.rs` |
| P0 | Specify branch switching shared service API. | Interface defined so CLI + plugin share same code path. | Identify behavior parity with `twig switch`. | ✅ Completed – request/response types and planner trait added to `twig-core/src/git/switch.rs` |
| P1 | Draft CLI UX for tree visualization (mock outputs). | Example outputs stored in spec or doc, capturing formatting rules. | Use ascii art similar to argit; gather from MIGRATING doc. | |
| P1 | Plan integration tests & fixtures. | List of test scenarios with coverage goals. | Include tree rendering snapshots, switching success/error cases. | |
| P1 | Outline documentation deliverables. | ToC for plugin README/tutorial. | Ensure canonical example requirement met. | |
| P2 | Investigate caching strategies for large repos. | Determine if caching needed; propose approach. | Could use `.twig` state file. | |
| P2 | Explore remote branch visualization options. | Document feasibility and requirements. | Possibly post-v1 scope. | |
| P3 | Consider GUI/TUI enhancements for future roadmap. | High-level ideas only. | Not in initial release. | |

### Risks & Mitigations

- **Risk:** Branch graph traversal may be expensive on large repos. **Mitigation:** Limit traversal depth by default; leverage Git's commit graph or caching.
- **Risk:** Divergence between `twig switch` and plugin switching logic if not centralized. **Mitigation:** Extract shared service into core crate with comprehensive tests.
- **Risk:** Plugin discovery/build integration complexity. **Mitigation:** Review existing plugin build pipeline, add documentation, create example manifest updates.
- **Risk:** Jira ticket resolution may require network access or config; plugin must gracefully handle missing config. **Mitigation:** Reuse existing Jira client initialization with clear error messages and offline fallback.
- **Risk:** Maintaining canonical example status requires up-to-date docs. **Mitigation:** Add automated doc tests or CI check referencing plugin README.

### Open Questions

- Should branch tree visualization include remote branches or only local by default?
- Do we need additional flags for sorting (e.g., last commit date) or filtering (e.g., only feature branches)?
- Should plugin support interactive mode (e.g., select branch via fuzzy finder) or remain non-interactive initially?
- How will plugin be packaged/released relative to main Twig binaries (Cargo feature, separate crate)?
- Are there security considerations for executing plugin commands that mutate branches (prompt confirmations)?

## Git Utility Audit

### Scope & Method

- Reviewed existing Git- and branch-centric modules under `twig-cli/src/git.rs`, `twig-cli/src/cli/switch.rs`, `twig-cli/src/cli/tree.rs`,
  `twig-cli/src/user_defined_dependency_resolver.rs`, and relevant helpers in `twig-core` (`git.rs`, `state.rs`, `tree_renderer.rs`).
- Focused on code paths that the forthcoming `twig flow` plugin must reuse or refactor: branch enumeration, graph construction, switching,
  and metadata persistence.

### Candidate Extractions & Reuse Targets

| Capability | Current Location | Extraction/Reuse Proposal | Notes for Implementation |
| --- | --- | --- | --- |
| Repository detection & branch lookup helpers | `twig-core/src/git.rs` (`detect_repository`, `get_local_branches`, `checkout_branch`) | Reuse as-is within plugin crate to avoid duplicate discovery logic. | Already plugin-friendly; expose via new helper module in plugin crate. |
| Registry interactions & stale-branch analytics | `twig-cli/src/git.rs` (`find_stale_branches_internal`, `StaleBranchInfo`) | Extract pure data-gathering pieces into `twig-core::git::stale` module for reuse when annotating branch graphs. | Separate user prompts/printing before moving logic. 【F:twig-cli/src/git.rs†L216-L391】【F:twig-cli/src/git.rs†L400-L579】 |
| Branch dependency visualization | `twig-cli/src/cli/tree.rs`, `twig-cli/src/user_defined_dependency_resolver.rs`, `twig-core/src/tree_renderer.rs` | Promote resolver into shared module that can coexist with commit-graph builder; keep renderer in `twig-core` but allow plugin to provide alternative data source. | Tree renderer already in core; resolver currently CLI-bound and should move alongside new graph utilities. 【F:twig-cli/src/cli/tree.rs†L1-L118】【F:twig-cli/src/user_defined_dependency_resolver.rs†L1-L200】 |
| Branch switching engine | `twig-cli/src/cli/switch.rs` (`handle_branch_switch`, `resolve_branch_base`, `try_checkout_remote_branch`, `create_and_switch_to_branch`, Jira/GitHub association helpers) | Extract branch resolution + mutation logic into `twig-core::git::switch` with IO-free API returning structured results. CLI and plugin add UX messaging on top. | Requires splitting out network client wiring from pure branch logic; RepoState interactions already in core crate. 【F:twig-cli/src/cli/switch.rs†L84-L720】 |
| Branch metadata persistence | `twig-core/src/state.rs` (`RepoState`, `BranchMetadata`, dependency helpers) | Reuse directly; ensure new graph module queries indices rather than reimplementing. | Provide lightweight facade for plugin consumption. 【F:twig-core/src/state.rs†L173-L276】 |
| Jira/GitHub association storage | `twig-cli/src/cli/switch.rs` (`store_jira_association`, `store_github_pr_association`) | Move into `twig-core::state` extension helpers returning `Result<()>` without printing. CLI/plugin can wrap for messaging. | Harmonize timestamp handling and deduplicate metadata writes. 【F:twig-cli/src/cli/switch.rs†L700-L764】 |

### Gaps Identified

- No shared commit-graph builder exists; need new `twig-core::git::graph` module that enumerates local branches (likely via `git2::Repository::branches`) and derives parent/child edges from merge bases or branch configuration.
- Switch workflow intermixes user messaging with side effects; refactor into a service struct returning an enum (`Switched`, `Created`, `RemoteTracked`, etc.) so plugin can render custom output.
- Stale branch analytics currently operate inline with user prompts; isolating data collection will enable tree overlays (e.g., highlight stale branches in visualization).
- User-defined dependency resolver is CLI-scoped; to support plugin overlays, migrate logic beside new graph module and expose a trait (`BranchTopologyProvider`) for pluggable sources (user-defined vs. git-derived).

## Status Tracking (to be updated by subagent)

- **Current focus:** _Draft CLI UX for tree visualization (mock outputs)_
- **Latest completed task:** _Specify branch switching shared service API_
- **Next up:** _Plan integration tests & fixtures_

## Lessons Learned (ongoing)

- Existing switch workflow tightly couples side effects with messaging; future extractions must return structured outcomes so multiple callers (CLI, plugins) can share logic without duplicating UX code.
- Establishing canonical request/response structs up front prevents the CLI refactor from blocking plugin development because both surfaces can be wired incrementally.
- Establishing the plugin as its own workspace member clarifies dependency wiring early and keeps cargo metadata accurate for future integration tests.
- Separating branch topology from annotations in the core graph types will let the CLI and plugin compose their own overlays without re-traversing Git data.
- Introducing an explicit `BranchKind` (local/remote/virtual) enum in the graph models keeps downstream consumers from guessing at node semantics.
- Collapsing edge variants into a single `BranchEdge` simplifies the model and leaves room for higher-level layers to interpret relationships as needed.
