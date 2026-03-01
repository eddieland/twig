#!/usr/bin/env bash
# Sets up a temporary git repo with twig state for VHS demo recordings.
# Called from tape files via: Hide / Type "source docs/tapes/demo-setup.sh" / Enter
#
# Requires: twig binary on PATH, git

set -euo pipefail

# Create an isolated demo workspace
export DEMO_DIR
DEMO_DIR="$(mktemp -d)"
export HOME="$DEMO_DIR/fakehome"
mkdir -p "$HOME"

export XDG_CONFIG_HOME="$HOME/.config"
export XDG_DATA_HOME="$HOME/.local/share"
export XDG_CACHE_HOME="$HOME/.cache"

# Seed the git repo
REPO_DIR="$DEMO_DIR/my-project"
mkdir -p "$REPO_DIR"
cd "$REPO_DIR"

git init -b main --quiet
git config user.email "demo@example.com"
git config user.name "Demo User"

# Initial commit on main
echo "# My Project" > README.md
git add README.md
git commit -m "Initial commit" --quiet

# Create a realistic branch stack: main -> PROJ-101/auth-middleware -> PROJ-102/auth-tests
git checkout -b PROJ-101/auth-middleware --quiet
echo 'pub fn auth() {}' > auth.rs
git add auth.rs
git commit -m "feat: add auth middleware" --quiet

git checkout -b PROJ-102/auth-tests --quiet
echo '#[test] fn test_auth() {}' > auth_test.rs
git add auth_test.rs
git commit -m "test: add auth middleware tests" --quiet

# Another branch off main: PROJ-200/api-endpoints
git checkout main --quiet
git checkout -b PROJ-200/api-endpoints --quiet
echo 'pub fn endpoints() {}' > api.rs
git add api.rs
git commit -m "feat: add API endpoints" --quiet

# Go back to main
git checkout main --quiet

# Initialize twig and set up state
twig init > /dev/null 2>&1 || true
twig git add "$REPO_DIR" > /dev/null 2>&1 || true

# Set up root branches and dependencies via twig commands
twig branch root add main > /dev/null 2>&1 || true
twig branch depend PROJ-101/auth-middleware main > /dev/null 2>&1 || true
twig branch depend PROJ-102/auth-tests PROJ-101/auth-middleware > /dev/null 2>&1 || true
twig branch depend PROJ-200/api-endpoints main > /dev/null 2>&1 || true

# Switch to a feature branch for the demo
git checkout PROJ-101/auth-middleware --quiet

# Clear the terminal for a fresh start
clear
