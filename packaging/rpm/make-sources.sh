#!/usr/bin/env bash
# Makes the two source archives the RPM spec needs, in the given folder (default: ./rpm-sources):
#   battery-health-center-<version>.tar.gz           the project at the release tag (or HEAD)
#   battery-health-center-<version>-vendor.tar.xz    every Rust crate it depends on, so the
#                                                     package builds without network access
#
# Usage: [REF=<git ref>] packaging/rpm/make-sources.sh [output folder]
#   REF defaults to the release tag v<version> when it exists, otherwise HEAD.
# Needs git, cargo and xz, plus network access to download the crates.
set -euo pipefail

cd "$(dirname "$0")/../.."
out=$(realpath -m "${1:-rpm-sources}")
version=$(cargo pkgid | sed 's/.*[#@]//')
name=battery-health-center
ref=${REF:-HEAD}
if [ -z "${REF:-}" ] && git rev-parse -q --verify "refs/tags/v$version" >/dev/null; then
    ref="v$version"
fi
mkdir -p "$out"

echo "Source archive: $name-$version.tar.gz (from $ref)"
git archive --format=tar.gz --prefix="$name-$version/" -o "$out/$name-$version.tar.gz" "$ref"

echo "Vendoring the Rust crates (needs network)..."
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
tar -xzf "$out/$name-$version.tar.gz" -C "$work"
(cd "$work/$name-$version" && cargo vendor --locked vendor >/dev/null)
tar -C "$work/$name-$version" -cf - vendor | xz -T0 -9 > "$out/$name-$version-vendor.tar.xz"

ls -la "$out"
