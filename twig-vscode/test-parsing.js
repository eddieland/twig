// Simple test to verify the parsing logic works with the tree output
const testOutput = `master
└── feature/authentication                                      [up-to-date]
    └── feature/user-profile                                    [up-to-date]
        └── feature/settings (current)                          [up-to-date]

📝 Orphaned branches (no dependencies defined):
  • feature/orphaned-branch

To organize these branches:
  • Add as root: twig branch root add <branch-name>
  • Add dependency: twig branch depend <child-branch> <parent-branch>
  • Reparent all to one branch: twig branch reparent <parent-branch>

ℹ To associate branches with issues and PRs:
  • Link Jira issues: twig jira branch link <issue-key>`;

console.log("Testing tree parsing with updated output:");
console.log(testOutput);
console.log("\n--- Parsing results ---");

const lines = testOutput.split('\n');
const ansiRegex = /\x1B\[[0-9;]*m/g;
let currentBranch = null;
let processedBranches = new Set();
let orphanedBranches = [];
let inOrphanedSection = false;

for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const trimmed = line.trim();
    
    // Skip empty lines and help text
    if (!trimmed || /^ℹ|^\u2139|To organize|To associate|Add as root|Add dependency|Link Jira/.test(trimmed)) {
        continue;
    }
    
    // Check for orphaned branches section
    if (trimmed.includes("📝 Orphaned branches")) {
        inOrphanedSection = true;
        console.log("Found orphaned branches section");
        continue;
    }
    
    if (inOrphanedSection) {
        // Stop processing orphaned section when we hit help text
        if (trimmed.startsWith("To organize") || trimmed.startsWith("ℹ")) {
            inOrphanedSection = false;
            continue;
        }
        
        // Extract branch names from bullet points
        const match = trimmed.match(/^• (.+)$/);
        if (match) {
            const orphanedBranch = match[1];
            orphanedBranches.push(orphanedBranch);
            processedBranches.add(orphanedBranch);
            console.log(`Orphaned branch: "${orphanedBranch}"`);
        }
        continue;
    }
    
    // Detect current branch
    const currentBranchMatch = line.match(/(.+?)\s+\(current\)/);
    if (currentBranchMatch) {
        currentBranch = currentBranchMatch[1].replace(/^[└├│─\s]*/, '').trim();
    }
    
    // Parse tree structure
    const cleanLine = line.replace(ansiRegex, '');
    const treeChars = cleanLine.match(/^[└├│─\s]*/);
    const indent = treeChars ? treeChars[0].length : 0;
    
    let branchName = cleanLine.replace(/^[└├│─\s]*/, '').trim();
    branchName = branchName.replace(/\s+\[.*?\]$/, '').replace(/\s+\(current\)$/, '').trim();
    
    if (branchName && branchName.length > 0) {
        const isCurrentBranchItem = currentBranch === branchName;
        processedBranches.add(branchName);
        console.log(`Branch: "${branchName}", Indent: ${indent}, Current: ${isCurrentBranchItem}`);
    }
}

console.log(`\nDetected current branch: "${currentBranch}"`);
console.log(`Orphaned branches found: ${orphanedBranches.length} (${orphanedBranches.join(', ')})`);
console.log(`Total processed branches: ${processedBranches.size}`);