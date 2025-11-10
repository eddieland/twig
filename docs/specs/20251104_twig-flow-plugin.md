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

   - Render the current repository's branch graph (local branches) using a hybrid tree-and-table layout: the first line prints column headers (e.g., `Branch`, `Story`, `PR`, `Notes`) while each branch row retains tree connectors inside the `Branch` column and keeps the additional metadata columns horizontally aligned.
   - The renderer must live in `twig-core` so it can be reused by the CLI, plugins, and future tooling; `twig flow` consumes this shared component rather than owning bespoke formatting code.

   - Provide boolean flags (`--root`, `--parent`) that perform an automatic checkout before rendering: `--root` moves the user to the configured root branch for the graph, while `--parent` switches to the current branch's primary parent. The visualization still renders the full tree, simply highlighting the new current branch. These tree-selection flags are mutually exclusive and the CLI should surface a clear error when multiple are supplied.
   - Integrate with Twig output styling, optionally using ASCII/Unicode connectors consistent with CLI guidelines, and ensure spacing remains column-aligned even when connectors are present.
   - Support an internally-configurable column schema so future UX iterations can add or remove columns (story, PR, lifecycle notes, etc.) without rewriting the renderer. The configuration remains hidden from end users for now but should be easy to expose later.

2. **Branch Switching (`twig flow <target>`)**

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

| Module                                   | Responsibility                                            | Notes                                                            |
| ---------------------------------------- | --------------------------------------------------------- | ---------------------------------------------------------------- |
| `plugins/twig-flow/src/lib.rs`           | Plugin registration, Clap integration, high-level routing | Should mirror other plugin examples.                             |
| `plugins/twig-flow/src/tree.rs`          | Branch graph construction, formatting, rendering          | Contains logic for `--root` and `--parent` filters.              |
| `plugins/twig-flow/src/switch.rs`        | Branch resolution & switching interface                   | Delegates to shared core functions extracted from `twig switch`. |
| `twig-core/src/git/graph.rs` (new)       | Core branch graph utilities (commit traversal, ancestry)  | Reusable for other commands needing branch topology.             |
| `twig-core/src/git/switch.rs` (refactor) | Common branch switch engine                               | Used by both CLI and plugin.                                     |
| `docs/plugins/twig-flow.md` (proposed)   | Human-readable plugin guide                               | Ensures canonical example status.                                |

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
  - `--root`: switch to the repository's configured root branch (e.g., `main`) and then render the full tree with that branch highlighted.
  - `--parent`: switch to the current branch's primary parent (if known) before rendering; acts as a shortcut for `twig switch` to the parent while still displaying the entire tree. If multiple parents are detected, emit an error listing the options and skip rendering (future enhancement: interactive selection/dialog).
  - Tree-selection flags (`--root`, `--parent`, future variants) belong to a Clap `ArgGroup` so that specifying more than one surfaces an immediate error and prevents any checkout side effects.
- `--show-remotes`: future extension; note in backlog.
- `--format json`: optional future; not in initial scope unless easy to provide.
- Output should highlight current branch (e.g., `* main`) while preserving column alignment under a shared header row.
- Header row defaults to `Branch`, `Story`, `PR`, `Notes`; renderer reads from a configurable (internal) schema so layout changes do not require code rewrites.
- Should handle no branches scenario (empty repo) gracefully.

### `twig flow <target>`

- Accepts same target syntax as `twig switch` (branch name, `owner/branch`, Jira key, `feature/foo`).
- Reuses `twig switch` fallback rules (prompt to create branch if missing, apply naming templates for Jira keys).
- Accept plugin-specific options if needed (e.g., `--no-track`).

## Renderer Component Scope

- Location: `twig-core/src/git/renderer.rs` (new module re-exported via `twig_core::git`), exposing a `BranchTableRenderer` struct that accepts a `BranchGraph`, column schema, and style configuration.
- API surface:
  - `BranchTableColumn` enum describing built-in columns (`Branch`, `Story`, `PR`, `Notes`) plus extensibility via custom metadata keys.
  - `BranchTableSchema` to configure column order, width behavior, and fallback placeholders.
  - `BranchTableRenderer::render(&self, graph: &BranchGraph, root: &BranchName, writer: impl Write)` returning `Result<()>`.
  - Optional helpers for width measurement, column alignment (leveraging `console::strip_ansi_codes` / internal width utilities).
- Responsibilities:
  - Compose tree connectors within the `Branch` column using `BranchGraph` topology (children, parents, current branch).
  - Populate additional columns from `BranchNodeMetadata` annotations/labels (e.g., Jira ticket, PR number, stale state).
  - Provide deterministic spacing suitable for snapshot testing; avoid terminal-dependent width detection.
  - Support internal configuration toggles (e.g., hidden feature flag to swap columns) without exposing user-facing CLI flags yet.
- Testing strategy (lives under `twig-core/src/git/renderer/tests` or `tests/renderer.rs`):
  - Unit tests covering width calculations, connector generation, and column placeholder logic.
  - Snapshot tests (via `insta`) for representative branch graphs: simple tree, deep nesting, multiple metadata combinations, detached HEAD / empty graphs.
  - Fixtures built with `twig-test-utils` to synthesise `BranchGraph` instances; avoid hitting real git repositories for renderer tests.
- Integration path:
  - `twig flow` plugin depends on the renderer after it is stabilized; plugin-specific formatting replaces only the high-level messaging.

### Renderer API & Column Schema

| Type                       | Responsibility                                                                                               | Notes                                                                                                                                                                                     |
| -------------------------- | ------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `BranchTableColumnKind`    | Encodes how a cell is populated (`Branch`, `FirstLabel`, `Annotation { key }`, `Notes`).                      | Annotation columns point at arbitrary metadata keys so future columns can be added without touching the renderer core.                                                                   |
| `BranchTableColumn`        | Couples a human-readable title with a `kind` and `min_width`.                                                | Convenience constructors (`branch()`, `story()`, `pull_request()`, `notes()`) seed the default schema; `with_min_width` lets internal callers tune width guarantees per column.          |
| `BranchTableSchema`        | Ordered list of columns plus presentation toggles (placeholder text, column spacing, header visibility).     | Defaults to `Branch/Story/PR/Notes`, placeholder `--`, spacing of two spaces, header enabled. Schema builders will live in `twig-core` so plugins/CLI can tweak layout before rendering. |
| `BranchTableRenderError`   | Error enum for empty schemas, branch-column validation, unknown branches, and `fmt::Error` passthrough.      | Keeps rendering failures distinct from Git/IO errors so callers can handle them deterministically (e.g., fall back to plain `twig tree`).                                                |
| `BranchTableRenderer`      | Stateful renderer that owns a `BranchTableSchema` and knows how to format a `BranchGraph` into any writer.   | Entry point: `render(&mut writer, &graph, &BranchName)`; returns `Result<(), BranchTableRenderError>`.                                                                                   |

**Default schema**

| Column  | Kind                    | Source                                                                 | Placeholder |
| ------- | ----------------------- | ---------------------------------------------------------------------- | ----------- |
| Branch  | `Branch`                | Graph topology + metadata (`is_current`) to add `*`, tree connectors.  | n/a         |
| Story   | `FirstLabel`            | First label from `BranchNodeMetadata.labels`.                          | `--`        |
| PR      | `Annotation { key }`    | `twig.pr` annotation (text/number).                                    | `--`        |
| Notes   | `Notes`                 | Prefers `twig.notes` annotation, falls back to `BranchStaleState`.     | `--`        |

Hidden configuration will reuse `BranchTableSchema` builders: callers can clone the default schema, swap/insert columns, adjust spacing, or suppress the header before passing it into the renderer. These overrides stay internal for now (toggled via config files under `.twig/`), but the data model fully supports surfacing them later without rewiring the renderer.

```rust
use twig_core::git::{BranchGraph, BranchName, BranchTableRenderer, BranchTableSchema};

fn render_flow(graph: &BranchGraph, root: &BranchName) -> anyhow::Result<String> {
  let schema = BranchTableSchema::default()
    .with_placeholder("—")
    .with_column_spacing(3);
  let mut output = String::new();
  BranchTableRenderer::new(schema).render(&mut output, graph, root)?;
  Ok(output)
}
```

_Module skeleton:_ `twig-core/src/git/renderer.rs` defines all structs/enums listed above, re-exports them via `twig_core::git`, and provides TODO stubs for future styling hooks (color, Unicode vs ASCII connectors). An empty schema now triggers a dedicated error so downstream callers receive actionable feedback during early integration work.

## CLI UX Mockups

- Tree visualization uses ASCII connectors (`├─`, `└─`, `│`) within the `Branch` column and prefixes the currently checked-out branch with `*`.
- A header row introduces the default columns (`Branch`, `Story`, `PR`, `Notes`) and each branch row keeps metadata horizontally aligned under the header. Missing metadata renders as `—` to preserve alignment.
- Branch annotations migrate out of inline square brackets and into the dedicated columns; colorization still leverages `twig_core::output`.
- `--root` and `--parent` trigger an explicit checkout before rendering; conflicting selections short-circuit with a Clap error prior to Git mutations. Detached-head or empty repository scenarios surface a warning followed by only the header row.

```text
$ twig flow
Branch                         Story        PR       Notes
* main                         —            —        —
├─ feature/auth-refresh        PROJ-451     —        active
│  └─ feature/auth-refresh-ui  PROJ-451     #982     in-review
├─ feature/payment-refactor    —            draft    stale 21d
│  ├─ feature/payment-api      —            —        —
│  └─ feature/payment-ui       —            —        —
└─ chore/cicd-cleanup          —            —        —
   └─ fix/gha-cache            —            —        stale 45d
```

```text
$ twig flow --root
Switched to branch "feature/payment-refactor" (root)
Branch                         Story        PR       Notes
* feature/payment-refactor     —            draft    —
├─ feature/payment-api         —            —        —
└─ feature/payment-ui          —            —        —
```

```text
$ twig flow --parent
Switched to parent branch "feature/auth-refresh"
Branch                         Story        PR       Notes
* feature/auth-refresh         PROJ-451     —        —
└─ feature/auth-refresh-ui     PROJ-451     #982     in-review
```

```text
$ twig flow --root --parent
error: the argument '--root' cannot be used with '--parent'
```

## Documentation Deliverables Outline

### Plugin README (plugins/twig-flow/README.md)

- Purpose & positioning: describe how `twig flow` complements the base Twig CLI and when to rely on it instead of `twig switch` / `twig tree`.
- Installation & upgrade: enumerate Cargo workspace feature flags, `cargo install --path plugins/twig-flow`, and how Twig locates plugin binaries.
- Quickstart commands: provide copy/paste snippets for `twig flow`, `twig flow --root`, `twig flow --parent`, and `twig flow <target>` paired with annotated output.
- Architecture overview: explain the separation between `twig-core` (graph + renderer + switch service) and the plugin crate (Clap parsing, IO orchestration).
- Configuration & prerequisites: call out required Twig config dirs, `.netrc` expectations, and how to opt into alternate column schemas while they remain hidden flags.
- Troubleshooting & FAQ: document likely error states (missing repo, detached HEAD, Jira config absent) with remediation steps that mirror CLI messaging.
- Contributing & testing: outline `make fmt`, `make test` / `cargo nextest`, snapshot update workflow, and integration-test locations for future contributors.

### Tutorial / Walkthrough (docs/plugins/twig-flow.md)

- Scenario narrative that walks through viewing a branch tree, switching via Jira key, and interpreting metadata columns on a sample repo.
- Embedded snapshots or ASCII renderings so readers can compare their output with the canonical renderer expectations.
- Cross-links back to `twig-core` modules and plugin source files so contributors can jump from docs directly into code.

### Inline Documentation Requirements

- Rustdoc coverage for every public type/function in `twig-core::git::{graph,renderer}` and the plugin's CLI entrypoints, including `# Errors`/`# Examples` sections where clarity is needed.
- Example snippets showing how to construct a `BranchTableSchema`, customize columns, and invoke the shared switch service without the plugin wrapper.
- Internal developer docs (module-level comments) describing invariants such as column alignment rules, connector selection, and metadata sourcing.

### Release Notes & CHANGELOG Hooks

- Plugin-scoped `CHANGELOG.md` seeded under `plugins/twig-flow/` so future UX changes have a canonical log.

## Subagent Execution Plan

### Task Backlog

| Priority | Task                                                                       | Definition of Done                                                                                   | Notes                                                                             | Status                                                                                                     |
| -------- | -------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| P0       | Audit existing Twig Git/branch utilities and document reusable components. | Summary document listing candidate functions/types and proposed extraction path.                     | Focus on `twig-cli/src/git.rs`, `twig-cli/src/cli/git.rs`, `twig-core` modules.   | ✅ Completed – see "Git Utility Audit" section                                                             |
| P0       | Define plugin crate scaffolding & build integration.                       | Plugin compiles as optional crate with minimal main function & Clap wiring.                          | Determine placement under `plugins/` or `twig-flow/`. Update workspace manifests. | ✅ Completed – plugin crate scaffolded under `plugins/twig-flow`                                           |
| P0       | Design branch graph data structures in `twig-core`.                        | Spec and initial interfaces ready for implementation.                                                | Consider performance implications for large repos.                                | ✅ Completed – branch graph domain models and builder scaffolding added under `twig-core/src/git/graph.rs` |
| P0       | Specify branch switching shared service API.                               | Interface defined so CLI + plugin share same code path.                                              | Identify behavior parity with `twig switch`.                                      | ✅ Completed – shared service API skeleton added under `twig-core/src/git/switch.rs`                       |
| P0       | Finalize renderer API & column schema.                                     | Document concrete structs/enums + default schema in spec and prepare module skeleton in `twig-core`. | Captures `BranchTableRenderer`, schema types, and metadata mapping rules.         | ✅ Completed – see "Renderer API & Column Schema" and `twig-core/src/git/renderer.rs`.                      |
| P0       | Implement renderer core in `twig-core`.                                    | Produce tree+table formatter operating on `BranchGraph` with alignment + placeholders.               | No CLI integration yet; include internal feature gate for hidden customization.   |                                                                                                            |
| P0       | Add unit & snapshot tests for renderer.                                    | Cover width calculations, connectors, and schema overrides using `insta` fixtures.                   | Lives under `twig-core` tests; uses synthetic graphs.                             |                                                                                                            |
| P0       | Handle multi-parent `--parent` edge case.                                  | Error messaging and parent listings defined; renderer call short-circuits when multiple parents.     | Future interactive selection tracked separately.                                  |                                                                                                            |
| P1       | Draft CLI UX for tree visualization (mock outputs).                        | Example outputs stored in spec or doc, capturing formatting rules.                                   | Hybrid tree/table layout with default `Branch/Story/PR/Notes` columns.            | ✅ Completed – see "CLI UX Mockups" section                                                                |
| P1       | Define internal column schema configuration.                               | Document data model + default columns for renderer with hidden config override.                      | Enables future customization without public CLI surface.                          | ⏳ Pending – unblocked by renderer API; implementation will hook config files into schema overrides.       |
| P1       | Plan integration tests & fixtures.                                         | List of test scenarios with coverage goals.                                                          | Include tree rendering snapshots, switching success/error cases.                  |                                                                                                            |
| P1       | Explore interactive parent selection UX.                                   | Outline potential dialogs/prompts for selecting among multiple parents.                              | Depends on multi-parent error groundwork.                                         |                                                                                                            |
| P1       | Outline documentation deliverables.                                        | ToC for plugin README/tutorial.                                                                      | Ensure canonical example requirement met.                                         | ✅ Completed – see "Documentation Deliverables Outline" section                                            |
| P2       | Investigate caching strategies for large repos.                            | Determine if caching needed; propose approach.                                                       | Could use `.twig` state file.                                                     |                                                                                                            |
| P2       | Explore remote branch visualization options.                               | Document feasibility and requirements.                                                               | Possibly post-v1 scope.                                                           |                                                                                                            |
| P3       | Consider GUI/TUI enhancements for future roadmap.                          | High-level ideas only.                                                                               | Not in initial release.                                                           |                                                                                                            |

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

| Capability                                     | Current Location                                                                                                                                                           | Extraction/Reuse Proposal                                                                                                                                         | Notes for Implementation                                                                                                                                                                                        |
| ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Repository detection & branch lookup helpers   | `twig-core/src/git.rs` (`detect_repository`, `get_local_branches`, `checkout_branch`)                                                                                      | Reuse as-is within plugin crate to avoid duplicate discovery logic.                                                                                               | Already plugin-friendly; expose via new helper module in plugin crate.                                                                                                                                          |
| Registry interactions & stale-branch analytics | `twig-cli/src/git.rs` (`find_stale_branches_internal`, `StaleBranchInfo`)                                                                                                  | Extract pure data-gathering pieces into `twig-core::git::stale` module for reuse when annotating branch graphs.                                                   | Separate user prompts/printing before moving logic. 【F:twig-cli/src/git.rs†L216-L391】【F:twig-cli/src/git.rs†L400-L579】                                                                                      |
| Branch dependency visualization                | `twig-cli/src/cli/tree.rs`, `twig-cli/src/user_defined_dependency_resolver.rs`, `twig-core/src/tree_renderer.rs`                                                           | Promote resolver into shared module that can coexist with commit-graph builder; keep renderer in `twig-core` but allow plugin to provide alternative data source. | Tree renderer already in core; resolver currently CLI-bound and should move alongside new graph utilities. 【F:twig-cli/src/cli/tree.rs†L1-L118】【F:twig-cli/src/user_defined_dependency_resolver.rs†L1-L200】 |
| Branch switching engine                        | `twig-cli/src/cli/switch.rs` (`handle_branch_switch`, `resolve_branch_base`, `try_checkout_remote_branch`, `create_and_switch_to_branch`, Jira/GitHub association helpers) | Extract branch resolution + mutation logic into `twig-core::git::switch` with IO-free API returning structured results. CLI and plugin add UX messaging on top.   | Requires splitting out network client wiring from pure branch logic; RepoState interactions already in core crate. 【F:twig-cli/src/cli/switch.rs†L84-L720】                                                    |
| Branch metadata persistence                    | `twig-core/src/state.rs` (`RepoState`, `BranchMetadata`, dependency helpers)                                                                                               | Reuse directly; ensure new graph module queries indices rather than reimplementing.                                                                               | Provide lightweight facade for plugin consumption. 【F:twig-core/src/state.rs†L173-L276】                                                                                                                       |
| Jira/GitHub association storage                | `twig-cli/src/cli/switch.rs` (`store_jira_association`, `store_github_pr_association`)                                                                                     | Move into `twig-core::state` extension helpers returning `Result<()>` without printing. CLI/plugin can wrap for messaging.                                        | Harmonize timestamp handling and deduplicate metadata writes. 【F:twig-cli/src/cli/switch.rs†L700-L764】                                                                                                        |

### Gaps Identified

- No shared commit-graph builder exists; need new `twig-core::git::graph` module that enumerates local branches (likely via `git2::Repository::branches`) and derives parent/child edges from merge bases or branch configuration.
- Switch workflow intermixes user messaging with side effects; refactor into a service struct returning an enum (`Switched`, `Created`, `RemoteTracked`, etc.) so plugin can render custom output.
- Stale branch analytics currently operate inline with user prompts; isolating data collection will enable tree overlays (e.g., highlight stale branches in visualization).
- User-defined dependency resolver is CLI-scoped; to support plugin overlays, migrate logic beside new graph module and expose a trait (`BranchTopologyProvider`) for pluggable sources (user-defined vs. git-derived).

## Status Tracking (to be updated by subagent)

- **Current focus:** _Plan integration tests & fixtures_
- **Latest completed task:** _Outline documentation deliverables_
- **Next up:** _Explore interactive parent selection UX_

## Lessons Learned (ongoing)

- Existing switch workflow tightly couples side effects with messaging; future extractions must return structured outcomes so multiple callers (CLI, plugins) can share logic without duplicating UX code.
- Establishing the plugin as its own workspace member clarifies dependency wiring early and keeps cargo metadata accurate for future integration tests.
- Separating branch topology from annotations in the core graph types will let the CLI and plugin compose their own overlays without re-traversing Git data.
- Introducing an explicit `BranchKind` (local/remote/virtual) enum in the graph models keeps downstream consumers from guessing at node semantics.
- Collapsing edge variants into a single `BranchEdge` simplifies the model and leaves room for higher-level layers to interpret relationships as needed.
- Defining the shared switch service API up front exposed configuration toggles (creation policy, tracking policy, dry-run) that both the CLI and plugin need to surface consistently.
- Capturing ASCII tree conventions before implementation ensures `twig flow` aligns with existing `twig tree` output and clarifies where metadata annotations should appear.
