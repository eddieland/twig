# Twig VSCode Extension

This extension displays the current Twig branch tree in a sidebar and the currently selected branch in the status bar, using the Twig CLI.

## Features
- **Sidebar tree view** of branches (from `twig tree`)
- **Status bar item** showing the current branch
- **Automatic refresh** when Git repository state changes
- **Real-time updates** when branches are created, deleted, or switched
- **File system monitoring** for `.twig/state.json` changes
- Manual refresh command for the branch tree

## Automatic Refresh

The extension automatically refreshes the tree view in the following situations:

### Via VS Code Git API
- Commits are made
- Branches are checked out
- Rebases, merges, or other Git operations complete
- Repository state changes

### Via File System Watchers
- `.git/HEAD` changes (branch switches)
- `.git/refs/heads/**` changes (branch creation/deletion/updates)
- `.twig/state.json` changes (Twig state updates)

No manual refresh needed! The tree view stays in sync with your Git operations automatically.

## Development
- Run `npm install` in this directory
- Build with `npm run compile`
- Launch the extension in a VSCode Extension Development Host

## Requirements
- Twig CLI must be installed and available in your PATH
- Works in a workspace with a Twig repository

## Installation

1. Open this folder (`twig-vscode`) in VS Code.
2. Run `npm install` to install dependencies.
3. Run `npm run compile` to build the extension.
4. Press `F5` to launch an Extension Development Host for testing.
5. To install manually:
	- Run `vsce package` (install [vsce](https://code.visualstudio.com/api/working-with-extensions/publishing-extension) if needed)
	- Install the generated `.vsix` file in VS Code via the Extensions view (three-dot menu > Install from VSIX...)

## TODO
- Error handling improvements
- Custom icons for branches
- Configuration options
