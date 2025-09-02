// Simple test to verify the tree parsing logic
const testOutput = `main (current)
├── new-feature
│   └── another-feature
│       └── third-feature
│           └── debug-test
└── debug-test2

📝 Orphaned branches (no dependencies defined):
  • error-test
  • test-branch

To organize these branches:
  • Add as root: twig branch root add <branch-name>
  • Add dependency: twig branch depend <parent-branch>

ℹ To associate branches with issues and PRs:
  • Link Jira issues: twig jira branch link <issue-key>
  • Link GitHub PRs: twig github pr link <pr-url>`;

// Mock TwigBranchItem class for testing
class TwigBranchItem {
    constructor(label, children = [], collapsible = false, isCurrentBranch = false, isOrphaned = false, isSection = false) {
        this.label = label;
        this.children = children;
        this.collapsible = collapsible;
        this.isCurrentBranch = isCurrentBranch;
        this.isOrphaned = isOrphaned;
        this.isSection = isSection;
        this.contextValue = isSection ? 'section' : 'branch';
    }
}

// Mock vscode module for testing
const vscode = {
    TreeItemCollapsibleState: {
        None: 0,
        Collapsed: 1,
        Expanded: 2
    },
    ThemeIcon: function(name, color) {
        return { name, color };
    },
    ThemeColor: function(color) {
        return { color };
    }
};

let currentBranch = null;

function parseTwigBranchTree(output) {
    const ansiRegex = /\x1B\[[0-9;]*m/g;
    const lines = output.split('\n');
    const result = [];
    currentBranch = null;
    
    let i = 0;
    let orphanedBranches = [];
    let inOrphanedSection = false;
    
    // Handle JIRA_HOST error gracefully
    if (output.includes("Jira host environment variable 'JIRA_HOST' not set")) {
        result.push(new TwigBranchItem("Error: JIRA_HOST environment variable not set. Please set JIRA_HOST and reload VS Code.", [], true, false, false, true));
        return result;
    }
    
    // Handle case where no root branches are defined
    if (output.includes("Found user-defined dependencies but no root branches")) {
        const availableBranches = [];
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
    const stack = [];
    
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
        
        // Detect current branch (with "(current)" suffix)
        if (trimmed.includes("(current)")) {
            currentBranch = trimmed.replace("(current)", "").trim();
        }
        
        // Parse tree structure
        const cleanLine = line.replace(ansiRegex, '');
        const treeChars = cleanLine.match(/^[│├└─\s]*/);
        const indent = treeChars ? treeChars[0].length : 0;
        const branchName = cleanLine.replace(/^[│├└─\s]*/, '').replace("(current)", "").trim();
        
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

// Test the parsing
console.log("Testing tree parsing...");
const parsed = parseTwigBranchTree(testOutput);

function printTree(items, indent = 0) {
    for (const item of items) {
        const prefix = "  ".repeat(indent);
        const marker = item.isCurrentBranch ? " (CURRENT)" : item.isOrphaned ? " (ORPHANED)" : item.isSection ? " (SECTION)" : "";
        console.log(`${prefix}- ${item.label}${marker}`);
        if (item.children && item.children.length > 0) {
            printTree(item.children, indent + 1);
        }
    }
}

printTree(parsed);
console.log(`\nCurrent branch detected: ${currentBranch}`);
