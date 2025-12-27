#!/bin/bash
# Fix plugin dependencies to use workspace = true for publishing

set -e

echo "Fixing plugin dependencies to use workspace = true..."

# Fix reovim-core in all plugins
find plugins -name "Cargo.toml" -type f -exec sed -i 's/reovim-core = { path = "[^"]*" }/reovim-core.workspace = true/g' {} \;

# Fix reovim-sys in treesitter
sed -i 's/reovim-sys = { path = "[^"]*" }/reovim-sys.workspace = true/g' plugins/features/treesitter/Cargo.toml

# Fix reovim-lsp in lsp plugin
sed -i 's/reovim-lsp = { path = "[^"]*" }/reovim-lsp.workspace = true/g' plugins/features/lsp/Cargo.toml

# Fix reovim-plugin-microscope in lsp and pickers
sed -i 's/reovim-plugin-microscope = { path = "[^"]*" }/reovim-plugin-microscope.workspace = true/g' plugins/features/lsp/Cargo.toml
sed -i 's/reovim-plugin-microscope = { path = "[^"]*" }/reovim-plugin-microscope.workspace = true/g' plugins/features/pickers/Cargo.toml

# Fix reovim-plugin-treesitter in all language plugins
find plugins/languages -name "Cargo.toml" -type f -exec sed -i 's/reovim-plugin-treesitter = { path = "[^"]*" }/reovim-plugin-treesitter.workspace = true/g' {} \;

echo "Done! Verifying changes..."
cargo check --workspace

echo "All dependencies fixed successfully!"
