#!/bin/bash

# CrystalCanvas Local Environment Setup
# Source this file to use project-local toolchains:
#   source dev_env.sh

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

export RUSTUP_HOME="$PROJECT_ROOT/.rustup"
export CARGO_HOME="$PROJECT_ROOT/.cargo"

# Homebrew paths: Intel (/usr/local/bin) and Apple Silicon (/opt/homebrew/bin)
# Required for cmake, node, and other system tools used by build.rs
export PATH="$PROJECT_ROOT/.cargo/bin:$PROJECT_ROOT/node_modules/.bin:/usr/local/bin:/opt/homebrew/bin:$PATH"

echo "✅ CrystalCanvas local environment activated."
echo "   Rust:  $(rustc --version)"
echo "   Cargo: $(cargo --version)"
echo "   Node:  $(node --version 2>/dev/null || echo 'not found')"
echo "   CMake: $(cmake --version 2>/dev/null | head -1 || echo 'not found')"
