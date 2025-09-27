# Twig Short-Term Improvement Plan

## Scope
- Focus on the highest-friction core flows: switching into work, maintaining dependency trees, and managing local worktrees.
- Defer tertiary tooling such as `twig git` subcommands; concentrate on experiences tied to branch lifecycle, Jira linkage, and cascaded rebases.

## High-Value Tasks

### 1. Auto-checkout Remote Branches During `twig switch`
- **Summary**: Extend branch discovery so `twig switch` can materialize remote-only branches that match the requested Jira issue, PR, or branch slug before falling back to branch creation.
- **User Value**: Prevents accidental duplicate branches and makes it effortless to resume work started on another machine or by a collaborator when the branch metadata exists remotely but not locally.
- **Implementation Notes**:
  - When `RepoState::get_branch_issue_by_jira` misses or the local branch is absent, query `origin` refs for names containing the normalized issue key or sanitized slug before creating a new branch.【F:twig-cli/src/cli/switch.rs†L289-L389】【F:twig-core/src/state.rs†L323-L343】
  - Add a helper that fetches (optional) and sets up a local tracking branch, then persists the association back into the repo state for subsequent lookups.【F:twig-cli/src/cli/switch.rs†L552-L644】
  - Cover scenarios with multiple matching remotes via an ordered prompt (e.g., prefer `<issue>/<slug>`), defaulting to branch creation only when nothing matches.
- **Estimated Effort**: ~250 LOC (new git helpers, refactoring switch flow, tests in `twig-cli`).
- **Dependencies**: Requires git remote parsing utilities but no external services.

### 2. Rich Context Summary After Switching
- **Summary**: After a successful `twig switch`, surface branch metadata such as Jira issue, linked PR, and immediate parent/child dependencies.
- **User Value**: Gives developers instant situational awareness, reducing the need to run `twig tree` or Jira commands to recall context.
- **Implementation Notes**:
  - Enhance `switch_to_branch` to load `RepoState`, fetch dependency parents/children, and print a short summary with helpers from `twig_core::output` (respect verbosity levels).【F:twig-cli/src/cli/switch.rs†L359-L420】【F:twig-core/src/state.rs†L323-L420】
  - Include graceful fallbacks when metadata is missing and gate expensive lookups behind a verbosity flag if needed.
  - Add unit tests around the formatter plus an integration test asserting the emitted summary when metadata is present.
- **Estimated Effort**: ~180 LOC (formatting helpers + tests).
- **Dependencies**: Builds on existing state indices; no service calls.

### 3. Collision-resistant Jira Branch Slug Generation
- **Summary**: Harden `twig switch`/`twig jira branch create` so Jira-derived branch names stay valid and unique even for punctuation-heavy or duplicate summaries.
- **User Value**: Avoids branch creation failures and confusing name collisions when Jira summaries are short, non-alphanumeric, or reused between issues.
- **Implementation Notes**:
  - Extend the sanitization pipeline to collapse repeated separators, enforce a maximum length, and fall back to the issue key alone when sanitization produces an empty slug.【F:twig-cli/src/cli/switch.rs†L535-L563】
  - Before creating the branch, probe for existing refs and append an incrementing suffix when needed (e.g., `/foo`, `/foo-2`).【F:twig-cli/src/cli/switch.rs†L367-L383】
  - Share the slugging logic between Jira branch creation paths to keep behavior consistent, with regression tests covering edge cases.
- **Estimated Effort**: ~160 LOC (shared slug helper, branch existence checks, tests).
- **Dependencies**: Local git only.

### 4. `twig cascade` Dry-Run Planner & Conflict Summary
- **Summary**: Introduce a `--dry-run` flag that previews the cascade order, parent relationships, and potential blockers without executing rebases; surface a consolidated conflict summary when operations fail mid-run.
- **User Value**: Allows developers to anticipate large rebases, understand impact, and communicate required manual steps before modifying history.
- **Implementation Notes**:
  - Compute and print the planned rebase queue (respecting `--max-depth`) and list branches lacking parents, but skip git executions when in dry-run mode.【F:twig-cli/src/cli/cascade.rs†L20-L200】
  - Track per-branch results to emit a final report highlighting successes, skipped branches, and conflicts, even when the command aborts early.
  - Add unit coverage for ordering logic plus a command test using fixtures that asserts dry-run output.
- **Estimated Effort**: ~220 LOC (argument plumbing, reporting structs, tests).
- **Dependencies**: Existing dependency resolver; no new services.

### 5. Worktree List with Branch & Issue Insights
- **Summary**: Enrich `twig worktree list` to display associated Jira issue/PR metadata and flag desynced worktrees (missing directories, detached branches).
- **User Value**: Gives a dashboard view of active worktrees, reducing the mental overhead of tracking which worktree maps to which ticket.
- **Implementation Notes**:
  - Join git worktree data with `RepoState::list_worktrees` and branch metadata to print issue keys, PR numbers, and dependency parents alongside each entry.【F:twig-cli/src/cli/worktree.rs†L81-L205】【F:twig-core/src/state.rs†L320-L376】
  - Highlight anomalies (e.g., state entry without a git worktree, missing filesystem path) with `print_warning`, suggesting cleanup commands.
  - Add tests covering both healthy and inconsistent states.
- **Estimated Effort**: ~200 LOC (output enhancements, helper functions, tests).
- **Dependencies**: Relies on local state; no remote calls.

## Next Steps
1. Validate priority ordering with stakeholders focused on branch lifecycle workflows.
2. Sequence work so the slug-generation improvements land before remote auto-checkout to minimize merge conflicts in `switch.rs`.
3. After each task, update user-facing docs or help text where applicable to advertise the enhancements.
