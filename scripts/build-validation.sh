#!/bin/bash
#
# Build Validation Script
# Comprehensive validation for twig build system including binary collision prevention,
# dependency validation, and build artifact verification.
#

set -euo pipefail

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Exit codes
EXIT_SUCCESS=0
EXIT_BUILD_FAILED=1
EXIT_BINARY_COLLISION=2
EXIT_DEPENDENCY_ISSUE=3
EXIT_ARTIFACT_VALIDATION_FAILED=4

# Validation counters
VALIDATION_CHECKS=0
VALIDATION_PASSED=0
VALIDATION_FAILED=0

# Function to run a validation check
run_check() {
    local name="$1"
    local command="$2"
    
    VALIDATION_CHECKS=$((VALIDATION_CHECKS + 1))
    log_info "Running validation: $name"
    
    if eval "$command"; then
        log_success "✓ $name passed"
        VALIDATION_PASSED=$((VALIDATION_PASSED + 1))
        return 0
    else
        log_error "✗ $name failed"
        VALIDATION_FAILED=$((VALIDATION_FAILED + 1))
        return 1
    fi
}

# Binary collision detection
check_binary_collisions() {
    log_info "Checking for binary name collisions..."
    
    # Extract all binary definitions from workspace Cargo.toml files
    local binaries=$(find . -name "Cargo.toml" -not -path "./target/*" -exec grep -l "\[\[bin\]\]" {} \; | \
        xargs -I {} sh -c 'echo "File: {}"; grep -A 2 "\[\[bin\]\]" "{}" || true')
    
    if [[ -n "$binaries" ]]; then
        echo "$binaries"
        
        # Extract binary names and check for duplicates
        local binary_names=$(find . -name "Cargo.toml" -not -path "./target/*" -exec grep -A 2 "\[\[bin\]\]" {} \; | \
            grep "name =" | cut -d'"' -f2 | sort)
        
        local duplicates=$(echo "$binary_names" | uniq -d)
        
        if [[ -n "$duplicates" ]]; then
            log_error "Binary name collision detected:"
            echo "$duplicates"
            return 1
        fi
    fi
    
    # Check for duplicate binary names in package definitions
    local package_bins=$(find . -name "Cargo.toml" -not -path "./target/*" -exec grep -H "name.*=.*\"twig\"" {} \;)
    if [[ $(echo "$package_bins" | wc -l) -gt 1 ]] && [[ -n "$package_bins" ]]; then
        log_warning "Multiple packages might produce 'twig' binary:"
        echo "$package_bins"
    fi
    
    return 0
}

# Dependency validation
check_dependencies() {
    log_info "Validating dependencies..."
    
    # Check for dependency version conflicts
    cargo tree --duplicates --workspace > /dev/null 2>&1 || {
        log_warning "Dependency conflicts detected, checking details..."
        cargo tree --duplicates --workspace || true
    }
    
    # Check for outdated dependencies (if cargo-outdated is available)
    if command -v cargo-outdated >/dev/null 2>&1; then
        local outdated=$(cargo outdated --workspace --exit-code 1 2>/dev/null || echo "outdated")
        if [[ "$outdated" == "outdated" ]]; then
            log_warning "Some dependencies are outdated (run 'cargo outdated' for details)"
        fi
    fi
    
    # Verify workspace dependencies are consistent
    log_info "Checking workspace dependency consistency..."
    local workspace_deps=$(grep -r "\.workspace = true" */Cargo.toml | cut -d: -f1 | sort | uniq)
    for file in $workspace_deps; do
        log_info "Workspace dependencies found in: $file"
    done
    
    return 0
}

# Build artifact validation
check_build_artifacts() {
    log_info "Validating build artifacts..."
    
    # Check if all expected binaries are built
    local expected_binary="target/debug/twig"
    if [[ ! -f "$expected_binary" ]]; then
        log_error "Expected binary not found: $expected_binary"
        return 1
    fi
    
    # Verify binary is executable and not corrupted
    if [[ -x "$expected_binary" ]]; then
        # Try to get version to ensure binary works
        if "$expected_binary" --version >/dev/null 2>&1; then
            log_success "Binary $expected_binary is valid and executable"
        else
            log_error "Binary $expected_binary exists but failed version check"
            return 1
        fi
    else
        log_error "Binary $expected_binary is not executable"
        return 1
    fi
    
    # Check binary size (warn if extremely large)
    local binary_size=$(stat -f%z "$expected_binary" 2>/dev/null || stat -c%s "$expected_binary" 2>/dev/null || echo "0")
    local size_mb=$((binary_size / 1024 / 1024))
    
    log_info "Binary size: ${size_mb}MB"
    if [[ $size_mb -gt 100 ]]; then
        log_warning "Binary size is quite large (${size_mb}MB), consider optimizing"
    fi
    
    return 0
}

# Workspace validation
check_workspace_structure() {
    log_info "Validating workspace structure..."
    
    # Check that workspace members are properly defined
    local workspace_members=$(grep -A 20 "members = \[" Cargo.toml | grep '"' | tr -d '",' | tr -d ' ')
    
    for member in $workspace_members; do
        if [[ -d "$member" ]] && [[ -f "$member/Cargo.toml" ]]; then
            log_success "Workspace member valid: $member"
        else
            log_error "Workspace member missing or invalid: $member"
            return 1
        fi
    done
    
    # Verify no orphaned Cargo.toml files
    local all_toml_dirs=$(find . -name "Cargo.toml" -not -path "./target/*" -exec dirname {} \; | sort)
    local workspace_root="."
    
    for dir in $all_toml_dirs; do
        if [[ "$dir" != "$workspace_root" ]]; then
            local rel_dir=${dir#./}
            if ! echo "$workspace_members" | grep -q "^$rel_dir$"; then
                log_warning "Potential orphaned package not in workspace: $dir"
            fi
        fi
    done
    
    return 0
}

# Test validation
check_test_status() {
    log_info "Validating test status..."
    
    # Run cargo check first to catch compilation errors
    cargo check --workspace --quiet || {
        log_error "Compilation check failed"
        return 1
    }
    
    # Run tests with nextest if available, otherwise use cargo test
    if command -v cargo-nextest >/dev/null 2>&1; then
        cargo nextest run --workspace --no-run >/dev/null 2>&1 || {
            log_error "Test compilation failed"
            return 1
        }
        log_success "All tests compile successfully"
    else
        cargo test --workspace --no-run >/dev/null 2>&1 || {
            log_error "Test compilation failed"
            return 1
        }
        log_success "All tests compile successfully"
    fi
    
    return 0
}

# Clippy validation (code quality)
check_code_quality() {
    log_info "Validating code quality with clippy..."
    
    # Run clippy with strict settings
    cargo clippy --workspace --all-targets --all-features -- -D warnings >/dev/null 2>&1 || {
        log_warning "Clippy warnings found (run 'make lint' for details)"
        return 1
    }
    
    log_success "Code quality validation passed"
    return 0
}

# Release build validation
check_release_build() {
    log_info "Validating release build..."
    
    # Build release version
    cargo build --release --workspace >/dev/null 2>&1 || {
        log_error "Release build failed"
        return 1
    }
    
    # Check release binary
    local release_binary="target/release/twig"
    if [[ -f "$release_binary" ]] && [[ -x "$release_binary" ]]; then
        if "$release_binary" --version >/dev/null 2>&1; then
            log_success "Release binary is valid"
            
            # Compare sizes
            local debug_size=$(stat -f%z "target/debug/twig" 2>/dev/null || stat -c%s "target/debug/twig" 2>/dev/null || echo "0")
            local release_size=$(stat -f%z "$release_binary" 2>/dev/null || stat -c%s "$release_binary" 2>/dev/null || echo "0")
            
            local debug_mb=$((debug_size / 1024 / 1024))
            local release_mb=$((release_size / 1024 / 1024))
            
            log_info "Debug binary: ${debug_mb}MB, Release binary: ${release_mb}MB"
            
            if [[ $release_size -lt $debug_size ]]; then
                log_success "Release binary is optimized (smaller than debug)"
            fi
        else
            log_error "Release binary failed version check"
            return 1
        fi
    else
        log_error "Release binary not found or not executable"
        return 1
    fi
    
    return 0
}

# Main validation function
main() {
    log_info "Starting comprehensive build validation for twig project"
    log_info "=================================================="
    
    # Ensure we have the necessary tools
    if ! command -v cargo >/dev/null 2>&1; then
        log_error "cargo not found in PATH"
        exit 1
    fi
    
    # Build the project first
    log_info "Building project..."
    cargo build --workspace || {
        log_error "Initial build failed"
        exit $EXIT_BUILD_FAILED
    }
    
    # Run all validation checks (continue even if some fail)
    run_check "Binary Collision Detection" "check_binary_collisions"
    run_check "Workspace Structure" "check_workspace_structure" 
    run_check "Dependency Validation" "check_dependencies"
    run_check "Test Compilation" "check_test_status"
    run_check "Build Artifact Validation" "check_build_artifacts"
    
    # Optional checks (warnings only)
    run_check "Code Quality (Clippy)" "check_code_quality" || true
    run_check "Release Build Validation" "check_release_build" || true
    
    # Summary
    log_info "=================================================="
    log_info "Build validation complete"
    log_info "Total checks: $VALIDATION_CHECKS"
    log_success "Passed: $VALIDATION_PASSED"
    
    if [[ $VALIDATION_FAILED -gt 0 ]]; then
        log_error "Failed: $VALIDATION_FAILED"
        log_error "Build validation failed with $VALIDATION_FAILED error(s)"
        exit $EXIT_ARTIFACT_VALIDATION_FAILED
    else
        log_success "All validation checks passed!"
        exit $EXIT_SUCCESS
    fi
}

# Run main function
main "$@"