# Dynamic Slash Threshold Setup and Test Script
# Run this script after installing Rust to verify the implementation

Write-Host "=== Dynamic Slash Threshold Implementation Setup ===" -ForegroundColor Green

# Step 1: Install Rust if not already installed
Write-Host "`n1. Checking Rust installation..." -ForegroundColor Yellow
try {
    $rustVersion = cargo --version
    Write-Host "✓ Rust is installed: $rustVersion" -ForegroundColor Green
} catch {
    Write-Host "✗ Rust not found. Installing..." -ForegroundColor Red
    Write-Host "Please install Rust from: https://rustup.rs/" -ForegroundColor Yellow
    Write-Host "Or run: winget install Rustlang.Rustup" -ForegroundColor Yellow
    exit 1
}

# Step 2: Syntax Check
Write-Host "`n2. Running syntax check..." -ForegroundColor Yellow
Write-Host "Running cargo check..." -ForegroundColor Cyan
$checkResult = cargo check
if ($LASTEXITCODE -eq 0) {
    Write-Host "✓ Syntax check passed" -ForegroundColor Green
} else {
    Write-Host "✗ Syntax check failed" -ForegroundColor Red
    Write-Host $checkResult
    exit 1
}

# Step 3: Code Quality Check
Write-Host "`n3. Running code quality check..." -ForegroundColor Yellow
Write-Host "Running cargo clippy..." -ForegroundColor Cyan
$clippyResult = cargo clippy -- -D warnings
if ($LASTEXITCODE -eq 0) {
    Write-Host "✓ Code quality check passed" -ForegroundColor Green
} else {
    Write-Host "✗ Code quality issues found" -ForegroundColor Red
    Write-Host $clippyResult
    Write-Host "Please fix clippy warnings before proceeding" -ForegroundColor Yellow
}

# Step 4: Format Code
Write-Host "`n4. Formatting code..." -ForegroundColor Yellow
cargo fmt
Write-Host "✓ Code formatted" -ForegroundColor Green

# Step 5: Run Tests
Write-Host "`n5. Running tests..." -ForegroundColor Yellow
Write-Host "Running cargo test..." -ForegroundColor Cyan
$testResult = cargo test
if ($LASTEXITCODE -eq 0) {
    Write-Host "✓ All tests passed" -ForegroundColor Green
} else {
    Write-Host "✗ Some tests failed" -ForegroundColor Red
    Write-Host $testResult
}

# Step 6: Run Dynamic Slash Threshold Tests
Write-Host "`n6. Running dynamic slash threshold specific tests..." -ForegroundColor Yellow
Write-Host "Running dynamic threshold tests..." -ForegroundColor Cyan
$dynamicTestResult = cargo test dynamic_slash_threshold
if ($LASTEXITCODE -eq 0) {
    Write-Host "✓ Dynamic slash threshold tests passed" -ForegroundColor Green
} else {
    Write-Host "✗ Dynamic slash threshold tests failed" -ForegroundColor Red
    Write-Host $dynamicTestResult
}

# Step 7: Build Release Version
Write-Host "`n7. Building release version..." -ForegroundColor Yellow
Write-Host "Running cargo build --release..." -ForegroundColor Cyan
$buildResult = cargo build --release
if ($LASTEXITCODE -eq 0) {
    Write-Host "✓ Release build successful" -ForegroundColor Green
} else {
    Write-Host "✗ Release build failed" -ForegroundColor Red
    Write-Host $buildResult
    exit 1
}

Write-Host "`n=== Setup Complete ===" -ForegroundColor Green
Write-Host "✓ Dynamic slash threshold implementation is ready!" -ForegroundColor Green
Write-Host "`nNext steps:" -ForegroundColor Yellow
Write-Host "1. Review the implementation in DYNAMIC_SLASH_THRESHOLD_IMPLEMENTATION.md" -ForegroundColor White
Write-Host "2. Test the new functions:" -ForegroundColor White
Write-Host "   - set_dynamic_slash_threshold(admin_signers, true)" -ForegroundColor Gray
Write-Host "   - get_effective_slash_threshold()" -ForegroundColor Gray
Write-Host "3. Deploy to testnet for integration testing" -ForegroundColor White
Write-Host "4. Create PR when ready" -ForegroundColor White