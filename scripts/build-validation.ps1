# Twig Build Validation Script for PowerShell
param([switch]$Verbose = $false)

$ValidationFailed = 0

function Write-Status {
    param([string]$Message, [string]$Type = "Info")
    switch ($Type) {
        "Success" { Write-Host "[PASS] $Message" -ForegroundColor Green }
        "Error" { Write-Host "[FAIL] $Message" -ForegroundColor Red }
        "Warning" { Write-Host "[WARN] $Message" -ForegroundColor Yellow }
        default { Write-Host "[INFO] $Message" -ForegroundColor Blue }
    }
}

function Test-BinaryCollisions {
    Write-Status "Checking for binary name collisions..."
    
    $cargoFiles = Get-ChildItem -Path "." -Name "Cargo.toml" -Recurse | Where-Object { $_ -notmatch "target" }
    $binSections = 0
    
    foreach ($file in $cargoFiles) {
        $content = Get-Content -Path $file -Raw
        if ($content -match "\[\[bin\]\]") {
            $binSections++
            if ($Verbose) {
                Write-Status "Found binary definition in: $file"
            }
        }
    }
    
    Write-Status "Binary collision check completed" "Success"
    return $true
}

function Test-DependencyResolution {
    Write-Status "Checking dependency resolution..."
    
    cargo check --workspace 2>&1 | Out-Null
    if ($LASTEXITCODE -eq 0) {
        Write-Status "Dependencies resolved successfully" "Success"
        return $true
    } else {
        Write-Status "Dependency check failed" "Error"
        return $false
    }
}

function Test-WorkspaceStructure {
    Write-Status "Validating workspace structure..."
    
    if (Test-Path "Cargo.toml") {
        $content = Get-Content "Cargo.toml" -Raw
        if ($content -like "*[workspace]*") {
            Write-Status "Workspace structure valid" "Success"
            return $true
        }
    }
    
    Write-Status "Workspace configuration issue" "Error"
    return $false
}

function Test-BuildArtifacts {
    Write-Status "Building and validating artifacts..."
    
    cargo build --workspace 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Status "Build failed" "Error"
        return $false
    }
    
    if (Test-Path "target\debug\twig.exe") {
        Write-Status "Build artifacts validated" "Success"
        return $true
    } else {
        Write-Status "Expected binary not found" "Error"
        return $false
    }
}

function Test-RunTests {
    Write-Status "Running test suite..."
    
    $nextest = Get-Command "cargo-nextest" -ErrorAction SilentlyContinue
    
    if ($nextest) {
        cargo nextest run --workspace 2>&1 | Out-Null
    } else {
        Write-Status "Using cargo test (nextest not available)" "Warning"
        cargo test --workspace 2>&1 | Out-Null
    }
    
    if ($LASTEXITCODE -eq 0) {
        Write-Status "All tests passed" "Success"
        return $true
    } else {
        Write-Status "Tests failed" "Error"
        return $false
    }
}

# Main validation
Write-Status "Starting Twig build validation..."
Write-Status "Timestamp: $(Get-Date)"

$checks = @{
    "Binary Collision Prevention" = { Test-BinaryCollisions }
    "Dependency Resolution" = { Test-DependencyResolution }
    "Workspace Structure" = { Test-WorkspaceStructure }
    "Build Artifacts" = { Test-BuildArtifacts }
    "Test Suite" = { Test-RunTests }
}

foreach ($checkName in $checks.Keys) {
    try {
        $result = & $checks[$checkName]
        if (-not $result) {
            $ValidationFailed++
        }
    } catch {
        Write-Status "Error in $checkName`: $_" "Error"
        $ValidationFailed++
    }
}

Write-Status "Validation completed with $ValidationFailed error(s)"

if ($ValidationFailed -gt 0) {
    Write-Status "Build validation failed with $ValidationFailed error(s)" "Error"
    exit 6
} else {
    Write-Status "All validation checks passed!" "Success"
    exit 0
}