#!/bin/sh
# Compiles the translations in po/*.po into <dir>/<lang>/LC_MESSAGES/battery-health-center.mo.
# Usage: scripts/build-locale.sh [dir]   (default: locale/ in the source tree)
# To run the app from the build tree in another language:
#   scripts/build-locale.sh && LANGUAGE=es BHC_LOCALEDIR=$PWD/locale cargo run
set -eu
cd "$(dirname "$0")/.."
dir=${1:-locale}
for po in po/*.po; do
    lang=$(basename "$po" .po)
    mkdir -p "$dir/$lang/LC_MESSAGES"
    msgfmt --check -o "$dir/$lang/LC_MESSAGES/battery-health-center.mo" "$po"
done
echo "translations compiled into $dir/"
