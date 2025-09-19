// Test the parsing logic with actual twig tree output
const testOutput = `another-root

main
└── experimental/bkrupa                                [+4]
    └── test-branch (current)                          [+2]

ℹ To associate branches with issues and PRs:
  • Link Jira issues: twig jira branch link <issue-key>`;

console.log('Testing commit status parsing...');

const lines = testOutput.split('\n');
for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    console.log(`Line ${i}: "${line}"`);
    
    // Extract commit status (e.g., [+3/-1], [up-to-date], [-3])
    const statusMatch = line.match(/\s+(\[.*?\])$/);
    if (statusMatch) {
        console.log(`  Found commit status: ${statusMatch[1]}`);
    }
    
    // Extract branch name
    const cleanLine = line.replace(/\x1B\[[0-9;]*m/g, '');
    const fullBranchLine = cleanLine.replace(/^[└├│─\s]*/, '').trim();
    const branchName = fullBranchLine.replace(/\s+\[.*?\]$/, '').replace(/\s+\(current\)$/, '').trim();
    
    if (branchName && branchName.length > 0 && !branchName.startsWith('ℹ')) {
        console.log(`  Branch name: "${branchName}"`);
        console.log(`  Full line after tree chars: "${fullBranchLine}"`);
    }
}