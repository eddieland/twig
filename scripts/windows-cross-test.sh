#!/usr/bin/env bash

# Script Name: scripts/windows-cross-test.sh
# Description: Cross-compiles the workspace for Windows and runs tests under wine using cargo-nextest.
# Usage: ./scripts/windows-cross-test.sh [nextest flags] [-- <test binary flags>]
# Flags:
#   --help: Show help text and exit.
# Arguments:
#   Arguments before the first -- are passed to cargo-nextest.
#   Arguments after -- are forwarded to each test binary.
# Environment Variables:
#   TARGET: Target triple to build for (default: x86_64-pc-windows-gnu).
#   PROFILE: Cargo profile to use (default: ci-windows).
#   CARGO_BIN: Cargo executable to invoke (default: cargo).
#   WINE_BIN: Wine runner used to execute Windows binaries (default: wine).
#   WINEDEBUG: Controls wine logging (default: -all).
# Dependencies:
#   rustup, cargo-nextest, wine (or compatible runner), and the requested cargo toolchain.

set -euo pipefail

TARGET="${TARGET:-x86_64-pc-windows-gnu}"
PROFILE="${PROFILE:-ci-windows}"
CARGO_BIN="${CARGO_BIN:-cargo}"
WINE_BIN="${WINE_BIN:-wine}"
export WINEDEBUG="${WINEDEBUG:--all}"

usage() {
  cat <<'EOF'
Cross-compile the workspace for Windows and run tests under wine using cargo-nextest.

Environment:
  TARGET     Target triple to build for (default: x86_64-pc-windows-gnu)
  PROFILE    Cargo profile to use (default: ci-windows)
  CARGO_BIN  Cargo executable to invoke (default: cargo)
  WINE_BIN   Wine runner to execute Windows binaries (default: wine)
  WINEDEBUG  Controls wine logging (default: -all)

Arguments before the first -- are passed to cargo nextest (e.g., -p twig-core).
Arguments after -- are passed to each test binary.
EOF
}

if [[ "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

cargo_args=()
test_args=()
forward_tests=false
for arg in "$@"; do
  if [[ "${arg}" == "--" ]]; then
    forward_tests=true
    continue
  fi

  if [[ "${forward_tests}" == false ]]; then
    cargo_args+=("${arg}")
  else
    test_args+=("${arg}")
  fi
done

echo "Running cargo-nextest for target=${TARGET}, profile=${PROFILE} via runner=${WINE_BIN}..."
if [[ "${#test_args[@]}" -gt 0 ]]; then
  "${CARGO_BIN}" nextest run \
    --workspace \
    --target "${TARGET}" \
    --cargo-profile "${PROFILE}" \
    "${cargo_args[@]}" \
    -- \
    "${test_args[@]}"
else
  "${CARGO_BIN}" nextest run \
    --workspace \
    --target "${TARGET}" \
    --cargo-profile "${PROFILE}" \
    "${cargo_args[@]}"
fi
