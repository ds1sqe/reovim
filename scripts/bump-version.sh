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

# Update workspace.dependencies versions for all local crates
# Core libraries
sed -i "s/\(reovim-core = { path = \"\.\/lib\/core\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-sys = { path = \"\.\/lib\/sys\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-lsp = { path = \"\.\/lib\/lsp\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"

# Feature plugins
sed -i "s/\(reovim-plugin-range-finder = { path = \"\.\/plugins\/features\/range-finder\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-plugin-settings-menu = { path = \"\.\/plugins\/features\/settings-menu\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-plugin-completion = { path = \"\.\/plugins\/features\/completion\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-plugin-explorer = { path = \"\.\/plugins\/features\/explorer\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-plugin-microscope = { path = \"\.\/plugins\/features\/microscope\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-plugin-pickers = { path = \"\.\/plugins\/features\/pickers\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-plugin-treesitter = { path = \"\.\/plugins\/features\/treesitter\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-plugin-pair = { path = \"\.\/plugins\/features\/pair\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-plugin-lsp = { path = \"\.\/plugins\/features\/lsp\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-plugin-which-key = { path = \"\.\/plugins\/features\/which-key\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-plugin-statusline = { path = \"\.\/plugins\/features\/statusline\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-plugin-notification = { path = \"\.\/plugins\/features\/notification\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-plugin-profiles = { path = \"\.\/plugins\/features\/profiles\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-plugin-health-check = { path = \"\.\/plugins\/features\/health-check\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-plugin-cmdline-completion = { path = \"\.\/plugins\/features\/cmdline-completion\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"

# Language plugins
sed -i "s/\(reovim-lang-rust = { path = \"\.\/plugins\/languages\/rust\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-lang-c = { path = \"\.\/plugins\/languages\/c\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-lang-javascript = { path = \"\.\/plugins\/languages\/javascript\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-lang-python = { path = \"\.\/plugins\/languages\/python\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-lang-json = { path = \"\.\/plugins\/languages\/json\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-lang-toml = { path = \"\.\/plugins\/languages\/toml\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-lang-markdown = { path = \"\.\/plugins\/languages\/markdown\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"
sed -i "s/\(reovim-lang-bash = { path = \"\.\/plugins\/languages\/bash\", version = \"\)[^\"]*/\1$NEW_VERSION/" "$ROOT_CARGO"

echo "Updated $ROOT_CARGO"

# Verify the changes
echo ""
echo "Verification:"
grep -E "^version = " "$ROOT_CARGO" | head -1
echo ""
echo "Local crate versions:"
grep -E 'reovim-.* = \{ path = .*, version = "' "$ROOT_CARGO" | wc -l
echo "crates updated"

# Run cargo check to verify
echo ""
echo "Running cargo check..."
cargo check --quiet

echo ""
echo "Version bumped to $NEW_VERSION successfully!"
echo "Don't forget to update CHANGELOG.md"
