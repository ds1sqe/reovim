#!/bin/bash
# Bump version across workspace
# Usage: ./scripts/bump-version.sh 0.5.0

set -e

if [ -z "$1" ]; then
    echo "Usage: $0 <new-version>"
    echo "Example: $0 0.5.0"
    exit 1
fi

NEW_VERSION="$1"
ROOT_CARGO="Cargo.toml"

# Validate version format (semver)
if ! [[ "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
    echo "Error: Invalid version format. Use semver (e.g., 0.5.0 or 0.5.0-beta.1)"
    exit 1
fi

echo "Bumping version to $NEW_VERSION"

# Get current version
CURRENT_VERSION=$(grep -E '^version = "' "$ROOT_CARGO" | head -1 | sed 's/version = "\(.*\)"/\1/')
echo "Current version: $CURRENT_VERSION"

# Update workspace.package version
sed -i "s/^\(version = \"\)[^\"]*\"/\1$NEW_VERSION\"/" "$ROOT_CARGO"

# Update workspace.dependencies versions for local crates
sed -i "s/\(reovim-core = { path = \"\.\/lib\/core\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-sys = { path = \"\.\/lib\/sys\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"

echo "Updated $ROOT_CARGO"

# Verify the changes
echo ""
echo "Verification:"
grep -E "^version = " "$ROOT_CARGO" | head -1
grep "reovim-core" "$ROOT_CARGO" | grep "path"
grep "reovim-sys" "$ROOT_CARGO" | grep "path"

# Run cargo check to verify
echo ""
echo "Running cargo check..."
cargo check --quiet

echo ""
echo "Version bumped to $NEW_VERSION successfully!"
echo "Don't forget to update CHANGELOG.md"
