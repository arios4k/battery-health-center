#!/bin/sh
# Capture the screenshots used by the user manual (docs/manual/img) from the current build.
# Usage: scripts/capture-manual.sh [path/to/battery-health-center]
# Settings and history are isolated, and real charge settings are never changed.
set -eu
cd "$(dirname "$0")/.."
APP=${1:-target/release/battery-health-center}
[ -x "$APP" ] || { echo "build the app first: cargo build --release" >&2; exit 1; }
OUT=${BHC_MANUAL_OUTPUT:-docs/manual/img}
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$OUT" "$WORK/files" "$WORK/data/battery-health-center" "$WORK/config"
SOURCE_DATA=${XDG_DATA_HOME:-$HOME/.local/share}/battery-health-center
for file in history.csv health.csv app-history.csv; do
    [ -f "$SOURCE_DATA/$file" ] && cp "$SOURCE_DATA/$file" "$WORK/data/battery-health-center/$file"
done
export XDG_CONFIG_HOME="$WORK/config" XDG_DATA_HOME="$WORK/data"
unset BHC_SCREENSHOTS_PAGE BHC_SCREENSHOTS_TREND BHC_SCREENSHOTS_ABOUT BHC_SCREENSHOTS_ACTION BHC_SCREENSHOTS_CLICK BHC_SCREENSHOTS_FULL
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

# shoot <style> <name> [VAR=value ...]: runs the app once and copies the capture to $OUT/<name>-<style>.png
shoot() {
    style=$1 name=$2; shift 2
    for attempt in 1 2 3; do
        SHOTS=$(mktemp -d "$WORK/shots.XXXXXX")
        env BHC_SCREENSHOTS_SIZE=1240x840 BHC_HELPER_FILES="$WORK/files" BHC_SCREENSHOTS="$SHOTS" \
            BHC_SCREENSHOTS_STYLE="$style" "$@" timeout 60 "$APP" >"$WORK/run.log" 2>&1 || true
        image=$(ls "$SHOTS"/*.png 2>/dev/null | grep -v -- -full.png | head -1 || true)
        if [ -n "$image" ] && [ "$(wc -c < "$image")" -gt 20000 ]; then
            cp "$image" "$OUT/$name-$style.png"; echo "$OUT/$name-$style.png"; return
        fi
    done
    cat "$WORK/run.log" >&2; echo "capture failed: $name-$style" >&2; exit 1
}

for style in light dark; do
    for page in overview health charging power usage; do
        shoot $style "$page" BHC_SCREENSHOTS_PAGE=$page BHC_SCREENSHOTS_FULL=1
        # One run writes the window (N-page.png) and the whole scrollable content (N-page-full.png).
        full=$(ls "$SHOTS"/*-full.png | head -1)
        cp "$full" "$OUT/$page-full-$style.png"; echo "$OUT/$page-full-$style.png"
    done
    shoot $style preferences BHC_SCREENSHOTS_PAGE=overview BHC_SCREENSHOTS_ACTION=win.preferences
    shoot $style add-time BHC_SCREENSHOTS_PAGE=charging BHC_SCREENSHOTS_SCROLL=850 BHC_SCREENSHOTS_CLICK="Add a Time…"
    shoot $style calibration BHC_SCREENSHOTS_PAGE=health BHC_SCREENSHOTS_CLICK="Start"
    shoot $style about BHC_SCREENSHOTS_ABOUT=1
    shoot $style app-detail BHC_SCREENSHOTS_TREND=1
done
