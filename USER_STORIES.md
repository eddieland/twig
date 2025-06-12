# User Stories from argit User Feedback

This document contains user stories derived from feedback provided by an argit user, capturing key workflows and feature requests that should be considered for twig implementation.

The user stories are categorized into:

1. **Core argit Features** - Existing functionality in argit that should be implemented in twig
2. **Enhancement Requests** - Nice-to-have improvements that weren't in the original argit

## Core argit Features

### Creating and Navigating Branches

1. I want to create new branches linked to tickets using a simple command (`flow`/`switch`) so that I can quickly start working on new features or fixes.

   **Implementation Status**: ✅ Fully implemented

   - In argit, `argit flow J-123` was used to switch to (and create if needed) a Jira branch
   - Twig provides `twig switch` command that can create branches from Jira issues
   - Also offers `twig jira branch create` for explicit creation
   - Example: `twig switch PROJ-123` or `twig jira branch create PROJ-123`

2. I want to quickly navigate to the root branch (`flow --root`/`switch --root`) so that I can update my local repository with the latest changes.

   **Implementation Status**: ⚠️ Partially implemented

   - Twig doesn't have a direct `--root` flag for switch
   - However, you can switch to them directly: `twig switch main`

3. I want to quickly open the web browser to view Jira issues or GitHub PRs (`story go`/`pr go`) so that I can easily access their details.

   **Implementation Status**: ✅ Fully implemented

   - In argit, `argit pr go` and `argit story go` opened the web browser to view the PR/story
   - Twig provides equivalent functionality with:
     - `twig jira open` - Opens Jira issue in browser (defaults to current branch's issue)
     - `twig github open` - Opens GitHub PR in browser (defaults to current branch's PR)
   - Examples: `twig jira open PROJ-123` or `twig github open 42`

### Branch Rebasing and Dependency Management

4. I want to perform cascading rebases (`cascade`) so that changes from parent branches automatically propagate to all child branches.

   **Implementation Status**: ✅ Fully implemented

   - Twig provides `twig cascade` command
   - Supports options like `--max-depth`, `--force`, `--show-graph`, and `--autostash`
   - Example: `twig cascade`

5. I want to easily identify when my tickets have landed (`tidy`) so that I can update ticket statuses in project management tools.

   **Implementation Status**: ⚠️ Partially implemented

   - Twig doesn't have a direct equivalent to argit's `tidy`
   - However, `twig git stale-branches` provides similar functionality to identify branches that haven't been updated recently
   - Example: `twig git stale-branches --days 30`

6. I want to add dependencies to parent branches (`track`) so that I can establish relationships between my current branch and its parent branch (e.g., `track feat/my-parent`).

   **Implementation Status**: ✅ Fully implemented

   - Twig provides `twig branch depend` command
   - Example: `twig branch depend feature/child-branch feature/parent-branch`
   - Dependencies are visualized with `twig tree`

### Branch Locking

7. I want to lock branches (`lock`) so that I can create temporary branches that I don't want to push but still want to consistently rebase.

   **Implementation Status**: ⚠️ Partially implemented

   - Similar behavior can be achieved by defining separate root branches
   - Direct branch locking functionality is planned for future implementation

## Enhancement Requests

### Branch Management Improvements

8. I want child branches to be automatically reparented when their parent branch is merged so that my branch hierarchy remains clean and accurate.

   **Implementation Status**: ❌ Not yet implemented

   - This feature is not currently available in twig
   - Planned for future implementation

9. I want an aggressive cleanup option (`tidy --aggressive`) so that I can remove empty parent branches and maintain a cleaner branch structure.

   **Implementation Status**: ❌ Not yet implemented

   - This feature is not currently available in twig
   - Planned for future implementation

### Configuration Management

10. I want a global configuration option that works across repositories so that I can use the same workflow tools even in repositories without specific tool configuration and so that I can keep repository content and history clean.

    **Implementation Status**: ⚠️ Partially implemented

    - Twig stores some configuration in XDG directories (`~/.config/twig`, etc.)
    - However, it still creates a `.twig` directory inside each repository
      - This directory needs to be added to `.gitignore` to keep the repository clean
    - Repositories are tracked globally: `twig git add /path/to/repo`

### Integration with External Tools

11. I want integration with lat-pr for handling parent branches and labels so that I can maintain branch dependencies when creating pull requests.

    **Implementation Status**: ❌ Not yet implemented

    - This feature is not currently available in twig
    - Planned for future implementation

### Enhanced Branch Locking

12. I want different levels of branch locking so that I can control how branches are managed:

    - Normal branches (standard workflow)
    - Local-only branches (for temporary work that shouldn't be pushed)
    - Remote-updates-only branches (based on someone else's branch)

    **Implementation Status**: ⚠️ Partially implemented

    - Similar behavior can be achieved by defining separate root branches
      - Root branches can be defined using `twig branch root add <branch-name>`
      - Different root branches can have separate dependency trees
      - This allows for isolation between different types of branches
    - Direct ignore/locking type support will be added in a future implementation

## Deprecated Features

The following features were mentioned as no longer used and may not need to be prioritized:

- Command runner (CI is used instead)
- Commit dropping (replaced by "bueller")
- PR command (replaced by lat-pr tui)

## Implementation Considerations

When implementing these features in twig, consider:

1. Maintaining compatibility with existing workflows while leveraging Rust's performance benefits
2. Providing clear migration paths from argit commands to twig equivalents
3. Enhancing the most frequently used commands first:
   - `flow`/`switch` - ✅ Implemented
   - `cascade` - ✅ Implemented
   - `tidy` - ⚠️ Partially implemented via `git stale-branches`
   - `track` - ✅ Implemented via `branch depend`
   - `story go`/`pr go` - ✅ Implemented via `jira open` and `github open`

## Additional Features in Twig

Twig offers several features that weren't in the original argit:

1. **Worktree Support**: Efficiently manage git worktrees for feature development

   - `twig worktree create feature/new-thing`
   - `twig worktree list`
   - `twig worktree clean`

2. **GitHub Integration**: Track PR status and review information

   - `twig github pr status`
   - `twig github checks`

3. **Batch Operations**: Execute commands across all tracked repositories

   - `twig git exec "git status"`

4. **Comprehensive Dashboard**: View all branches, PRs, and issues in one place

   - `twig dashboard`

5. **Upward Rebasing**: Rebase current branch on its ancestors

   - `twig rebase`

6. **Plugin Architecture**: Extensible kubectl/Docker-inspired plugin system for custom functionality

   - Plugins are executable files named `twig-<plugin-name>` discovered via `$PATH`
   - Built-in commands take precedence over plugins
   - Plugins receive context through environment variables (`TWIG_CONFIG_DIR`, `TWIG_CURRENT_REPO`, etc.)
   - Can be implemented in any language (Rust, Python, Shell, etc.)
     - Rust plugins can directly use the `twig-core` library for deep integration
     - Other languages can potentially use FFI (Rust has robust FFI support, e.g., Python bindings)
   - Examples: `twig deploy`, `twig backup`, `twig lint`
   - See [`docs/PLUGINS.md`](docs/PLUGINS.md) for development guide
