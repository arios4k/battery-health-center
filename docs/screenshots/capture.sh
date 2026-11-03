#!/bin/sh
# Capture a complete, verified README image set from the current build.
# Usage: docs/screenshots/capture.sh [path/to/battery-health-center]
# Outputs to docs/screenshots/v<Cargo version>/ (override with BHC_SCREENSHOTS_OUTPUT).
# Capture files and history are isolated; real charge settings are never changed.
set -eu
cd "$(dirname "$0")/../.."
APP=${1:-target/release/battery-health-center}
[ -x "$APP" ] || { echo "build the app first: cargo build --release" >&2; exit 1; }
VERSION=$(sed -n 's/^version = "\([^"]*\)"$/\1/p' Cargo.toml | head -1)
OUT=${BHC_SCREENSHOTS_OUTPUT:-docs/screenshots/v$VERSION}
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$OUT" "$WORK/files" "$WORK/data/battery-health-center" "$WORK/config"
SOURCE_DATA=${XDG_DATA_HOME:-$HOME/.local/share}/battery-health-center
for file in history.csv health.csv app-history.csv; do
    if [ -f "$SOURCE_DATA/$file" ]; then
        cp "$SOURCE_DATA/$file" "$WORK/data/battery-health-center/$file"
    fi
done
export XDG_CONFIG_HOME="$WORK/config" XDG_DATA_HOME="$WORK/data"
unset BHC_SCREENSHOTS_PAGE BHC_SCREENSHOTS_TREND BHC_SCREENSHOTS_ABOUT
printf 'BAT0 75 80\n' > "$WORK/files/battery-health-center.conf"
printf 'optimized 1\nlead 120\nrule 31 07:30\nrule 96 10:00\n' > "$WORK/files/battery-health-center.schedule"
python3 - "$WORK/files/unplug-log" <<'PY'
import datetime, sys
now = datetime.datetime.now()
times = []
for d in range(1, 20):
    day = now - datetime.timedelta(days=d)
    if day.weekday() < 5:
        times.append(day.replace(hour=8, minute=10 + (d * 7) % 25, second=0))
    elif d % 2:
        times.append(day.replace(hour=11, minute=5, second=0))
open(sys.argv[1], "w").write("\n".join(str(int(t.timestamp())) for t in sorted(times)) + "\n")
PY

# Reject missing, undersized or flat-color captures. Uses only Python's standard library.
valid() {
    python3 - "$1" <<'PY'
import pathlib, struct, sys, zlib
try:
    data = pathlib.Path(sys.argv[1]).read_bytes()
    assert data[:8] == b'\x89PNG\r\n\x1a\n'
    width, height = struct.unpack('>II', data[16:24])
    assert width >= 300 and height >= 300
    at, compressed = 8, bytearray()
    while at < len(data):
        size = struct.unpack('>I', data[at:at + 4])[0]
        if data[at + 4:at + 8] == b'IDAT':
            compressed.extend(data[at + 8:at + 8 + size])
        at += size + 12
    assert len(set(zlib.decompress(compressed))) >= 32
except (OSError, AssertionError, ValueError, struct.error, zlib.error):
    sys.exit(1)
PY
}

run_capture() {
    style=$1 page=$2 scroll=$3 trend=$4
    SHOTS=$(mktemp -d "$WORK/shots.XXXXXX")
    set -- env BHC_SCREENSHOTS_SIZE=1240x840 BHC_HELPER_FILES="$WORK/files" \
        BHC_SCREENSHOTS="$SHOTS" BHC_SCREENSHOTS_STYLE="$style" BHC_SCREENSHOTS_SCROLL="$scroll"
    if [ "$page" != all ]; then set -- "$@" BHC_SCREENSHOTS_PAGE="$page"; fi
    if [ "$trend" = 1 ]; then set -- "$@" BHC_SCREENSHOTS_TREND=1; fi
    "$@" timeout 45 "$APP" >"$WORK/run.log" 2>&1 || true
}

for style in light dark; do
    run_capture "$style" all 0 0
    ALL_SHOTS=$SHOTS
    for item in '1 overview' '2 health' '3 charging' '4 power' '5 usage'; do
        set -- $item
        number=$1 name=$2
        image="$ALL_SHOTS/$number-$name.png"
        for attempt in 1 2 3; do
            if valid "$image"; then break; fi
            run_capture "$style" "$name" 0 0
            image="$SHOTS/$number-$name.png"
        done
        valid "$image" || { cat "$WORK/run.log" >&2; echo "invalid capture: $name-$style" >&2; exit 1; }
        cp "$image" "$OUT/$name-$style.png"
        echo "$OUT/$name-$style.png"
    done
    for attempt in 1 2 3; do
        run_capture "$style" charging 850 0
        image="$SHOTS/3-charging.png"
        if valid "$image"; then break; fi
    done
    valid "$image" || { echo "invalid schedule capture: $style" >&2; exit 1; }
    cp "$image" "$OUT/schedule-$style.png"
    echo "$OUT/schedule-$style.png"
done
for attempt in 1 2 3; do
    run_capture dark all 0 1
    image="$SHOTS/app-trend.png"
    if valid "$image"; then break; fi
done
valid "$image" || { echo 'invalid app-detail capture' >&2; exit 1; }
cp "$image" "$OUT/app-trend-dark.png"
echo "$OUT/app-trend-dark.png"
