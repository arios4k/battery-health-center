#!/bin/sh
# Builds the .deb and publishes it to the apt repository
# (https://arios4k.github.io/apt, served by GitHub Pages from arios4k/apt).
#
# Before running it for a new version:
#   1. bump the version in Cargo.toml, and add an entry to data/deb/changelog
#   2. make sure your signing key is available (gpg --list-secret-keys)
#
# Usage: scripts/publish-apt.sh [debian-revision]   (default revision: 1)
#        DRY_RUN=1 scripts/publish-apt.sh           (print the steps, change nothing)
#
# The repository's private files (config and database) live in $BHC_APT_REPO
# (default ~/apt-repo). Only the public files are in $BHC_APT_SITE (default
# ~/apt-site, a clone of arios4k/apt), which this script commits and pushes.
set -eu

REPO_DIR=${BHC_APT_REPO:-$HOME/apt-repo}
SITE_DIR=${BHC_APT_SITE:-$HOME/apt-site}
SUITE=stable
REVISION=${1:-1}

run() {
    echo "+ $*"
    [ "${DRY_RUN:-0}" = 1 ] || "$@"
}

cd "$(dirname "$0")/.."

for dir in "$REPO_DIR/conf" "$SITE_DIR/.git"; do
    [ -e "$dir" ] || { echo "missing $dir (see the header of this script)" >&2; exit 1; }
done
command -v reprepro >/dev/null || { echo "reprepro is not installed" >&2; exit 1; }

version=$(cargo pkgid | sed 's/.*[#@]//')
deb="target/debian/battery-health-center_${version}-${REVISION}_amd64.deb"
echo "Publishing battery-health-center ${version}-${REVISION}"

if ! grep -q "^battery-health-center (${version}-${REVISION})" data/deb/changelog; then
    echo "data/deb/changelog has no entry for ${version}-${REVISION}; add one first" >&2
    exit 1
fi
if [ -n "$(git -C "$SITE_DIR" status --porcelain)" ]; then
    echo "$SITE_DIR has uncommitted changes; commit or discard them first" >&2
    exit 1
fi

run cargo deb --deb-revision "$REVISION"
# reprepro refuses to replace a version it already has; bump the revision to rebuild.
run reprepro -b "$REPO_DIR" includedeb "$SUITE" "$deb"
run git -C "$SITE_DIR" add -A
run git -C "$SITE_DIR" commit -q -m "battery-health-center ${version}-${REVISION}"
run git -C "$SITE_DIR" push -q origin main

echo "Done. It can take a minute or two for GitHub Pages to update."
echo "Check: apt-cache policy battery-health-center (after sudo apt update)"
