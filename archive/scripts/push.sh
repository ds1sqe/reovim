#!/usr/bin/env bash
# Push script: Push commits and create/push version tag
# Usage: ./scripts/push.sh
#
# Reads version from Cargo.toml automatically

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

print_step() {
    echo -e "\n${YELLOW}==> $1${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

# Get version from Cargo.toml
VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/version = "\(.*\)"/\1/')

if [[ -z "$VERSION" ]]; then
    print_error "Failed to read version from Cargo.toml"
    exit 1
fi

TAG="v${VERSION}"

# Confirm with user
echo -e "${YELLOW}About to push:${NC}"
echo "  Version: ${VERSION}"
echo "  Tag: ${TAG}"
echo "  Branch: $(git branch --show-current)"
echo ""
read -p "Continue? [y/N] " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 1
fi

# Step 1: Push commits
print_step "Pushing commits to origin..."
if git push origin "$(git branch --show-current)"; then
    print_success "Commits pushed"
else
    print_error "Failed to push commits"
    exit 1
fi

# Step 2: Create tag
print_step "Creating tag ${TAG}..."
if git tag -a "${TAG}" -m "v${VERSION}"; then
    print_success "Tag ${TAG} created"
else
    print_error "Failed to create tag (may already exist)"
    exit 1
fi

# Step 3: Push tag
print_step "Pushing tag ${TAG} to origin..."
if git push origin "${TAG}"; then
    print_success "Tag ${TAG} pushed"
else
    print_error "Failed to push tag"
    exit 1
fi

echo -e "\n${GREEN}========================================${NC}"
echo -e "${GREEN}Push ${VERSION} complete!${NC}"
echo -e "${GREEN}========================================${NC}"
