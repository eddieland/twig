// Debug the TwigBranchItem creation
console.log('Testing TwigBranchItem creation...');

// Simulate the constructor logic
function createTestItem(label, commitStatus) {
    console.log(`Input: label="${label}", commitStatus="${commitStatus}"`);
    
    // Create display label with commit status at the beginning
    let displayLabel = label;
    if (commitStatus && commitStatus !== undefined) {
        displayLabel = `${commitStatus} ${label}`;
    }
    
    console.log(`Output displayLabel: "${displayLabel}"`);
    return displayLabel;
}

// Test with sample data
createTestItem('experimental/bkrupa', '[+4]');
createTestItem('test-branch', '[+2]');
createTestItem('main', undefined);
createTestItem('another-root', undefined);