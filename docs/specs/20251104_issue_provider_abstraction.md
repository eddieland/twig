# Issue Provider Abstraction for Twig

## Purpose

- Introduce a first-class abstraction that decouples Twig's issue management workflows from concrete provider implementations (e.g., GitHub Issues, Jira) so the CLI can evolve without duplicating logic.
- Establish shared terminology, data contracts, and configuration boundaries that enable future providers while delivering immediate GitHub Issues support.
- Non-goals: redesigning unrelated parts of Twig (e.g., Git operations), shipping a multi-provider orchestrator, or altering existing Jira functionality beyond necessary integration points.

## Guiding Constraints

- Default assumption: each repository/project is associated with at most one issue provider at a time; design should not preclude adding multi-provider support later, but we avoid optimizing for it now.
- Preserve backward compatibility for existing Jira-centric flows (commands, config paths, state files) so current users experience no regressions.
- Favor provider-agnostic domain models inside `twig-core` and `twig-cli`, pushing provider-specific logic into dedicated crates/modules (`twig-gh`, `twig-jira`, future `twig-<provider>`).
- Configuration must remain file-based (leveraging existing XDG directories) and work in offline-friendly scenarios.
- Provider abstractions must be testable without live network calls by using trait-driven interfaces and in-memory/mock implementations in `twig-test-utils`.
- Avoid blocking future asynchronous refactors: traits and runtime interactions should be async-ready, using Tokio-compatible signatures where relevant.

## Target Capabilities

1. **Unified Issue Provider Interface:** Define a trait (or trait family) capturing authentication, issue retrieval, creation, transition, commenting, and metadata queries that both Jira and GitHub implementations satisfy.
2. **Provider Registration & Selection:** Provide CLI/state mechanisms to configure the active issue provider per repository, including detection, validation, and persistence.
3. **GitHub Issues Support:** Deliver a production-ready GitHub provider that implements the abstraction, reusing `twig-gh` HTTP client code and exposing parity with existing Jira-backed commands where feasible.
4. **Shared CLI Workflows:** Update commands (e.g., `twig issue`, `twig sync`, automations) to interact through the abstraction so behavior is consistent regardless of the provider.
5. **Testing & Tooling:** Supply integration tests and fixtures covering provider selection, GitHub flows, and regression cases for Jira.
6. **Observability & Error Handling:** Ensure provider interactions emit structured errors/logs that surface actionable feedback to users.

## Architecture Overview

### Core Abstractions

- **IssueProvider trait family (new, `twig-core`)**
  - `IssueProvider` (async trait) covering lifecycle operations: authenticate, fetch issue by key, search by filters, create/update/comment, transition/close, and retrieve metadata (available states, labels, assignees).
  - `IssueReference` struct abstracting provider-specific identifiers (Jira key vs. GitHub number) with typed fields and display helpers.
  - `IssueSnapshot` domain model capturing normalized fields (title, body, status, labels/tags, assignee, estimate/story points, timestamps) with extensible `HashMap<String, Value>` for provider-specific properties.
  - `ProviderCapabilities` bitflags enumerating optional behaviors (supports transitions, supports label editing, supports custom fields, supports backlog ranking, etc.) surfaced to the CLI for conditional UX.
  - Error types consolidated into `IssueProviderError` with variants for auth, network, rate limit, validation, and unsupported operations.

- **Provider Implementations**
  - `JiraIssueProvider` (in `twig-jira`) wraps existing client; adapter converts between Jira models and normalized domain types. Introduce conversion layer to minimize churn in `twig-jira`.
  - `GithubIssueProvider` (new module in `twig-gh`) leverages REST GraphQL? (decide) API; handles pagination, label/color mapping, comment threads, closing via state transitions.
  - Each provider exposes configuration schema (e.g., Jira site URL vs. GitHub repo owner/name) via `ProviderDescriptor` trait returning metadata for CLI prompts.

- **Provider Registry & Factory**
  - Central registry within `twig-core` mapping provider identifiers (e.g., `jira`, `github`) to factory functions that produce boxed providers using runtime + credentials from `twig-cli::clients`.
  - `ProviderConfig` stored in repo-level state file `.twig/state.json` (or new `issue_provider.toml`) containing provider id plus provider-specific settings.
  - CLI helper to resolve provider: read repo config, fallback to global default, error if missing; supply `ProviderHandle` bundling runtime + provider trait object.

### Configuration & UX Changes

- **Setup Flow**
  - Extend `twig setup` or introduce `twig issue provider set` to guide selection. CLI prompts: choose provider (Jira/GitHub), gather required inputs (e.g., GitHub owner/repo), validate via test API call, persist config.
  - Provide `twig issue provider show` for introspection and `twig issue provider clear` for removal.

- **State Files**
  - Add new schema version to repo config to record provider choice; include migration path for existing repos defaulting to Jira (auto-detected by presence of Jira config).
  - Global config may store last-used provider to streamline new repo onboarding.

- **Command Routing**
  - Update `twig-cli/src/cli` commands to request provider handles via `ProviderContext` object that caches results per invocation and manages runtime lifetimes.
  - Commands must inspect `ProviderCapabilities` before invoking provider-specific features; degrade gracefully (e.g., warn if GitHub lacks concept of story points).

### Data Flow

1. CLI command resolves repo path and loads repo state.
2. `ProviderRegistry` instantiates provider using saved configuration and credentials (netrc tokens).
3. Command interacts with provider via trait methods, receiving normalized domain models.
4. Output layer renders provider-neutral information, optionally augmenting with provider-specific metadata (flag-controlled).
5. Any updates (create/comment/transition) propagate through provider, with errors mapped to human-readable messages.

### Testing Strategy

- Unit tests for trait implementations using mocked HTTP clients (existing pattern in `twig-jira`, extend to `twig-gh`).
- Integration tests within `twig-cli` using `twig-test-utils` to simulate repo config and verifying CLI output for both providers.
- Contract tests ensuring parity: define behavior-driven scenarios executed against both Jira and GitHub implementations to confirm shared expectations.
- Snapshot tests for CLI output to detect regressions in formatting or messaging.

### Documentation & Developer Experience

- Update `docs/` with provider overview, configuration examples, troubleshooting, and migration guidance.
- Add inline Rustdoc on traits/structs describing semantics and provider-specific nuances.
- Provide ADR or spec appendix (if needed) capturing rationale for single-provider-per-repo assumption and future multi-provider path.

## Subagent Execution Plan

The following backlog is prioritized for a single subagent (or small group) to implement iteratively. Update the _Status_ and _Lessons Learned_ sections while working.

### Task Backlog

| Priority | Task | Definition of Done | Notes | Status |
| -------- | ---- | ------------------ | ----- | ------ |
| P0 | Audit current issue-related flows and document touchpoints needing abstraction | Inventory commands/modules, map dependencies on Jira models, and produce diagram of current state | Covers `twig-cli/src/cli/jira.rs`, `twig-cli/src/jira`, `twig-core` state config | |
| P0 | Design and codify provider trait(s) and domain models in `twig-core` | Trait definitions merged with documentation/tests, Jira implementation behind feature flag | Ensure async compatibility; consider capability flags | |
| P0 | Introduce provider selection configuration | Repo-level config schema updated, migration path defined, CLI UX specified | Include detection/validation logic and error messaging | |
| P0 | Implement Jira provider adapter conforming to new traits | Existing functionality ported without regressions, integration tests pass | Provide shims for legacy commands until deprecated | |
| P0 | Implement GitHub Issues provider using `twig-gh` | Supports required operations with tests (unit + integration) | Handle authentication, pagination, rate-limits | |
| P1 | Update CLI commands to use provider abstraction | Commands compile against trait objects, manual QA confirms parity | Include new help/docs entries | |
| P1 | Add testing infrastructure and mocks | `twig-test-utils` updated, new fixtures for GitHub API responses | Cover offline tests and error cases | |
| P2 | Provide migration tooling/documentation | `docs/` updated, release notes prepared, sample configs provided | Possibly auto-migrate repo configs | |
| P2 | Explore multi-provider roadmap hooks | Document extension points, add TODOs/tests verifying ability to register multiple providers later | Keep code minimal but future-proof | |
| P3 | Telemetry/metrics for provider usage (if desired) | Optional analytics wiring behind flag | Coordinate with broader observability roadmap | |

### Risks & Mitigations

- **Risk:** Trait abstraction obscures provider-specific capabilities (e.g., Jira transitions vs. GitHub labels). **Mitigation:** Introduce capability flags or extension traits, document provider differences, and design CLI commands to degrade gracefully.
- **Risk:** Regression in existing Jira workflows during refactor. **Mitigation:** Add comprehensive integration tests before migrating, run comparison snapshots, and roll out behind feature flags.
- **Risk:** GitHub API rate limits or authentication edge cases disrupt workflows. **Mitigation:** Cache responses when feasible, surface warnings, and allow configurable retry/backoff.
- **Risk:** Configuration complexity confuses users switching providers. **Mitigation:** Provide guided CLI wizard, clear error messages, and documentation updates.

### Open Questions

- Should provider selection live in repo-level config, global registry, or both to support monorepos spanning multiple providers?
- Do we need command-level overrides (e.g., forcing Jira for specific operations) or is repo-level scoping sufficient?
- How do we reconcile provider-specific metadata (e.g., Jira issue types vs. GitHub labels) in shared commands?
- What is the minimum viable subset of GitHub Issue operations required for parity with Jira-powered automations?

## Status Tracking (to be updated by subagent)

- **Current focus:** _Not yet started (awaiting implementation kickoff)._ 
- **Latest completed task:** _None._
- **Next up:** _Audit current issue-related flows and document touchpoints needing abstraction._

## Lessons Learned (ongoing)

- _To be populated during execution._
