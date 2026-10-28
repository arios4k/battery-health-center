#!/usr/bin/env bash
# Adds a .deb to the signed apt repository (https://arios4k.github.io/apt, served by
# GitHub Pages from the arios4k/apt repository) and signs the index. The release
# workflow uses it; you can also run it by hand against a clone of arios4k/apt.
#
#   packaging/apt/publish.sh <deb> <clone of arios4k/apt>
#
# It rebuilds dists/ and pool/ from scratch, so the repository keeps only this
# version (as it always has), and refreshes the "Latest version" lines of the
# site's README.md and index.html. It does not commit or push.
#
# Environment:
#   APT_SIGNING_KEY         the secret signing key, armored; imported if set
#   APT_SIGNING_PASSPHRASE  its passphrase, if it has one
#   APT_KEY_ID              the key's fingerprint (default: the project's key)
#
# Needs reprepro and gpg.
set -euo pipefail

deb=${1:?usage: publish.sh <deb> <apt site directory>}
site=${2:?usage: publish.sh <deb> <apt site directory>}
key=${APT_KEY_ID:-F090D01D74F714C85A9523CA4773367EBD976CF5}
here=$(cd "$(dirname "$0")" && pwd)

[ -f "$deb" ] || { echo "no such file: $deb" >&2; exit 1; }
[ -d "$site" ] || { echo "no such directory: $site" >&2; exit 1; }
command -v reprepro >/dev/null || { echo "reprepro is not installed" >&2; exit 1; }

if [ -n "${APT_SIGNING_KEY:-}" ]; then
    printf '%s\n' "$APT_SIGNING_KEY" | gpg --batch --import
fi
gpg --batch --list-secret-keys "$key" >/dev/null 2>&1 || { echo "the signing key $key is not available" >&2; exit 1; }

# A key with a passphrase can't be unlocked by a person here, so hand the passphrase
# to gpg-agent once for the key and its subkeys.
if [ -n "${APT_SIGNING_PASSPHRASE:-}" ]; then
    echo "allow-preset-passphrase" >> "$(gpgconf --list-dirs homedir)/gpg-agent.conf"
    gpgconf --kill gpg-agent
    preset=$(gpgconf --list-dirs libexecdir)/gpg-preset-passphrase
    [ -x "$preset" ] || preset=/usr/lib/gnupg/gpg-preset-passphrase
    gpg --batch --with-colons --with-keygrip --list-secret-keys "$key" | awk -F: '/^grp:/ {print $10}' | while read -r grip; do
        "$preset" --preset --passphrase "$APT_SIGNING_PASSPHRASE" "$grip"
    done
fi

# A fresh reprepro database each time: only the version being published exists.
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir "$work/conf"
sed "s/@KEY@/$key/" "$here/distributions" > "$work/conf/distributions"
rm -rf "$site/dists" "$site/pool"
reprepro -b "$work" --outdir "$site" includedeb stable "$deb"

version=$(dpkg-deb --field "$deb" Version)
version=${version%-*}
for page in README.md index.html; do
    [ -f "$site/$page" ] && sed -i -E "s/(Latest version:\*{0,2} )[0-9][0-9.]*/\1${version}/" "$site/$page"
done
echo "Published $(basename "$deb") (version $version) to $site"
