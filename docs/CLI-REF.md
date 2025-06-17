# Twig CLI Reference

```
Twig helps developers manage multiple Git repositories and worktrees efficiently.

It provides commands for repository tracking, batch operations, and worktree
management to streamline your development workflow.

Usage: twig [OPTIONS] [PLUGIN_ARGS]... [COMMAND]

Commands:
  branch      Branch dependency and root management
  cascade     Perform a cascading rebase from the current branch to its children
  commit      Create a commit using Jira issue information
  fixup       Create fixup commits interactively
  completion  Generate shell completions
  creds       Credential management
  dashboard   Show a comprehensive dashboard of local branches, PRs, and issues
  diagnose    Run system diagnostics
  git         Git repository management
  github      GitHub integration
  init        Initialize twig configuration
  jira        Jira integration
  rebase      Rebase the current branch on its parent(s)
  switch      Switch to branches by Jira issue, GitHub PR, or branch name
  sync        Automatically link branches to Jira issues and GitHub PRs
  tree        Show your branch tree with user-defined dependencies
  worktree    Worktree management

Arguments:
  [PLUGIN_ARGS]...
          Plugin name and arguments (when no subcommand matches)

Options:
  -v, --verbose...
          Sets the level of verbosity for tracing and logging output.
          
          -v: Show info level messages
          -vv: Show debug level messages
          -vvv: Show trace level messages

      --colors <COLORS>
          Controls when colored output is used
          
          [default: auto]

          Possible values:
          - yes:    Enable colored output
          - always: Enable colored output (alias for Yes)
          - auto:   Automatically detect if colors should be used based on terminal capabilities
          - no:     Disable colored output
          - never:  Disable colored output (alias for No)

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## branch
```
Manage custom branch dependencies and root branches.

This command group allows you to define custom parent-child relationships
between branches beyond Git's automatic detection. You can also manage
which branches should be treated as root branches in the tree view.

Usage: twig branch <COMMAND>

Commands:
  depend      Add a dependency between branches
  remove-dep  Remove a dependency between branches
  root        Root branch management

Options:
  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## cascade
```
Perform a cascading rebase from the current branch to its children.

This command rebases all child branches on their parent(s) in a cascading manner,
starting from the current branch and working down the dependency tree.

Usage: twig cascade [OPTIONS]

Options:
      --max-depth <DEPTH>
          Maximum depth for cascading rebase

      --force
          Force rebase even if branches are up-to-date

      --show-graph
          Show dependency graph before rebasing

      --autostash
          Automatically stash and pop pending changes

  -r, --repo <PATH>
          Path to a specific repository

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## completion
```
Generates shell completion scripts for twig commands.

This command generates completion scripts that provide tab completion for twig
commands and options in your shell. Supported shells include bash, zsh, and fish.

Usage: twig completion <SHELL>

Arguments:
  <SHELL>
          Shell to generate completions for

          Possible values:
          - bash:        Bourne Again `SHell` (bash)
          - fish:        Friendly Interactive `SHell` (fish)
          - power-shell: `PowerShell`
          - zsh:         Z `SHell` (zsh)

Options:
  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## creds
```
Manage credentials for external services like Jira and GitHub.

This command group helps you check and set up credentials for the
external services that twig integrates with. Credentials are stored
in your .netrc file for security and compatibility with other tools.

Usage: twig creds <COMMAND>

Commands:
  check  Check if credentials are properly configured
  setup  Set up credentials interactively

Options:
  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## dashboard
```
Show a comprehensive dashboard of local branches, PRs, and issues.

This command displays a unified view of your development context,
including local branches, associated pull requests, and related Jira issues.
It helps you keep track of your work across different systems.

By default, only local branches are shown. Use --include-remote to include remote branches.

Use --no-github or --no-jira to disable GitHub or Jira API requests respectively.
Use --simple for a basic view that shows only branches without making any API requests.

Usage: twig dashboard [OPTIONS]

Options:
  -m, --mine
          Show only items assigned to or created by the current user

  -r, --recent
          Show only recent items (last 7 days)

  -p, --repo <PATH>
          Path to a specific repository (defaults to current repository)

  -f, --format <FORMAT>
          Output format
          
          [default: text]
          [possible values: text, json]

      --include-remote
          Include remote branches in the dashboard

      --no-github
          Disable GitHub PR information (avoids GitHub API requests)

      --no-jira
          Disable Jira issue information (avoids Jira API requests)

  -s, --simple
          Simple view (equivalent to --no-github --no-jira)

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## git
```
Manage multiple Git repositories through twig.

This command group allows you to register, track, and perform operations
across multiple repositories. Repositories added to twig can be referenced
in other commands and batch operations.

Usage: twig git <COMMAND>

Commands:
  add             Add a repository to the registry
  exec            Execute a git command in repositories
  fetch           Fetch updates for repositories
  list            List all repositories in the registry
  remove          Remove a repository from the registry
  stale-branches  List stale branches in repositories

Options:
  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## github
```
Interact with GitHub repositories and pull requests.

This command group provides functionality for working with GitHub,
including checking authentication, viewing pull request status,
and linking branches to pull requests.

Usage: twig github <COMMAND>

Commands:
  check   Check GitHub authentication
  checks  View CI/CD checks for a PR
  open    Open GitHub PR in browser
  pr      Pull request operations

Options:
  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## jira
```
Interact with Jira issues and create branches from them.

This command group provides functionality for working with Jira,
including viewing issues, transitioning issues through workflows,
and creating branches from issues.

Usage: twig jira <COMMAND>

Commands:
  open           Open Jira issue in browser
  create-branch  Create a branch from a Jira issue
  link-branch    Link a branch to a Jira issue
  transition     Transition a Jira issue
  view           View a Jira issue

Options:
  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## rebase
```
Rebase the current branch on its parent(s).

This command rebases the current branch on its parent(s) based on
the dependency tree. It can optionally start from the root branch.

Usage: twig rebase [OPTIONS]

Options:
      --force
          Force rebase even if branches are up-to-date

      --show-graph
          Show dependency graph before rebasing

      --autostash
          Automatically stash and pop pending changes

  -r, --repo <PATH>
          Path to a specific repository

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## switch
```
Intelligently switch to branches based on various inputs.

This command can switch branches based on:
• Jira issue key (e.g., PROJ-123)
• Jira issue URL
• GitHub PR ID (e.g., 12345 or PR#12345)
• GitHub PR URL
• Branch name

The command will automatically detect the input type and find the
corresponding branch. By default, missing branches will be created
automatically. Use --no-create to disable this behavior.

Usage: twig switch [OPTIONS] [INPUT]

Arguments:
  [INPUT]
          Jira issue, GitHub PR, or branch name
          
          Can be any of the following:
          • Jira issue key (PROJ-123)
          • Jira issue URL (https://company.atlassian.net/browse/PROJ-123)
          • GitHub PR ID (12345 or PR#12345)
          • GitHub PR URL (https://github.com/owner/repo/pull/123)
          • Branch name (feature/my-branch)
          
          Not required when using --root flag.

Options:
      --root
          Switch to the current branch's dependency tree root
          
          Traverses up the dependency chain from the current branch to find and switch to
          the topmost parent branch. If the current branch has no dependencies, it will
          remain on the current branch. This helps navigate to the root of a feature
          branch dependency tree.

      --no-create
          Don't create branch if it doesn't exist
          
          Disable the default behavior of creating branches when they don't exist.
          By default, twig switch will create missing branches. Use this flag
          to only switch to existing branches.

  -p, --parent [<PARENT>]
          Set parent dependency for the new branch (only applies when creating a new branch)
          
          Specify a parent branch to create a dependency relationship when a new branch is created.
          This option is ignored when switching to existing branches.
          Values can be:
          • 'current' (default if flag used without value): Use current branch
          • A branch name: Use the specified branch
          • A Jira issue key (e.g., PROJ-123): Use branch associated with Jira issue
          • 'none': Don't set any parent (use default root)

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## sync
```
Scan local branches and automatically detect and link them to their corresponding
Jira issues and GitHub PRs.

For GitHub PRs, this command:
• First searches GitHub's API for pull requests matching the branch name
• Falls back to detecting patterns in branch names if API is unavailable

For Jira issues, it looks for patterns in branch names like:
• PROJ-123/feature-name, feature/PROJ-123-description

GitHub PR branch naming patterns (fallback detection):
• pr-123-description, github-pr-123, pull-123, pr/123

It will automatically create associations for detected patterns and report
any branches that couldn't be linked.

Usage: twig sync [OPTIONS]

Options:
  -r, --repo <PATH>
          Path to a specific repository

      --dry-run
          Show what would be synced without making changes

      --force
          Update existing associations that differ from detected patterns

      --no-jira
          Skip detection and linking of Jira issues

      --no-github
          Skip detection and linking of GitHub PRs

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## tree
```
Display local branches in a tree-like view based on user-defined dependencies.

This command shows branch relationships that you have explicitly defined using
the 'twig branch depend' command. It also displays associated Jira issues and
GitHub PRs. Branches without defined dependencies or root status will be shown
as orphaned branches. Use 'twig branch depend' to create relationships and
'twig branch root add' to designate root branches.

Usage: twig tree [OPTIONS]

Options:
  -r, --repo <PATH>
          Path to a specific repository

  -d, --max-depth <DEPTH>
          Maximum depth to display in the tree

      --no-color
          Disable colored output

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## worktree
```
Manage Git worktrees for efficient multi-branch development.

Worktrees allow you to check out multiple branches simultaneously in separate
directories, all connected to the same repository. This enables working on
different features or fixes concurrently without stashing or committing
incomplete work.

Usage: twig worktree <COMMAND>

Commands:
  clean   Clean up stale worktrees
  create  Create a new worktree for a branch
  list    List all worktrees for a repository

Options:
  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```
