# Twig Flow Tutorial

This walkthrough shows how to use the `twig flow` plugin to browse and switch branches with Twig conventions.

## Prerequisites

- Twig CLI installed and on `PATH`.
- `twig-flow` plugin installed (`cargo install --path plugins/twig-flow`) and discoverable on `PATH`.
- A git repository with local branches; Jira/PR resolution optional but supported when configured.

## Quickstart

```
twig flow
twig flow --root
twig flow --parent
twig flow feature/auth-refresh
```

What to expect:

- The default `twig flow` renders a tree-plus-table with headers `Branch | Story | PR | Notes`, highlighting the current
  branch.
- `--root` switches to the configured root (e.g., `main`) before rendering.
- `--parent` switches to the primary parent branch (errors if multiple parents exist).
- Supplying a target runs the shared switch engine: checkout existing, create new when allowed, resolve Jira keys, and
  track PR references.

## Reading the Output

- Branch column shows tree connectors and highlights the current branch with `*`.
- Additional columns align under the header; missing metadata renders as placeholders.
- Ahead/behind counts appear next to branch names when available to show divergence from parents.

## Tips & Notes

- Works in non-interactive environments; warnings/errors are printed via Twig output helpers.
- Detached HEAD or empty repo results in header-only output with guidance.
- Interactive parent selection is deferred for now; multiple parents produce a descriptive error list.

## Testing

- Integration coverage lives in `plugins/twig-flow/tests`; run via `make test` (cargo nextest).
- Snapshot-style expectations depend on stable column alignment; avoid terminal-width assumptions in tests.
