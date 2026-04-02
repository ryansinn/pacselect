#!/usr/bin/env bash
# release.sh — bump version, tag, push to GitHub, then update AUR
#
# Usage:
#   ./release.sh <version> [commit-suffix]
#
# Prerequisites:
#   - CHANGELOG.md already has an entry for <version>
#   - All code changes are in tracked files (untracked files are ignored)
#   - SSH key registered on both GitHub and AUR
#
# What it does:
#   1.  Bumps version in Cargo.toml and both PKGBUILDs
#   2.  Builds locally to verify compilation and update Cargo.lock
#   3.  Stages all tracked modifications, commits, tags, pushes to GitHub
#   4.  Waits for GitHub to generate the source tarball
#   5.  Computes sha256 of the tarball
#   6.  Updates sha256 in the project PKGBUILD, commits, pushes
#   7.  Updates AUR PKGBUILD + regenerates .SRCINFO, pushes to AUR

set -euo pipefail

NEW_VER="${1:?Usage: ./release.sh <version> [commit-suffix]  e.g. ./release.sh 0.6.1}"
SUFFIX="${2:-}"
REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
AUR_DIR="$HOME/Projects/aur/pacselect"
TARBALL_URL="https://github.com/ryansinn/pacselect/archive/refs/tags/v${NEW_VER}.tar.gz"

# ── Helpers ──────────────────────────────────────────────────────────────────
info()  { echo "  --> $*"; }
step()  { echo ""; echo "==> $*"; }
die()   { echo "ERROR: $*" >&2; exit 1; }

step "Releasing pacselect v${NEW_VER}"

# ── 1. Version bumps ──────────────────────────────────────────────────────────
step "Bumping versions"

info "Cargo.toml"
sed -i "s/^version = \"[^\"]*\"/version = \"${NEW_VER}\"/" "$REPO_DIR/Cargo.toml"

info "Project PKGBUILD (sha256 will be filled after tarball is available)"
sed -i "s/^pkgver=.*/pkgver=${NEW_VER}/"      "$REPO_DIR/PKGBUILD"
sed -i "s/^pkgrel=.*/pkgrel=1/"               "$REPO_DIR/PKGBUILD"
sed -i "s/^sha256sums=.*/sha256sums=('SKIP')/" "$REPO_DIR/PKGBUILD"

# ── 2. Build locally to verify and update Cargo.lock ─────────────────────────
step "Building (cargo build --release)"
cd "$REPO_DIR"
cargo build --release

# ── 3. Commit, tag, push ──────────────────────────────────────────────────────
step "Committing and tagging v${NEW_VER}"
cd "$REPO_DIR"

git add -u             # stage all tracked modifications
git add release.sh     # no-op after first commit; needed on initial run

# Show what's going in
git diff --cached --stat

if git diff --cached --quiet; then
    die "Nothing staged to commit. Did the version bump produce no changes?"
fi

COMMIT_MSG="release: ${NEW_VER}"
[[ -n "$SUFFIX" ]] && COMMIT_MSG="${COMMIT_MSG} — ${SUFFIX}"
git commit -m "$COMMIT_MSG"
git tag "v${NEW_VER}"

info "Pushing main branch and tag"
git push origin main
git push origin "v${NEW_VER}"

# ── 4. Wait for GitHub tarball ────────────────────────────────────────────────
step "Waiting for GitHub tarball"
info "$TARBALL_URL"

for i in $(seq 1 36); do
    HTTP_STATUS=$(curl -sLI -o /dev/null -w "%{http_code}" "$TARBALL_URL")
    if [[ "$HTTP_STATUS" == "200" ]]; then
        info "Tarball ready."
        break
    fi
    echo "    attempt ${i}/36 (HTTP ${HTTP_STATUS}), retrying in 10s..."
    sleep 10
done

[[ "$HTTP_STATUS" != "200" ]] && die "Tarball not available after 6 minutes. Check GitHub."

# ── 5. Compute sha256 ─────────────────────────────────────────────────────────
step "Computing sha256"
SHA256=$(curl -sL "$TARBALL_URL" | sha256sum | awk '{print $1}')
info "sha256: ${SHA256}"

[[ -z "$SHA256" ]] && die "Failed to compute sha256."

# ── 6. Update project PKGBUILD sha256 and push ───────────────────────────────
step "Updating project PKGBUILD sha256"
sed -i "s/^sha256sums=.*/sha256sums=('${SHA256}')/" "$REPO_DIR/PKGBUILD"

cd "$REPO_DIR"
git add PKGBUILD
git commit -m "PKGBUILD: set sha256sum for v${NEW_VER}"
git push origin main

# ── 7. Update AUR package ─────────────────────────────────────────────────────
step "Updating AUR package"

[[ -d "$AUR_DIR" ]] || die "AUR directory not found: $AUR_DIR"

info "Updating $AUR_DIR/PKGBUILD"
sed -i "s/^pkgver=.*/pkgver=${NEW_VER}/"       "$AUR_DIR/PKGBUILD"
sed -i "s/^pkgrel=.*/pkgrel=1/"                "$AUR_DIR/PKGBUILD"
sed -i "s/^sha256sums=.*/sha256sums=('${SHA256}')/" "$AUR_DIR/PKGBUILD"

info "Regenerating .SRCINFO"
cd "$AUR_DIR"
makepkg --printsrcinfo > .SRCINFO

info "Pushing to AUR"
git add PKGBUILD .SRCINFO
git commit -m "Update to v${NEW_VER}"
git push origin master

# ── Done ──────────────────────────────────────────────────────────────────────
echo ""
echo "==> Released pacselect v${NEW_VER}"
echo "    GitHub: https://github.com/ryansinn/pacselect/releases/tag/v${NEW_VER}"
echo "    AUR:    https://aur.archlinux.org/packages/pacselect"
