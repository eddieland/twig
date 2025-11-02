import * as vscode from 'vscode';
import { exec } from 'child_process';

class TwigBranchTreeProvider implements vscode.TreeDataProvider<TwigBranchItem> {
    private _onDidChangeTreeData: vscode.EventEmitter<TwigBranchItem | undefined | void> = new vscode.EventEmitter<TwigBranchItem | undefined | void>();
    readonly onDidChangeTreeData: vscode.Event<TwigBranchItem | undefined | void> = this._onDidChangeTreeData.event;

    refresh(): void {
        this._onDidChangeTreeData.fire();
    }

    getTreeItem(element: TwigBranchItem): vscode.TreeItem {
        return element;
    }

    getChildren(element?: TwigBranchItem): Thenable<TwigBranchItem[]> {
        if (element) {
            // Return children of the selected element
            return Promise.resolve(element.children);
        } else {
            // Return root elements by parsing twig tree
            return new Promise((resolve) => {
                const workspaceRoot = getWorkspaceRoot();
                if (!workspaceRoot) {
                    console.error('Twig: No workspace folder found');
                    resolve([new TwigBranchItem('No workspace folder open', [], true, false, false, true)]);
                    return;
                }
                
                console.log(`Twig: Executing 'twig tree' in directory: ${workspaceRoot}`);
                
                // First check if twig command is available
                exec('where twig', (whereErr, whereStdout) => {
                    if (whereErr) {
                        console.error('Twig: "twig" command not found in PATH');
                        resolve([
                            new TwigBranchItem('Error: twig command not found in PATH', [], true, false, false, true),
                            new TwigBranchItem('Install twig CLI: cargo install --path twig-cli', [], true, false, false, false)
                        ]);
                        return;
                    }
                    console.log(`Twig: Found twig at: ${whereStdout.trim()}`);
                    
                    // Use --repo flag to explicitly specify repository path
                    exec(`twig tree --repo "${workspaceRoot}"`, (err, stdout, stderr) => {
                        if (err) {
                            const errorMsg = stderr ? stderr : err.message;
                            console.error(`Twig: Error executing 'twig tree': ${errorMsg}`);
                            console.error(`Twig: Working directory was: ${workspaceRoot}`);
                            console.error(`Twig: stderr: ${stderr}`);
                            console.error(`Twig: stdout: ${stdout}`);
                            resolve([
                                new TwigBranchItem(`Error fetching branch tree: ${errorMsg}`, [], true, false, false, true),
                                new TwigBranchItem(`Working directory: ${workspaceRoot}`, [], true, false, false, false)
                            ]);
                            return;
                        }
                        console.log('Twig: Successfully fetched branch tree');
                        const items = parseTwigBranchTree(stdout);
                        resolve(items);
                    });
                });
            });
        }
    }
}

class TwigBranchItem extends vscode.TreeItem {
    constructor(
        public readonly label: string,
        public readonly children: TwigBranchItem[] = [],
        public readonly collapsible: boolean = false,
        public readonly isCurrentBranch: boolean = false,
        public readonly isOrphaned: boolean = false,
        public readonly isSection: boolean = false
    ) {
        super(label, children.length > 0 ? vscode.TreeItemCollapsibleState.Collapsed : vscode.TreeItemCollapsibleState.None);
        
        // Set context value for commands
        this.contextValue = isSection ? 'section' : 'branch';
        
        // Add visual indicators
        if (isCurrentBranch) {
            this.iconPath = new vscode.ThemeIcon('star-full', new vscode.ThemeColor('list.activeSelectionForeground'));
            this.description = '(current)';
        } else if (isOrphaned) {
            this.iconPath = new vscode.ThemeIcon('warning', new vscode.ThemeColor('list.warningForeground'));
        } else {
            this.iconPath = new vscode.ThemeIcon('git-branch');
        }
        
        // Style for section headers
        if (isSection) {
            this.iconPath = new vscode.ThemeIcon('folder');
            this.collapsibleState = vscode.TreeItemCollapsibleState.Expanded;
        }
    }
}

/**
 * Get the workspace root path for executing twig commands
 */
function getWorkspaceRoot(): string | undefined {
    const workspaceFolders = vscode.workspace.workspaceFolders;
    return workspaceFolders && workspaceFolders.length > 0 
        ? workspaceFolders[0].uri.fsPath 
        : undefined;
}

let currentBranch: string | null = null;

function parseTwigBranchTree(output: string): TwigBranchItem[] {
    const ansiRegex = /\x1B\[[0-9;]*m/g;
    const lines = output.split('\n');
    const result: TwigBranchItem[] = [];
    currentBranch = null;
    
    let i = 0;
    let orphanedBranches: string[] = [];
    let inOrphanedSection = false;
    
    // Handle JIRA_HOST error gracefully
    if (output.includes("Jira host environment variable 'JIRA_HOST' not set")) {
        result.push(new TwigBranchItem("Error: JIRA_HOST environment variable not set. Please set JIRA_HOST and reload VS Code.", [], true, false, false, true));
        return result;
    }
    
    // Handle case where no root branches are defined
    if (output.includes("Found user-defined dependencies but no root branches")) {
        const availableBranches: string[] = [];
        let inAvailableSection = false;
        
        for (const line of lines) {
            const trimmed = line.trim();
            if (trimmed === "Available branches:") {
                inAvailableSection = true;
                continue;
            }
            if (inAvailableSection && trimmed.startsWith("To fix this")) {
                break;
            }
            if (inAvailableSection && trimmed.length > 0 && !trimmed.startsWith("To fix")) {
                availableBranches.push(trimmed);
            }
        }
        
        if (availableBranches.length > 0) {
            const orphanedSection = new TwigBranchItem("📝 Available Branches", [], false, false, false, true);
            for (const branch of availableBranches) {
                orphanedSection.children.push(new TwigBranchItem(branch, [], false, false, true, false));
            }
            result.push(orphanedSection);
        }
        
        return result;
    }
    
    // Parse the tree structure
    const stack: {item: TwigBranchItem, indent: number}[] = [];
    
    while (i < lines.length) {
        const line = lines[i];
        const trimmed = line.trim();
        
        // Skip empty lines
        if (!trimmed) {
            i++;
            continue;
        }
        
        // Check for orphaned branches section
        if (trimmed.includes("📝 Orphaned branches")) {
            inOrphanedSection = true;
            i++;
            
            // Collect orphaned branches
            while (i < lines.length) {
                const orphanLine = lines[i].trim();
                if (!orphanLine) {
                    i++;
                    continue;
                }
                
                // Stop when we hit help text or end
                if (orphanLine.startsWith("To organize") || 
                    orphanLine.startsWith("ℹ") || 
                    orphanLine.startsWith("•") && !orphanLine.includes("• ")) {
                    break;
                }
                
                // Extract branch names from bullet points
                const match = orphanLine.match(/^• (.+)$/);
                if (match) {
                    orphanedBranches.push(match[1]);
                }
                i++;
            }
            break;
        }
        
        // Skip help/info lines
        if (/^ℹ|^\u2139|To organize|Add as root|Add dependency|Link Jira|issues:|PRs:|https?:\/\//.test(trimmed)) {
            i++;
            continue;
        }
        
        // Detect current branch (colored output)
        if (/\x1B\[[0-9;]*m/.test(line)) {
            const cleanedLine = line.replace(ansiRegex, '');
            const branchMatch = cleanedLine.match(/[│├└─\s]*(.+)/);
            if (branchMatch) {
                currentBranch = branchMatch[1].trim();
            }
        }
        
        // Parse tree structure
        const cleanLine = line.replace(ansiRegex, '');
        const treeChars = cleanLine.match(/^[│├└─\s]*/);
        const indent = treeChars ? treeChars[0].length : 0;
        const branchName = cleanLine.replace(/^[│├└─\s]*/, '').trim();
        
        if (branchName) {
            const isCurrentBranchItem = currentBranch === branchName;
            const item = new TwigBranchItem(branchName, [], false, isCurrentBranchItem, false, false);
            
            // Handle hierarchy using indentation
            while (stack.length > 0 && stack[stack.length - 1].indent >= indent) {
                stack.pop();
            }
            
            if (stack.length === 0) {
                result.push(item);
            } else {
                stack[stack.length - 1].item.children.push(item);
                // Update parent's collapsible state
                if (stack[stack.length - 1].item.collapsibleState === vscode.TreeItemCollapsibleState.None) {
                    stack[stack.length - 1].item.collapsibleState = vscode.TreeItemCollapsibleState.Collapsed;
                }
            }
            
            stack.push({item, indent});
        }
        
        i++;
    }
    
    // Add orphaned branches section if any exist
    if (orphanedBranches.length > 0) {
        const orphanedSection = new TwigBranchItem("📝 Orphaned Branches", [], false, false, false, true);
        for (const branch of orphanedBranches) {
            const isCurrentBranchItem = currentBranch === branch;
            orphanedSection.children.push(new TwigBranchItem(branch, [], false, isCurrentBranchItem, true, false));
        }
        result.push(orphanedSection);
    }
    
    return result;
}

/**
 * Set up watchers to automatically refresh the tree view when Git repository state changes
 */
async function setupGitRepositoryWatchers(context: vscode.ExtensionContext, treeDataProvider: TwigBranchTreeProvider) {
    const gitExtension = vscode.extensions.getExtension<any>('vscode.git');
    if (!gitExtension) {
        console.warn('Twig: Built-in Git extension not found; automatic refresh disabled.');
        return;
    }

    try {
        if (!gitExtension.isActive) {
            await gitExtension.activate();
        }
    } catch (error) {
        console.error('Twig: Failed to activate Git extension; automatic refresh disabled.', error);
        return;
    }

    const gitApi = (gitExtension.exports && typeof gitExtension.exports.getAPI === 'function')
        ? gitExtension.exports.getAPI(1)
        : undefined;

    if (!gitApi) {
        console.warn('Twig: Git extension API unavailable; automatic refresh disabled.');
        return;
    }

    if (gitApi.repositories.length > 0) {
        gitApi.repositories.forEach((repo: any) => {
            const disposable = repo.state.onDidChange(() => {
                console.log('Git repository state changed - refreshing Twig view');
                treeDataProvider.refresh();
            });
            context.subscriptions.push(disposable);
        });

        const onDidOpenRepository = gitApi.onDidOpenRepository((repo: any) => {
            console.log('New Git repository opened - setting up watchers');
            const disposable = repo.state.onDidChange(() => {
                console.log('Git repository state changed - refreshing Twig view');
                treeDataProvider.refresh();
            });
            context.subscriptions.push(disposable);
        });
        context.subscriptions.push(onDidOpenRepository);
    }
    
    // Also watch for file system changes to .twig directory
    const workspaceFolders = vscode.workspace.workspaceFolders;
    if (workspaceFolders && workspaceFolders.length > 0) {
        // Watch .twig/state.json for twig-specific state changes
        const twigStatePattern = new vscode.RelativePattern(
            workspaceFolders[0],
            '.twig/state.json'
        );
        const twigWatcher = vscode.workspace.createFileSystemWatcher(twigStatePattern);
        
        twigWatcher.onDidChange(() => {
            console.log('.twig/state.json changed - refreshing Twig view');
            treeDataProvider.refresh();
        });
        
        twigWatcher.onDidCreate(() => {
            console.log('.twig/state.json created - refreshing Twig view');
            treeDataProvider.refresh();
        });
        
        twigWatcher.onDidDelete(() => {
            console.log('.twig/state.json deleted - refreshing Twig view');
            treeDataProvider.refresh();
        });
        
        context.subscriptions.push(twigWatcher);
        
        // Watch .git/HEAD for branch switches
        const gitHeadPattern = new vscode.RelativePattern(
            workspaceFolders[0],
            '.git/HEAD'
        );
        const gitHeadWatcher = vscode.workspace.createFileSystemWatcher(gitHeadPattern);
        
        gitHeadWatcher.onDidChange(() => {
            console.log('.git/HEAD changed - refreshing Twig view');
            treeDataProvider.refresh();
        });
        
        context.subscriptions.push(gitHeadWatcher);
        
        // Watch .git/refs/heads for branch creation/deletion
        const gitRefsPattern = new vscode.RelativePattern(
            workspaceFolders[0],
            '.git/refs/heads/**'
        );
        const gitRefsWatcher = vscode.workspace.createFileSystemWatcher(gitRefsPattern);
        
        gitRefsWatcher.onDidCreate(() => {
            console.log('Git branch created - refreshing Twig view');
            treeDataProvider.refresh();
        });
        
        gitRefsWatcher.onDidDelete(() => {
            console.log('Git branch deleted - refreshing Twig view');
            treeDataProvider.refresh();
        });
        
        gitRefsWatcher.onDidChange(() => {
            console.log('Git branch updated - refreshing Twig view');
            treeDataProvider.refresh();
        });
        
        context.subscriptions.push(gitRefsWatcher);
    }
}

export function activate(context: vscode.ExtensionContext) {
    const treeDataProvider = new TwigBranchTreeProvider();
    vscode.window.registerTreeDataProvider('twigBranchTreeView', treeDataProvider);

    // Set up Git repository monitoring for automatic refresh
    setupGitRepositoryWatchers(context, treeDataProvider);
    context.subscriptions.push(
        vscode.commands.registerCommand('twig.refreshBranchTree', () => {
            treeDataProvider.refresh();
        })
    );
    context.subscriptions.push(
        vscode.commands.registerCommand('twig.switchBranch', (item: TwigBranchItem) => {
            // If it's a section item, don't allow switching
            if (item && item.isSection) {
                vscode.window.showInformationMessage('Cannot switch to a section. Please select a branch.');
                return;
            }
            
            let targetBranch: string | null = null;
            
            if (item && !item.isSection) {
                // Use the selected branch name directly
                targetBranch = item.label;
            } else {
                // If no item provided, prompt for branch name
                vscode.window.showInputBox({ prompt: 'Enter branch name to switch to' }).then((newBranchName) => {
                    if (!newBranchName) {
                        vscode.window.showErrorMessage('Branch name is required.');
                        return;
                    }
                    switchToBranch(newBranchName, treeDataProvider);
                });
                return;
            }
            
            if (targetBranch) {
                switchToBranch(targetBranch, treeDataProvider);
            }
        })
    );

    // Helper function to switch branches
    function switchToBranch(targetBranch: string, treeDataProvider: TwigBranchTreeProvider) {
        const workspaceRoot = getWorkspaceRoot();
        if (!workspaceRoot) {
            vscode.window.showErrorMessage('No workspace folder open');
            return;
        }
        
        if (currentBranch === targetBranch) {
            vscode.window.showInformationMessage(`Already on branch: ${targetBranch}`);
            return;
        }

        vscode.window.withProgress({ 
            location: vscode.ProgressLocation.Notification, 
            title: `Switching to ${targetBranch}` 
        }, async () => {
            return new Promise<void>((resolve) => {
                exec(`twig switch "${targetBranch}"`, { cwd: workspaceRoot }, (err, stdout, stderr) => {
                    if (err) {
                        vscode.window.showErrorMessage(`Failed to switch branch: ${stderr ? stderr : err.message}`);
                    } else {
                        vscode.window.showInformationMessage(`Successfully switched to branch: ${targetBranch}`);
                        // Refresh the tree view to reflect the new state
                        treeDataProvider.refresh();
                    }
                    resolve();
                });
            });
        });
    }
    context.subscriptions.push(
        vscode.commands.registerCommand('twig.createBranchFromCurrent', async () => {
            const workspaceRoot = getWorkspaceRoot();
            if (!workspaceRoot) {
                vscode.window.showErrorMessage('No workspace folder open');
                return;
            }
            
            // Detect the current branch from 'twig tree' output
            exec('twig tree', { cwd: workspaceRoot }, (err, stdout) => {
                if (err) {
                    vscode.window.showErrorMessage('Failed to fetch branch list.');
                    return;
                }
                // Find the colored line (current branch)
                const ansiRegex = /\x1B\[[0-9;]*m/g;
                let originalBranch: string | null = null;
                for (const line of stdout.split('\n')) {
                    if (/\x1B\[[0-9;]*m/.test(line)) {
                        originalBranch = line.replace(ansiRegex, '').replace(/^[│├└─\s]+/, '').trim();
                        break;
                    }
                }
                if (!originalBranch) {
                    vscode.window.showErrorMessage('Could not detect current branch.');
                    return;
                }
                vscode.window.showInputBox({ prompt: 'Enter new branch name' }).then((newBranch) => {
                    if (!newBranch) {
                        vscode.window.showErrorMessage('Branch name is required.');
                        return;
                    }
                    // Remove tree-drawing characters and whitespace from all branch names
                    const branchExists = stdout.split('\n')
                        .map(l => l.replace(/^[│├└─\s]+/, '').replace(ansiRegex, '').trim())
                        .includes(newBranch);
                    if (branchExists) {
                        vscode.window.showErrorMessage(`Branch '${newBranch}' already exists.`);
                        return;
                    }
                    vscode.window.withProgress({ location: vscode.ProgressLocation.Notification, title: `Creating branch ${newBranch} from ${originalBranch}` }, async () => {
                        return new Promise<void>((resolve) => {
                            // Create new branch by switching to it
                            exec(`twig switch -p "${originalBranch}" "${newBranch}"`, { cwd: workspaceRoot }, (err1, stdout1, stderr1) => {
                                if (err1) {
                                    vscode.window.showErrorMessage(`Failed to create branch: ${stderr1 ? stderr1 : err1.message}`);
                                } else {
                                    vscode.window.showInformationMessage(`Successfully created and switched to branch: ${newBranch}`);
                                    // Refresh the tree view to reflect the new state
                                    treeDataProvider.refresh();
                                }
                                resolve();
                            });
                        });
                    });
                });
            });
        })
    );
}

export function deactivate() {}
