#!/bin/sh
# Builds the snap and releases it to the Snap Store.
#
# Before running it for a new version:
#   1. bump the version in Cargo.toml AND snap/snapcraft.yaml (they must match)
#   2. you must be logged in to the store (snapcraft whoami)
#
# Usage: scripts/publish-snap.sh [channel]   (default channel: stable)
#        DRY_RUN=1 scripts/publish-snap.sh   (print the steps, change nothing)
#
# Uploading a version does NOT change the listing's description or images. After
# the release the script compares the store's description with the one in
# snap/snapcraft.yaml; if they differ it writes store-description.txt for you to
# paste on the listing page (https://snapcraft.io/battery-health-center/listing).
# Do NOT use `snapcraft upload-metadata` for this: it rewrites the listing's
# images and once removed the banner.
set -eu

CHANNEL=${1:-stable}
STORE_NAME=battery-health-center
DESCRIPTION_LIMIT=4096

run() {
    echo "+ $*"
    [ "${DRY_RUN:-0}" = 1 ] || "$@"
}

fail() {
    echo "error: $*" >&2
    exit 1
}

cd "$(dirname "$0")/.."

command -v snapcraft >/dev/null || fail "snapcraft is not installed"
command -v python3 >/dev/null || fail "python3 is needed for the checks"

cargo_version=$(cargo pkgid | sed 's/.*[#@]//')
snap_version=$(sed -n "s/^version: *['\"]\{0,1\}\([^'\"]*\)['\"]\{0,1\} *$/\1/p" snap/snapcraft.yaml | head -n 1)
[ "$cargo_version" = "$snap_version" ] ||
    fail "version mismatch: Cargo.toml says $cargo_version, snap/snapcraft.yaml says $snap_version"
echo "Releasing $STORE_NAME $cargo_version to the $CHANNEL channel"

# The store refuses a description over its limit, but only after a long build.
python3 - "$DESCRIPTION_LIMIT" <<'EOF' || fail "the description is too long for the store"
import sys
try:
    import yaml
except ImportError:
    print("(PyYAML is not installed, skipping the description length check)")
    sys.exit(0)
limit = int(sys.argv[1])
n = len(yaml.safe_load(open("snap/snapcraft.yaml"))["description"])
print(f"description: {n} of {limit} characters")
sys.exit(0 if n <= limit else 1)
EOF

if [ -n "$(git status --porcelain)" ]; then
    echo "warning: the working tree has uncommitted changes; the snap is built from them"
fi

file="${STORE_NAME}_${cargo_version}_amd64.snap"

if ! run snapcraft pack; then
    cat >&2 <<'EOF'

The build failed. If the log says "Timed out waiting for networking to be ready",
a firewall is blocking LXD's network bridge. Allow it (one time) with:
  sudo ufw allow in on lxdbr0
  sudo ufw route allow in on lxdbr0
  sudo ufw route allow out on lxdbr0
EOF
    exit 1
fi

run snapcraft upload --release="$CHANNEL" "$file"

if [ "${DRY_RUN:-0}" != 1 ]; then
    echo "Checking that the listing kept its images..."
    sleep 20
    curl -fsS -H 'Snap-Device-Series: 16' \
        "https://api.snapcraft.io/v2/snaps/info/${STORE_NAME}?fields=media,version" |
        python3 -c '
import json, sys
snap = json.load(sys.stdin)["snap"]
kinds = [m["type"] for m in snap.get("media", [])]
print("store media:", ", ".join(kinds) or "none")
if "banner" not in kinds:
    print("warning: the banner is missing; upload it again on the listing page", file=sys.stderr)
'
    curl -fsS -H 'Snap-Device-Series: 16' \
        "https://api.snapcraft.io/v2/snaps/info/${STORE_NAME}?fields=description" |
        python3 -c '
import json, sys
import yaml
store = json.load(sys.stdin)["snap"]["description"]
local = yaml.safe_load(open("snap/snapcraft.yaml"))["description"]
norm = lambda t: " ".join(t.split())
if norm(store) == norm(local):
    print("The store description matches snap/snapcraft.yaml.")
else:
    open("store-description.txt", "w").write(local)
    print("note: the store description differs from snap/snapcraft.yaml.")
    print("      Paste store-description.txt into the Description box at")
    print("      https://snapcraft.io/battery-health-center/listing and save.")
' || echo "(could not compare descriptions; PyYAML or the store API was unavailable)"
    echo "Done. Check: snap info $STORE_NAME"
fi
