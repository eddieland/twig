# Issue Provider Abstraction Spec

## Summary
This spec introduces an abstraction layer for "issue" providers so that Twig can coordinate with services such as GitHub Issues (initial target) and Jira (existing implementation). The abstraction will:

- Provide a unified trait/API for resolving, fetching, and mutating issues regardless of provider.
- Allow repositories to select exactly one active issue provider at a time (with minimal accommodations so the system could evolve to multi-provider in the future if demanded).
- Integrate with existing Twig state/config plumbing so branch metadata, dashboards, and automation can consume provider-agnostic issue data.

## Motivations & Goals
- **Decouple Jira-specific logic** currently embedded across `twig-cli`, `twig-core`, and `twig-jira` to unlock GitHub Issues support without duplicating workflows.
- **Enable provider-specific capabilities** (e.g., GitHub label syncing) behind a consistent command surface.
- **Support repository-level configuration** to declare the chosen provider and required credentials.
- **Maintain backward compatibility** for Jira-focused commands while providing a migration story.

## Non-Goals
- Building a multi-provider orchestration layer (e.g., concurrently using Jira and GitHub Issues) in the first iteration.
- Implementing deep provider-specific features (e.g., Jira transitions, GitHub project management). Those will be incremental follow-ups leveraging the abstraction.
- Replacing existing GitHub PR integrations (though they may reuse similar patterns later).

## Current State & Gaps
- Jira support is hard-coded through `twig-cli` modules (sync, dashboard, tree rendering) that expect Jira issue keys and data models from `twig-jira`.
- GitHub integration presently focuses on pull requests (`twig-gh`) but has no issue APIs.
- Repo state (`twig-core::state::BranchMetadata`) stores a `jira_issue: Option<String>` which is provider-specific.
- CLI commands expose Jira-centric flags (e.g., `--skip-jira`) and output emojis/columns tied to Jira.

## Proposed Architecture

### Provider Registry & Trait
- Introduce a `twig-core::issues` module that defines:
  - `IssueProviderKind` enum (e.g., `Jira`, `GitHub`) used in configs and telemetry.
  - `IssueReference` struct (provider-agnostic identifier & optional display key).
  - `IssueDetails` struct capturing shared fields (key, title, status, assignee, url, updated_at, provider-native metadata map for extensions).
  - `IssueProvider` trait exposing async methods (executed via Tokio runtime abstraction already used in `clients.rs`):
    - `fn id(&self) -> IssueProviderKind`
    - `async fn resolve_issue(&self, reference: &IssueReference) -> Result<IssueDetails>`
    - `async fn search_issues(&self, query: IssueQuery) -> Result<Vec<IssueDetails>>`
    - `async fn transition_or_comment(&self, action: IssueAction) -> Result<IssueActionOutcome>` (extensible command pattern for provider-specific mutations).
  - `IssueQuery`, `IssueAction`, and `IssueActionOutcome` enums capturing the minimal cross-provider surface; richer provider hooks can extend via variants or metadata payloads.

### Provider Implementations
- **Jira**: Wrap existing `twig-jira` client in a struct implementing `IssueProvider`. Map Jira issue fields into the new `IssueDetails` schema. Translate existing transition/comment workflows into `IssueAction` handling.
- **GitHub Issues**: Add endpoints to `twig-gh` for listing, fetching, and commenting on issues. Implement `IssueProvider` by leveraging GitHub REST API v3 (authenticated via `.netrc` like PR support). Ensure pagination & rate limit headers are handled gracefully.

### Configuration Model
- Extend repo-local config (`ConfigDirs::repo_state_path` JSON) with:
  ```json
  {
    "issues": {
      "provider": "github",
      "settings": {
        "owner": "org",
        "repo": "project"
      }
    }
  }
  ```
- Update global registry schema if necessary to record default provider for new repos.
- CLI commands gain `twig repo issues set-provider --provider github --owner org --repo project` (exact UX to be detailed in follow-up CLI spec).
- **Single-provider assumption**: validation ensures only one provider block exists. Schema will support an array later (`providers: [ ... ]`) but remain unused initially.

### Client Instantiation Flow
- Enhance `twig-cli/src/clients.rs` with `create_issue_provider_runtime_and_client(config)` that returns a `TokioRuntimeGuard` and boxed `dyn IssueProvider` based on repo configuration.
- Use this factory in commands currently creating Jira runtimes (sync, dashboard, tree rendering) so they operate on provider-agnostic data.
- Handle "provider not configured" as a first-class error surfaced to the CLI with actionable guidance.

### Branch Metadata & State Changes
- Replace `BranchMetadata.jira_issue: Option<String>` with a provider-neutral representation:
  ```rust
  pub struct IssueLink {
      pub provider: IssueProviderKind,
      pub reference: IssueReference,
      pub cached: Option<IssueDetails>,
  }
  ```
- `BranchMetadata` becomes:
  ```rust
  pub issue: Option<IssueLink>;
  ```
- Migration path: when loading legacy state, populate `issue` with `provider = Jira` and `reference.key = legacy_jira_issue`.
- Update helper APIs (`get_branch_issue`, tree renderer, dashboards) to use new structure while maintaining existing output (e.g., still showing 🎫 for Jira but maybe 🐙 for GitHub Issues later).

### Command Updates
- **sync**: detection logic stays provider-specific (regex for Jira, branch naming heuristics for GitHub). Introduce trait extension points so each provider can supply detection strategies or reuse config hints. Results stored via new `IssueLink` struct.
- **dashboard/tree**: render provider name/shortcode alongside issue key. Provide fallbacks if provider lacks certain fields (e.g., GitHub status may map to label/state).
- **auto-dependency discovery**: read issue provider from metadata rather than assuming Jira when validating parent/feature relationships.

### Testing Strategy
- Unit tests for provider trait conversions (mock provider).
- Integration tests using `twig-test-utils` to simulate repo state migrations.
- HTTP client mocks for GitHub Issues similar to existing Jira mockito tests.

## Migration & Rollout
1. Introduce new core types (`IssueProviderKind`, `IssueLink`, trait) alongside legacy fields (add serde aliases) to maintain compatibility.
2. Update CLI commands incrementally, starting with read paths (dashboard/tree) before mutating commands.
3. Add GitHub Issues provider implementation and feature flag it behind `--issues-provider github` until stable.
4. Provide migration command or automatic upgrade when repo state lacks `issues.provider` (default to Jira for backward compatibility).

## Risks & Mitigations
- **Complexity creep**: Keep initial trait surface narrow; prefer `IssueAction::ProviderSpecific(serde_json::Value)` for future extensibility.
- **Performance**: Ensure caching/lazy fetching to avoid slower GitHub REST responses; respect rate limits via retry/backoff utilities.
- **User confusion**: Document differences in provider capabilities and add CLI hints when commands are unsupported for the active provider.

## Open Questions
- Should branch detection strategies be configurable per provider or via shared heuristics?
- Is there a need for a "null" provider (i.e., disable issue integration)? Potentially represent with `IssueProviderKind::None`.
- How should CLI output differentiate providers (emoji, prefix, color)? Decide prior to UI updates.

## Future Extensions
- Support multiple concurrent providers by allowing `issues.providers` array and updating state indices to be keyed by `(provider, reference)`.
- Add webhook/polling integrations for automatic updates when issues change upstream.
- Extend abstraction to cover attachments, assignee updates, and status transitions with richer typed APIs.
