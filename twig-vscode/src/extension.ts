import * as vscode from 'vscode';
import { exec } from 'child_process';

class TwigBranchTreeProvider implements vscode.TreeDataProvider<TwigBranchItem>, vscode.TreeDragAndDropController<TwigBranchItem> {
    private _onDidChangeTreeData: vscode.EventEmitter<TwigBranchItem | undefined | void> = new vscode.EventEmitter<TwigBranchItem | undefined | void>();
    readonly onDidChangeTreeData: vscode.Event<TwigBranchItem | undefined | void> = this._onDidChangeTreeData.event;

    // Define MIME types for drag and drop
    dropMimeTypes = ['application/vnd.code.tree.twigbranchtreeview'];
    dragMimeTypes = ['application/vnd.code.tree.twigbranchtreeview'];

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
            // Return root elements by parsing twig tree and checking for untracked branches
            return new Promise((resolve) => {
                // First, get the current branch from git
                exec('git branch --show-current', { cwd: vscode.workspace.rootPath }, (gitCurrentErr, gitCurrentOutput) => {
                    const gitCurrentBranch = gitCurrentErr ? null : gitCurrentOutput.trim();
                    
                    exec('twig tree', { cwd: vscode.workspace.rootPath }, (err, stdout, stderr) => {
                        if (err) {
                            const errorMsg = stderr ? stderr : err.message;
                            resolve([new TwigBranchItem(`Error fetching branch tree: ${errorMsg}`, [], true, false, false, true)]);
                            return;
                        }
                        
                        const items = parseTwigBranchTree(stdout, gitCurrentBranch);
                        
                        // Check for any local branches that might not be shown in twig tree output
                        exec('git branch --format="%(refname:short)"', { cwd: vscode.workspace.rootPath }, (gitErr, gitOutput) => {
                            if (!gitErr && gitOutput) {
                                const allLocalBranches = gitOutput.split('\n').filter(b => b.trim().length > 0);
                                const processedBranches = new Set<string>();
                                
                                // Collect all branch names that were processed by twig tree
                                const collectBranches = (items: TwigBranchItem[]) => {
                                    for (const item of items) {
                                        if (!item.isSection) {
                                            processedBranches.add(item.label);
                                        }
                                        if (item.children.length > 0) {
                                            collectBranches(item.children);
                                        }
                                    }
                                };
                                collectBranches(items);
                                
                                const missingBranches = allLocalBranches.filter(branch => !processedBranches.has(branch));
                                
                                if (missingBranches.length > 0) {
                                    const missingSection = new TwigBranchItem("🔍 Untracked Branches", [], false, false, false, true);
                                    for (const branch of missingBranches) {
                                        const isCurrentBranchItem = (currentBranch === branch) || (gitCurrentBranch === branch);
                                        missingSection.children.push(new TwigBranchItem(branch, [], false, isCurrentBranchItem, true, false, false));
                                    }
                                    items.push(missingSection);
                                }
                            }
                            
                            resolve(items);
                        });
                    });
                });
            });
        }
    }

    // Handle drag operation - when a branch is being dragged
    async handleDrag(source: TwigBranchItem[], treeDataTransfer: vscode.DataTransfer, token: vscode.CancellationToken): Promise<void> {
        // Only allow dragging single branches (not sections)
        const draggedBranches = source.filter(item => !item.isSection);
        if (draggedBranches.length === 0) {
            return;
        }

        // Store the dragged branch information
        const dragData = draggedBranches.map(item => ({
            label: item.label,
            isCurrentBranch: item.isCurrentBranch,
            isOrphaned: item.isOrphaned
        }));

        treeDataTransfer.set('application/vnd.code.tree.twigbranchtreeview', new vscode.DataTransferItem(dragData));
    }

    // Handle drop operation - when a branch is dropped onto another branch
    async handleDrop(target: TwigBranchItem | undefined, sources: vscode.DataTransfer, token: vscode.CancellationToken): Promise<void> {
        const transferItem = sources.get('application/vnd.code.tree.twigbranchtreeview');
        if (!transferItem) {
            return;
        }

        const draggedBranches = transferItem.value;
        if (!Array.isArray(draggedBranches) || draggedBranches.length === 0) {
            return;
        }

        const draggedBranch = draggedBranches[0]; // Only handle single branch for now

        // Don't allow dropping onto sections or the same branch
        if (!target || target.isSection || target.label === draggedBranch.label) {
            vscode.window.showWarningMessage('Cannot drop branch here. Please drop onto a valid target branch.');
            return;
        }

        // Don't allow dropping current branch
        if (draggedBranch.isCurrentBranch) {
            vscode.window.showWarningMessage('Cannot reparent the current branch. Please switch to another branch first.');
            return;
        }

        const childBranch = draggedBranch.label;
        const newParent = target.label;

        // Confirm the reparenting operation
        const confirmation = await vscode.window.showWarningMessage(
            `Are you sure you want to reparent "${childBranch}" to "${newParent}"?`,
            'Yes', 'No'
        );

        if (confirmation !== 'Yes') {
            return;
        }

        // Perform the reparenting
        vscode.window.withProgress({
            location: vscode.ProgressLocation.Notification,
            title: `Reparenting ${childBranch} to ${newParent}...`
        }, async () => {
            return new Promise<void>((resolve) => {
                // First, get the current tree structure to find existing parent
                exec('twig tree', { cwd: vscode.workspace.rootPath }, (treeErr, treeOutput) => {
                    if (treeErr) {
                        vscode.window.showErrorMessage(`Failed to get tree structure: ${treeErr.message}`);
                        resolve();
                        return;
                    }

                    // Parse the tree to find the current parent of the child branch
                    const currentParent = this.findCurrentParent(treeOutput, childBranch);
                    
                    // Function to add the new dependency after removing the old one
                    const addNewDependency = () => {
                        exec(`twig branch depend "${childBranch}" "${newParent}"`, { cwd: vscode.workspace.rootPath }, (err, stdout, stderr) => {
                            if (err) {
                                const errorMsg = stderr ? stderr : err.message;
                                vscode.window.showErrorMessage(`Failed to add new dependency: ${errorMsg}`);
                            } else {
                                vscode.window.showInformationMessage(`Successfully reparented "${childBranch}" to "${newParent}"`);
                                // Refresh the tree view
                                this.refresh();
                            }
                            resolve();
                        });
                    };

                    // If there's an existing parent, remove it first
                    if (currentParent && currentParent !== newParent) {
                        exec(`twig branch remove-dep "${childBranch}" "${currentParent}"`, { cwd: vscode.workspace.rootPath }, (removeErr, removeStdout, removeStderr) => {
                            if (removeErr) {
                                vscode.window.showErrorMessage(`Failed to remove existing dependency: ${removeErr.message}`);
                                resolve();
                                return;
                            }
                            // Now add the new dependency
                            addNewDependency();
                        });
                    } else {
                        // No existing parent or same parent, just add the new dependency
                        addNewDependency();
                    }
                });
            });
        });
    }

    // Helper function to find the current parent of a branch from tree output
    private findCurrentParent(treeOutput: string, branchName: string): string | null {
        const lines = treeOutput.split('\n');
        let parentStack: string[] = [];
        
        for (let i = 0; i < lines.length; i++) {
            const line = lines[i];
            
            // Skip empty lines and help text
            if (!line.trim() || line.includes('ℹ') || line.includes('To associate')) {
                continue;
            }
            
            // Clean the line of ANSI codes and tree characters
            const cleanLine = line.replace(/\x1B\[[0-9;]*m/g, '');
            const treeChars = cleanLine.match(/^[└├│─\s]*/);
            const indent = treeChars ? treeChars[0].length : 0;
            
            // Extract branch name
            let currentBranchName = cleanLine.replace(/^[└├│─\s]*/, '').trim();
            currentBranchName = currentBranchName.replace(/\s+\[.*?\]$/, '').replace(/\s+\(current\)$/, '').trim();
            
            if (!currentBranchName) continue;
            
            // Update parent stack based on indentation
            // Each level of indentation represents a parent-child relationship
            const level = Math.floor(indent / 4); // Assuming 4 spaces per indent level
            parentStack = parentStack.slice(0, level);
            
            // If this is the branch we're looking for, return its parent
            if (currentBranchName === branchName) {
                return parentStack.length > 0 ? parentStack[parentStack.length - 1] : null;
            }
            
            // Add current branch to the stack
            parentStack.push(currentBranchName);
        }
        
        return null;
    }
}

class TwigBranchItem extends vscode.TreeItem {
    constructor(
        public readonly label: string,
        public readonly children: TwigBranchItem[] = [],
        public readonly collapsible: boolean = false,
        public readonly isCurrentBranch: boolean = false,
        public readonly isOrphaned: boolean = false,
        public readonly isSection: boolean = false,
        public readonly isRootBranch: boolean = false
    ) {
        super(label, children.length > 0 ? vscode.TreeItemCollapsibleState.Expanded : vscode.TreeItemCollapsibleState.None);
        
        // Set context value for commands
        if (isSection) {
            this.contextValue = 'section';
        } else if (isRootBranch) {
            this.contextValue = 'rootBranch';
        } else {
            this.contextValue = 'branch';
        }
        
        // Add visual indicators
        if (isCurrentBranch) {
            this.iconPath = new vscode.ThemeIcon('star-full', new vscode.ThemeColor('charts.green'));
            this.description = '(current)';
            // Highlight the current branch with a different resource URI to trigger styling
            this.resourceUri = vscode.Uri.parse('twig-current-branch://current');
        } else if (isOrphaned) {
            this.iconPath = new vscode.ThemeIcon('warning', new vscode.ThemeColor('list.warningForeground'));
            this.description = '(orphaned)';
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


function updateStatusBar() {
    exec('twig current-branch', { cwd: vscode.workspace.rootPath }, (err, stdout) => {
        if (err) {
            statusBarItem.text = 'Twig: Error';
        } else {
            statusBarItem.text = `Twig: ${stdout.trim()}`;
        }
        statusBarItem.show();
    });
}

let statusBarItem: vscode.StatusBarItem;
let currentBranch: string | null = null;

function parseTwigBranchTree(output: string, gitCurrentBranch?: string | null): TwigBranchItem[] {
    const ansiRegex = /\x1B\[[0-9;]*m/g;
    const lines = output.split('\n');
    const result: TwigBranchItem[] = [];
    currentBranch = null;
    
    let i = 0;
    let orphanedBranches: string[] = [];
    let inOrphanedSection = false;
    let processedBranches = new Set<string>(); // Track all branches we've seen
    let rootBranches = new Set<string>(); // Track which branches are roots
    
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
                processedBranches.add(trimmed); // Track available branch
            }
        }
        
        if (availableBranches.length > 0) {
            const orphanedSection = new TwigBranchItem("📝 Available Branches", [], false, false, false, true);
            for (const branch of availableBranches) {
                const isCurrentBranchItem = (currentBranch === branch) || (gitCurrentBranch === branch);
                orphanedSection.children.push(new TwigBranchItem(branch, [], false, isCurrentBranchItem, true, false, false));
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
                        orphanLine.startsWith("•") && orphanLine.includes("Add as root")) {
                        break;
                    }
                    
                    // Extract branch names from bullet points
                    const match = orphanLine.match(/^• (.+)$/);
                    if (match) {
                        const orphanedBranch = match[1];
                        orphanedBranches.push(orphanedBranch);
                        processedBranches.add(orphanedBranch); // Track orphaned branch
                    }
                    i++;
                }
                break;
            }
            
            // Skip help/info lines and empty lines
            if (/^ℹ|^\u2139|To organize|To associate|Add as root|Add dependency|Link Jira|issues:|PRs:|https?:\/\//.test(trimmed)) {
                i++;
                continue;
            }
            
            // Skip warning lines
            if (/^⚠️|^Found user-defined dependencies|^Available branches:|^To fix this/.test(trimmed)) {
                i++;
                continue;
            }
            
            // Detect current branch from (current) indicator
            const currentBranchMatch = line.match(/(.+?)\s+\(current\)/);
            if (currentBranchMatch) {
                currentBranch = currentBranchMatch[1].replace(/^[└├│─\s]*/, '').trim();
            }
            
            // Parse tree structure with proper Unicode character support
            const cleanLine = line.replace(ansiRegex, '');
            // Match tree drawing characters: └── ├── │ and spaces for indentation
            const treeChars = cleanLine.match(/^[└├│─\s]*/);
            const indent = treeChars ? treeChars[0].length : 0;
            
            // Extract branch name, removing tree characters and status info
            let branchName = cleanLine.replace(/^[└├│─\s]*/, '').trim();
            
            // Remove status indicators like [up-to-date], (current), etc.
            branchName = branchName.replace(/\s+\[.*?\]$/, '').replace(/\s+\(current\)$/, '').trim();
            
            if (branchName && branchName.length > 0) {
                const isCurrentBranchItem = (currentBranch === branchName) || (gitCurrentBranch === branchName);
                // A branch is considered a root branch if it appears at the top level (indent === 0)
                const isRootBranchItem = (indent === 0);
                if (isRootBranchItem) {
                    rootBranches.add(branchName);
                }
                const item = new TwigBranchItem(branchName, [], false, isCurrentBranchItem, false, false, isRootBranchItem);
                processedBranches.add(branchName); // Track this branch
                
                // Handle hierarchy using indentation
                while (stack.length > 0 && stack[stack.length - 1].indent >= indent) {
                    stack.pop();
                }
                
                if (stack.length === 0) {
                    result.push(item);
                } else {
                    stack[stack.length - 1].item.children.push(item);
                    // Update parent's collapsible state to expanded
                    if (stack[stack.length - 1].item.collapsibleState === vscode.TreeItemCollapsibleState.None) {
                        stack[stack.length - 1].item.collapsibleState = vscode.TreeItemCollapsibleState.Expanded;
                    }
                }
                
                stack.push({item, indent});
            }
            
            i++;
        }    // Add orphaned branches section if any exist
    if (orphanedBranches.length > 0) {
        const orphanedSection = new TwigBranchItem("📝 Orphaned Branches", [], false, false, false, true);
        for (const branch of orphanedBranches) {
            const isCurrentBranchItem = (currentBranch === branch) || (gitCurrentBranch === branch);
            orphanedSection.children.push(new TwigBranchItem(branch, [], false, isCurrentBranchItem, true, false, false));
        }
        result.push(orphanedSection);
    }
    
    return result;
}

function updateStatusBarFromTree() {
    if (currentBranch) {
        statusBarItem.text = `Twig: ${currentBranch}`;
    } else {
        statusBarItem.text = 'Twig: (no branch detected)';
    }
    statusBarItem.show();
}

export function activate(context: vscode.ExtensionContext) {
    const treeDataProvider = new TwigBranchTreeProvider();
    
    // Create tree view with drag and drop support
    const treeView = vscode.window.createTreeView('twigBranchTreeView', {
        treeDataProvider,
        dragAndDropController: treeDataProvider
    });
    
    context.subscriptions.push(treeView);
    context.subscriptions.push(
        vscode.commands.registerCommand('twig.refreshBranchTree', () => {
            treeDataProvider.refresh();
            updateStatusBarFromTree();
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
        if (currentBranch === targetBranch) {
            vscode.window.showInformationMessage(`Already on branch: ${targetBranch}`);
            return;
        }

        vscode.window.withProgress({ 
            location: vscode.ProgressLocation.Notification, 
            title: `Switching to ${targetBranch}` 
        }, async () => {
            return new Promise<void>((resolve) => {
                exec(`twig switch "${targetBranch}"`, { cwd: vscode.workspace.rootPath }, (err, stdout, stderr) => {
                    if (err) {
                        vscode.window.showErrorMessage(`Failed to switch branch: ${stderr ? stderr : err.message}`);
                    } else {
                        vscode.window.showInformationMessage(`Successfully switched to branch: ${targetBranch}`);
                        // Refresh the tree view and status bar
                        treeDataProvider.refresh();
                        updateStatusBarFromTree();
                    }
                    resolve();
                });
            });
        });
    }
    context.subscriptions.push(
        vscode.commands.registerCommand('twig.createBranchFromCurrent', async () => {
            // Detect the current branch from 'twig tree' output
            exec('twig tree', { cwd: vscode.workspace.rootPath }, (err, stdout) => {
                if (err) {
                    vscode.window.showErrorMessage('Failed to fetch branch list.');
                    return;
                }
                // Find the current branch indicated by (current)
                const lines = stdout.split('\n');
                let originalBranch: string | null = null;
                
                for (const line of lines) {
                    const currentBranchMatch = line.match(/(.+?)\s+\(current\)/);
                    if (currentBranchMatch) {
                        originalBranch = currentBranchMatch[1].replace(/^[└├│─\s]*/, '').trim();
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
                    const ansiRegex = /\x1B\[[0-9;]*m/g;
                    const branchExists = stdout.split('\n')
                        .map(l => l.replace(/^[└├│─\s]*/, '').replace(ansiRegex, '').replace(/\s+\[.*?\]$/, '').replace(/\s+\(current\)$/, '').trim())
                        .filter(name => name.length > 0)
                        .includes(newBranch);
                    if (branchExists) {
                        vscode.window.showErrorMessage(`Branch '${newBranch}' already exists.`);
                        return;
                    }
                    vscode.window.withProgress({ location: vscode.ProgressLocation.Notification, title: `Creating branch ${newBranch} from ${originalBranch}` }, async () => {
                        return new Promise<void>((resolve) => {
                            // Create new branch by switching to it
                            exec(`twig switch -p "${originalBranch}" "${newBranch}"`, { cwd: vscode.workspace.rootPath }, (err1, stdout1, stderr1) => {
                                if (err1) {
                                    vscode.window.showErrorMessage(`Failed to create branch: ${stderr1 ? stderr1 : err1.message}`);
                                } else {
                                    vscode.window.showInformationMessage(`Successfully created and switched to branch: ${newBranch}`);
                                    // Refresh the tree view and status bar
                                    treeDataProvider.refresh();
                                    updateStatusBarFromTree();
                                }
                                resolve();
                            });
                        });
                    });
                });
            });
        })
    );
    context.subscriptions.push(
        vscode.commands.registerCommand('twig.createPullRequest', (item: TwigBranchItem) => {
            // If it's a section item, use current branch
            let targetBranch: string | null = null;
            
            if (item && !item.isSection) {
                // Use the selected branch name directly
                targetBranch = item.label;
            } else {
                // Use current branch if no specific branch selected
                targetBranch = currentBranch;
            }
            
            if (!targetBranch) {
                vscode.window.showErrorMessage('No branch selected for creating pull request.');
                return;
            }
            
            // Show progress notification
            vscode.window.withProgress({ 
                location: vscode.ProgressLocation.Notification, 
                title: `Creating pull request for ${targetBranch}...`,
                cancellable: true
            }, async (progress, token) => {
                return new Promise<void>((resolve) => {
                    const createPrProcess = exec(`twig github pr create-pr`, { 
                        cwd: vscode.workspace.rootPath,
                        timeout: 30000 // 30 second timeout
                    }, (err, stdout, stderr) => {
                        if (token.isCancellationRequested) {
                            resolve();
                            return;
                        }
                        
                        if (err) {
                            const errorMsg = stderr ? stderr : err.message;
                            
                            // Check for common authentication errors
                            if (errorMsg.includes('authentication') || errorMsg.includes('token') || errorMsg.includes('credential')) {
                                vscode.window.showErrorMessage(
                                    'GitHub authentication required. Please set up your GitHub credentials.',
                                    'Learn More'
                                ).then(selection => {
                                    if (selection === 'Learn More') {
                                        vscode.env.openExternal(vscode.Uri.parse('https://docs.github.com/en/authentication'));
                                    }
                                });
                            } else if (errorMsg.includes('not found') || errorMsg.includes('repository')) {
                                vscode.window.showErrorMessage('This repository may not be connected to GitHub or the remote is not accessible.');
                            } else {
                                vscode.window.showErrorMessage(`Failed to create pull request: ${errorMsg}`);
                            }
                        } else {
                            // Parse output to look for PR URL
                            const prUrlMatch = stdout.match(/https:\/\/github\.com\/[^\s]+\/pull\/\d+/);
                            if (prUrlMatch) {
                                const prUrl = prUrlMatch[0];
                                vscode.window.showInformationMessage(
                                    `Successfully created pull request for ${targetBranch}`,
                                    'Open PR'
                                ).then(selection => {
                                    if (selection === 'Open PR') {
                                        vscode.env.openExternal(vscode.Uri.parse(prUrl));
                                    }
                                });
                            } else {
                                vscode.window.showInformationMessage(`Successfully created pull request for ${targetBranch}`);
                            }
                            
                            // Refresh the tree view
                            treeDataProvider.refresh();
                        }
                        resolve();
                    });
                    
                    // Handle cancellation
                    token.onCancellationRequested(() => {
                        createPrProcess.kill();
                    });
                });
            });
        })
    );
    context.subscriptions.push(
        vscode.commands.registerCommand('twig.reparentToRoot', (item: TwigBranchItem) => {
            // If it's a section item, don't allow reparenting
            if (item && item.isSection) {
                vscode.window.showInformationMessage('Cannot reparent a section. Please select a branch.');
                return;
            }
            
            if (!item || !item.label) {
                vscode.window.showErrorMessage('No branch selected for reparenting.');
                return;
            }
            
            const branchName = item.label;
            
            // First, get the list of root branches
            exec('twig branch root list', { cwd: vscode.workspace.rootPath }, (err, stdout, stderr) => {
                if (err) {
                    vscode.window.showErrorMessage(`Failed to get root branches: ${stderr || err.message}`);
                    return;
                }
                
                // Parse root branches from output
                const lines = stdout.split('\n');
                const rootBranches: string[] = [];
                let inRootSection = false;
                
                for (const line of lines) {
                    const trimmed = line.trim();
                    if (trimmed.includes('Root branches:')) {
                        inRootSection = true;
                        continue;
                    }
                    if (inRootSection && trimmed.length > 0) {
                        // Look for lines that start with Unicode info character or whitespace and contain branch names
                        const branchMatch = trimmed.match(/^(?:ℹ\s*|•\s*)?(.+)$/);
                        if (branchMatch && branchMatch[1].trim().length > 0) {
                            const branchName = branchMatch[1].trim();
                            // Skip lines that are just help text or empty
                            if (!branchName.includes(':') && !branchName.startsWith('To ') && !branchName.startsWith('•')) {
                                rootBranches.push(branchName);
                            }
                        }
                    }
                }
                
                if (rootBranches.length === 0) {
                    vscode.window.showErrorMessage('No root branches found. Please add a root branch first using "twig branch root add <branch-name>".');
                    return;
                }
                
                // Function to perform the reparenting
                const performReparent = (targetRoot: string) => {
                    vscode.window.showWarningMessage(
                        `Are you sure you want to reparent "${branchName}" to "${targetRoot}"? This will remove its current parent dependency.`,
                        'Yes', 'No'
                    ).then(selection => {
                        if (selection === 'Yes') {
                            vscode.window.withProgress({ 
                                location: vscode.ProgressLocation.Notification, 
                                title: `Reparenting ${branchName} to ${targetRoot}...` 
                            }, async () => {
                                return new Promise<void>((resolve) => {
                                    exec(`twig branch depend "${branchName}" "${targetRoot}"`, { cwd: vscode.workspace.rootPath }, (err, stdout, stderr) => {
                                        if (err) {
                                            const errorMsg = stderr ? stderr : err.message;
                                            vscode.window.showErrorMessage(`Failed to reparent branch: ${errorMsg}`);
                                        } else {
                                            vscode.window.showInformationMessage(`Successfully reparented "${branchName}" to "${targetRoot}"`);
                                            // Refresh the tree view
                                            treeDataProvider.refresh();
                                        }
                                        resolve();
                                    });
                                });
                            });
                        }
                    });
                };
                
                if (rootBranches.length === 1) {
                    // Only one root branch, use it directly
                    performReparent(rootBranches[0]);
                } else {
                    // Multiple root branches, let user choose
                    vscode.window.showQuickPick(rootBranches, {
                        placeHolder: `Select root branch to reparent "${branchName}" to:`
                    }).then(selectedRoot => {
                        if (selectedRoot) {
                            performReparent(selectedRoot);
                        }
                    });
                }
            });
        })
    );
    context.subscriptions.push(
        vscode.commands.registerCommand('twig.deleteBranch', (item: TwigBranchItem) => {
            // If it's a section item, don't allow deletion
            if (item && item.isSection) {
                vscode.window.showInformationMessage('Cannot delete a section. Please select a branch.');
                return;
            }
            
            if (!item || !item.label) {
                vscode.window.showErrorMessage('No branch selected for deletion.');
                return;
            }
            
            const branchName = item.label;
            
            // Don't allow deleting the current branch
            if (item.isCurrentBranch) {
                vscode.window.showErrorMessage('Cannot delete the current branch. Please switch to another branch first.');
                return;
            }
            
            // Confirm the dangerous action
            vscode.window.showWarningMessage(
                `Are you sure you want to permanently delete branch "${branchName}"? This action cannot be undone.`,
                { modal: true },
                'Delete Branch', 'Cancel'
            ).then(selection => {
                if (selection === 'Delete Branch') {
                    vscode.window.withProgress({ 
                        location: vscode.ProgressLocation.Notification, 
                        title: `Deleting branch ${branchName}...` 
                    }, async () => {
                        return new Promise<void>((resolve) => {
                            // First remove from twig configuration, then delete the git branch
                            exec(`git branch -D "${branchName}"`, { cwd: vscode.workspace.rootPath }, (err, stdout, stderr) => {
                                if (err) {
                                    const errorMsg = stderr ? stderr : err.message;
                                    // Check if it's because the branch has unmerged changes
                                    if (errorMsg.includes('not fully merged')) {
                                        vscode.window.showErrorMessage(
                                            `Branch "${branchName}" has unmerged changes. Use 'git branch -D' if you want to force delete.`,
                                            'Force Delete', 'Cancel'
                                        ).then(forceSelection => {
                                            if (forceSelection === 'Force Delete') {
                                                exec(`git branch -D "${branchName}"`, { cwd: vscode.workspace.rootPath }, (forceErr, forceStdout, forceStderr) => {
                                                    if (forceErr) {
                                                        vscode.window.showErrorMessage(`Failed to force delete branch: ${forceStderr ? forceStderr : forceErr.message}`);
                                                    } else {
                                                        // Clean up twig configuration
                                                        exec(`twig tidy prune -f`, { cwd: vscode.workspace.rootPath }, () => {
                                                            vscode.window.showInformationMessage(`Successfully deleted branch "${branchName}"`);
                                                            treeDataProvider.refresh();
                                                        });
                                                    }
                                                });
                                            }
                                        });
                                    } else {
                                        vscode.window.showErrorMessage(`Failed to delete branch: ${errorMsg}`);
                                    }
                                } else {
                                    // Clean up twig configuration after successful deletion
                                    exec(`twig tidy prune -f`, { cwd: vscode.workspace.rootPath }, () => {
                                        vscode.window.showInformationMessage(`Successfully deleted branch "${branchName}"`);
                                        treeDataProvider.refresh();
                                    });
                                }
                                resolve();
                            });
                        });
                    });
                }
            });
        })
    );
    context.subscriptions.push(
        vscode.commands.registerCommand('twig.removeAsRoot', (item: TwigBranchItem) => {
            // If it's a section item, don't allow removing as root
            if (item && item.isSection) {
                vscode.window.showInformationMessage('Cannot remove a section as root. Please select a branch.');
                return;
            }
            
            if (!item || !item.label) {
                vscode.window.showErrorMessage('No branch selected for removing as root.');
                return;
            }
            
            if (!item.isRootBranch) {
                vscode.window.showInformationMessage(`Branch "${item.label}" is not currently marked as a root branch.`);
                return;
            }
            
            const branchName = item.label;
            
            // Confirm the action
            vscode.window.showWarningMessage(
                `Are you sure you want to remove "${branchName}" as a root branch?`,
                'Yes', 'No'
            ).then(selection => {
                if (selection === 'Yes') {
                    vscode.window.withProgress({ 
                        location: vscode.ProgressLocation.Notification, 
                        title: `Removing ${branchName} as root branch...` 
                    }, async () => {
                        return new Promise<void>((resolve) => {
                            exec(`twig branch root remove "${branchName}"`, { cwd: vscode.workspace.rootPath }, (err, stdout, stderr) => {
                                if (err) {
                                    const errorMsg = stderr ? stderr : err.message;
                                    vscode.window.showErrorMessage(`Failed to remove branch as root: ${errorMsg}`);
                                } else {
                                    vscode.window.showInformationMessage(`Successfully removed "${branchName}" as root branch`);
                                    // Refresh the tree view
                                    treeDataProvider.refresh();
                                }
                                resolve();
                            });
                        });
                    });
                }
            });
        })
    );
    statusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
    context.subscriptions.push(statusBarItem);
    updateStatusBarFromTree();
    vscode.workspace.onDidChangeWorkspaceFolders(updateStatusBarFromTree);
    vscode.workspace.onDidChangeConfiguration(updateStatusBarFromTree);
}

export function deactivate() {}
