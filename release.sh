#!/bin/bash
# Release script for Screen Time Manager
# Designed for MINGW on Windows

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Get the directory where the script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo -e "${GREEN}=== Screen Time Manager Release Script ===${NC}"

# Check for uncommitted changes (excluding Cargo.toml which we'll modify)
if [[ -n $(git status --porcelain | grep -v "Cargo.toml" | grep -v "Cargo.lock") ]]; then
    echo -e "${RED}Error: You have uncommitted changes. Please commit or stash them first.${NC}"
    git status --short
    exit 1
fi

# Get current version from Cargo.toml
CURRENT_VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
echo -e "Current version: ${YELLOW}${CURRENT_VERSION}${NC}"

# Get the last version tag
LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
if [[ -z "$LAST_TAG" ]]; then
    echo -e "${YELLOW}No previous tags found. Will include all commits in changelog.${NC}"
    LAST_TAG=$(git rev-list --max-parents=0 HEAD)
fi
echo -e "Last tag: ${YELLOW}${LAST_TAG}${NC}"

# Determine new version
if [[ -n "$1" ]]; then
    NEW_VERSION="$1"
else
    # Auto-increment patch version
    IFS='.' read -ra VERSION_PARTS <<< "$CURRENT_VERSION"
    MAJOR=${VERSION_PARTS[0]}
    MINOR=${VERSION_PARTS[1]}
    PATCH=${VERSION_PARTS[2]}
    NEW_PATCH=$((PATCH + 1))
    NEW_VERSION="${MAJOR}.${MINOR}.${NEW_PATCH}"
fi

echo -e "New version: ${GREEN}${NEW_VERSION}${NC}"

# Confirm with user
read -p "Proceed with release v${NEW_VERSION}? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 1
fi

# Generate changelog from commits since last tag
echo -e "\n${GREEN}Generating changelog...${NC}"
CHANGELOG=$(git log "${LAST_TAG}..HEAD" --pretty=format:"- %s" --no-merges 2>/dev/null || git log --pretty=format:"- %s" --no-merges)

if [[ -z "$CHANGELOG" ]]; then
    CHANGELOG="- Version bump"
fi

echo -e "\nChanges since ${LAST_TAG}:"
echo "$CHANGELOG"

# Create commit message
COMMIT_MSG="Release v${NEW_VERSION}

Changes:
${CHANGELOG}"

# Update version in Cargo.toml
echo -e "\n${GREEN}Updating Cargo.toml...${NC}"
sed -i "s/^version = \"${CURRENT_VERSION}\"/version = \"${NEW_VERSION}\"/" Cargo.toml

# Verify the change
NEW_VERSION_CHECK=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
if [[ "$NEW_VERSION_CHECK" != "$NEW_VERSION" ]]; then
    echo -e "${RED}Error: Failed to update version in Cargo.toml${NC}"
    git checkout Cargo.toml
    exit 1
fi

# Update Cargo.lock by running cargo check
echo -e "${GREEN}Updating Cargo.lock...${NC}"
cargo check --quiet 2>/dev/null || true

# Stage and commit
echo -e "\n${GREEN}Committing changes...${NC}"
git add Cargo.toml Cargo.lock
git commit -m "$COMMIT_MSG"

# Create tag
echo -e "${GREEN}Creating tag v${NEW_VERSION}...${NC}"
git tag -a "v${NEW_VERSION}" -m "Release v${NEW_VERSION}"

# Get current branch
CURRENT_BRANCH=$(git branch --show-current)
echo -e "Current branch: ${YELLOW}${CURRENT_BRANCH}${NC}"

# Ensure release branch exists and is updated
echo -e "\n${GREEN}Updating release branch...${NC}"

# Check if release branch exists
if git show-ref --verify --quiet refs/heads/release; then
    # Release branch exists locally
    git checkout release
    git merge "$CURRENT_BRANCH" -m "Merge ${CURRENT_BRANCH} into release for v${NEW_VERSION}"
else
    # Check if release exists on remote
    if git ls-remote --exit-code --heads origin release >/dev/null 2>&1; then
        git checkout -b release origin/release
        git merge "$CURRENT_BRANCH" -m "Merge ${CURRENT_BRANCH} into release for v${NEW_VERSION}"
    else
        # Create new release branch from current branch
        git checkout -b release
    fi
fi

# Push everything
echo -e "\n${GREEN}Pushing to remote...${NC}"
git push origin release
git push origin "$CURRENT_BRANCH"
git push origin "v${NEW_VERSION}"

# Switch back to original branch
git checkout "$CURRENT_BRANCH"

echo -e "\n${GREEN}=== Release v${NEW_VERSION} complete! ===${NC}"
echo -e "Summary:"
echo -e "  - Updated version: ${CURRENT_VERSION} -> ${NEW_VERSION}"
echo -e "  - Created tag: v${NEW_VERSION}"
echo -e "  - Updated and pushed release branch"
echo -e "  - Pushed to origin"
