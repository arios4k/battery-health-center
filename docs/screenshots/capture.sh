#!/bin/sh
# Retakes the README screenshots (light and dark) with the app's built-in
# screenshot mode. It runs the app once per page and style (using the ZorinBlue
# theme, so run it on a system that has it, or change the theme names below), so
# it takes a couple of minutes and opens windows briefly on your desktop.
#
# The Charging page shows the schedule only when a charge limit is saved, so the
# script points the app at a stand-in folder (BHC_HELPER_FILES) with a saved
# limit, a schedule and some unplug times instead of touching your settings.
#
# Usage: docs/screenshots/capture.sh [path/to/battery-health-center]
set -eu
cd "$(dirname "$0")/../.."

APP=${1:-target/release/battery-health-center}
[ -x "$APP" ] || { echo "build the app first: cargo build --release" >&2; exit 1; }
OUT=docs/screenshots
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

mkdir "$WORK/files"
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

# page number, file name, pixels to scroll down first
PAGES="1 overview 0
2 health 0
3 charging 0
3 schedule 9999
4 power 0
5 usage 0"

for style in light dark; do
    # The same theme in both styles, so the pair matches.
    case $style in light) theme=ZorinBlue-Light ;; *) theme=ZorinBlue-Dark ;; esac
    echo "$PAGES" | while read -r n name scroll; do
        # A page occasionally fails to render; try again.
        for try in 1 2 3; do
            rm -rf "$WORK/shots"
            mkdir "$WORK/shots"
            GTK_THEME=$theme BHC_HELPER_FILES="$WORK/files" BHC_SCREENSHOTS="$WORK/shots" \
                BHC_SCREENSHOTS_STYLE=$style BHC_SCREENSHOTS_SCROLL=$scroll timeout 90 "$APP" >/dev/null 2>&1 || true
            f=$(ls "$WORK/shots/$n-"*.png 2>/dev/null | grep -v -- '-full' | head -1 || true)
            [ -n "$f" ] && break
        done
        cp "$f" "$OUT/$name-$style.png"
        echo "$OUT/$name-$style.png"
    done
done
