# Twig VSCode Extension

This extension displays the current Twig branch tree in a sidebar and the currently selected branch in the status bar, using the Twig CLI.

## Features
- Sidebar tree view of branches (from `twig branch-tree`)
- Status bar item showing the current branch (from `twig current-branch`)
- Refresh command for the branch tree

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
