# User Stories from argit User Feedback

This document contains user stories derived from feedback provided by an argit user, capturing key workflows and feature requests that should be considered for twig implementation.

The user stories are categorized into:
1. **Core argit Features** - Existing functionality in argit that should be implemented in twig
2. **Enhancement Requests** - Nice-to-have improvements that weren't in the original argit

## Core argit Features

### Creating and Navigating Branches

1. **As a developer**, I want to create new branches linked to tickets using a simple command (`flow`/`switch`) so that I can quickly start working on new features or fixes.

2. **As a developer**, I want to quickly navigate to the root branch (`flow --root`/`switch --root`) so that I can update my local repository with the latest changes.

3. **As a developer**, I want to quickly navigate to branches associated with specific stories or PRs (`story go`/`pr go`) so that I can efficiently switch between different tasks.

### Branch Rebasing and Dependency Management

4. **As a developer**, I want to perform cascading rebases (`cascade`) so that changes from parent branches automatically propagate to all child branches.

5. **As a developer**, I want to easily identify when my tickets have landed (`tidy`) so that I can update ticket statuses in project management tools.

6. **As a developer**, I want to add dependencies to parent branches (`track`) so that I can establish relationships between my current branch and its parent branch (e.g., `track feat/my-parent`).

### Branch Locking

7. **As a developer**, I want to lock branches (`lock`) so that I can create temporary branches that I don't want to push but still want to consistently rebase.

## Enhancement Requests

### Branch Management Improvements

8. **As a developer**, I want child branches to be automatically reparented when their parent branch is merged so that my branch hierarchy remains clean and accurate.

9. **As a developer**, I want an aggressive cleanup option (`tidy --aggressive`) so that I can remove empty parent branches and maintain a cleaner branch structure.

### Configuration Management

10. **As a developer**, I want a global configuration option that works across repositories so that I can use the same workflow tools even in repositories without specific tool configuration.

11. **As a developer**, I want to initialize repositories without checking in configuration files so that I can keep repository history clean while still using workflow tools.

### Integration with External Tools

12. **As a developer**, I want integration with lat-pr for handling parent branches and labels so that I can maintain branch dependencies when creating pull requests.

### Enhanced Branch Locking

13. **As a developer**, I want different levels of branch locking so that I can control how branches are managed:
   - Normal branches (standard workflow)
   - Local-only branches (for temporary work that shouldn't be pushed)
   - Remote-updates-only branches (based on someone else's branch)

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
   - `flow`/`switch`
   - `cascade`
   - `tidy`
   - `track`
   - `story go`/`pr go`
