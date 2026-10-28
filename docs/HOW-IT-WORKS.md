# How Battery Health Center works

The details behind the app, for the curious and for contributors. Back to the [README](../README.md).

- **Battery data** is read directly from `/sys/class/power_supply/BAT*`. Both energy-based (µWh) and
  charge-based (µAh) drivers are supported.
- **Charge limits** are written to `charge_control_start_threshold` and `charge_control_end_threshold`
  by `bhc-helper`, the only part that runs as root. It uses only the Rust standard library (plus libc for
  the local time), accepts only validated arguments, and writes only those sysfs files, its own config
  files in `/etc` (the saved limit and the charging schedule) and, for optimized charging, a list of
  unplug times in `/var/lib/battery-health-center`. If you
  turn on **Keep After Restart**, the systemd service re-applies the limit at boot and after resume.
  Supported by ThinkPads, many ASUS, Dell, Huawei, LG, Samsung and System76 laptops, and others with
  a kernel driver for the thresholds.
- **The charging schedule** needs a saved charge limit, because it lifts that limit to 100% from the
  lead time (2 hours by default) before each time you set, until 30 minutes after it, and then puts it
  back. A root systemd service (`battery-health-center-scheduler`, started by a `.path` unit only while
  a schedule exists; in the snap, the helper service) does this without asking for a password each
  time. **Optimized charging** also records when you unplug after the laptop has been plugged in for
  at least 3 hours. Once it has seen at least 3 such unplugs in the last 28 days on weekdays (or on
  weekends) that fall within an hour and a half of each other, it uses the earlier end of that range as
  a ready-by time. Nothing leaves your computer.
- **App energy usage** is an estimate, because Linux doesn't measure power per app. CPU time is sampled
  from `/proc`. Processes are grouped into apps using the systemd scope GNOME launches each app in,
  falling back to the parent process for apps that move their processes elsewhere (such as Chromium-based
  browsers). On battery, the measured power above the idle level is shared between apps by CPU use.
- **Power mode** uses the power-profiles-daemon D-Bus API, the same one GNOME's quick settings use.
- **History** is saved to `~/.local/share/battery-health-center/history.csv` once a minute while the app
  runs. Turn on **Record History in the Background** to start it hidden at login.
