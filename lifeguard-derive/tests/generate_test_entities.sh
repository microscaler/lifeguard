#!/bin/bash
# Generate entity code for all test files using lifeguard-codegen

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
GENERATED_DIR="$SCRIPT_DIR/generated"
INPUT_DIR="$PROJECT_ROOT/lifeguard-codegen/input"

# Create generated directory
mkdir -p "$GENERATED_DIR"

# Find all entity definitions in input directory
echo "🔧 Generating entity code for tests..."

# Generate user.rs (already exists)
if [ -f "$INPUT_DIR/user.rs" ]; then
    echo "  ✅ Generating user.rs..."
    "$PROJECT_ROOT/target/debug/lifeguard-codegen" generate \
        --input "$INPUT_DIR/user.rs" \
        --output "$GENERATED_DIR/" 2>&1 | grep -v "^$" || true
fi

echo "✨ Code generation complete!"
