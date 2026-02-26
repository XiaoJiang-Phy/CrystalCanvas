#!/bin/bash

# CrystalCanvas Local Environment Setup
# Source this file to use project-local toolchains:
#   source dev_env.sh

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

export RUSTUP_HOME="$PROJECT_ROOT/.rustup"
export CARGO_HOME="$PROJECT_ROOT/.cargo"
export PATH="$PROJECT_ROOT/.cargo/bin:$PATH"

# Node.js paths (if needed, though npm uses node_modules/.bin automatically)
export PATH="$PROJECT_ROOT/node_modules/.bin:$PATH"

echo "✅ CrystalCanvas local environment activated."
echo "   Rust: $(rustc --version)"
echo "   Cargo: $(cargo --version)"
echo "   Node: $(node --version)"
