#!/usr/bin/env bash

# Local CI check script - runs all checks that run in GitHub Actions CI
# Run this before pushing to catch issues early.

set -e

echo "🔍 Running local CI checks..."
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Track overall success across all checks. run_check intentionally swallows
# individual failures so every check runs in one pass — you see every
# problem at once instead of fixing-and-rerunning N times.
FAILED=0

run_check() {
    echo -e "${YELLOW}▶ $1${NC}"
    if eval "$2"; then
        echo -e "${GREEN}✓ $1 passed${NC}"
        echo ""
    else
        echo -e "${RED}✗ $1 failed${NC}"
        echo ""
        FAILED=1
    fi
}

# Rust checks
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "🦀 Rust Checks"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
cd server
run_check "Rust formatting" "cargo fmt --all -- --check"
run_check "Rust linting (clippy)" "cargo clippy --all-targets --all-features -- -D warnings"
run_check "Rust tests" "cargo test --all-features"
cd ..

# Frontend checks
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "⚛️  Frontend Checks"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
run_check "Frontend formatting (dprint)" "dprint check"
run_check "Frontend linting (ESLint)" "bun run lint"
# `bun test` uses Bun's built-in runner (no jsdom); `bun run test` invokes the
# npm script which runs vitest with the jsdom env. Match what .github/workflows/ci.yml does.
run_check "Frontend tests" "bun run test"

# Typos check
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📝 Spell Check"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if command -v typos &> /dev/null; then
    run_check "Typos check" "typos --config .typos.toml"
else
    echo -e "${YELLOW}⚠ typos-cli not installed. Install with: brew install typos-cli${NC}"
    echo ""
fi

# Summary
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}✓ All checks passed! Ready to push.${NC}"
    exit 0
else
    echo -e "${RED}✗ Some checks failed. Fix them before pushing.${NC}"
    exit 1
fi
