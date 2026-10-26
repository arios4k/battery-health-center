# Battery Health Center

All your laptop battery information in one place, for GNOME. Written in Rust with GTK 4 and libadwaita.

- **Overview**: charge level, time remaining, power draw, charger type and wattage, battery use since
  the last charge, wireless device batteries (mouse, keyboard, headphones), a health summary and
  recommendations (for example, a weak charger, high drain during sleep, or an app draining the battery).
- **Health**: maximum capacity compared with design capacity, condition, cycle count, voltage,
  manufacturer, model, chemistry, capacity over time with a projected 80% date, and guided calibration.
  Laptops with two batteries are supported.
- **Charging**: set a maximum charge (and where charging resumes), keep it after a restart or sleep,
  charge to 100% once ("Charge to 100% Now"), pause charging or run on battery while plugged in, and
  switch power mode.
- **Usage**: battery level and power draw (24 hours / 7 days), battery use during sleep, and the apps
  using the most energy now, since opening, or over 24 hours / 7 days.
- **Notifications** (Preferences): charge limit reached, fully charged without a limit, low battery,
  apps draining the battery, and high drain during sleep.
- **Battery report**: export an HTML report from the menu, or run
  `battery-health-center --export-report report.html`.

## Install from the Snap Store

```bash
sudo snap install battery-health-center
sudo snap connect battery-health-center:hardware-observe
sudo snap connect battery-health-center:system-observe
sudo snap connect battery-health-center:power-control
```

The snap is strictly confined. A bundled root service (`bhc-helper serve`) applies charge limits
through the `power-control` interface, which only allows the `charge_start_threshold` /
`charge_stop_threshold` files. Charging Mode and Power Mode are hidden in the snap because no
interface allows them. Calibration and Charge to 100% Now work in the snap. To build it locally, run
`snapcraft pack`.

Store screenshots are in `screenshots/`. To retake them, run the app with
`BHC_SCREENSHOTS=<folder>` (and optionally `BHC_SCREENSHOTS_STYLE=light` or `dark`). It saves one
image per page and quits.

## Requirements

GTK ≥ 4.14, libadwaita ≥ 1.5, and Rust. On Ubuntu, Zorin OS or Debian:

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev pkg-config
```

## Run from the source tree

```bash
cargo run --release
```

Changing the charge limit uses `pkexec` to run `bhc-helper` from the build folder, so it asks for your
password every time until the app is installed.

## Install

```bash
./install.sh
```

This installs:

| File | Path |
| --- | --- |
| App | `/usr/local/bin/battery-health-center` |
| Privileged helper | `/usr/local/libexec/battery-health-center/bhc-helper` |
| Polkit policy | `/usr/share/polkit-1/actions/io.github.BatteryHealthCenter.policy` |
| Restore service | `/etc/systemd/system/battery-health-center-restore.service` |
| Launcher and icon | `/usr/local/share/applications`, `/usr/local/share/icons` |

To remove it, run `./install.sh --uninstall`.

## How it works

- **Battery data** is read directly from `/sys/class/power_supply/BAT*`. Both energy-based (µWh) and
  charge-based (µAh) drivers are supported.
- **Charge limits** are written to `charge_control_start_threshold` and `charge_control_end_threshold`
  by `bhc-helper`, the only part that runs as root. It uses only the Rust standard library, accepts only
  validated arguments, and writes only those sysfs files and `/etc/battery-health-center.conf`. If you
  turn on **Keep After Restart**, the systemd service re-applies the limit at boot and after resume.
  Supported by ThinkPads, many ASUS, Dell, Huawei, LG, Samsung and System76 laptops, and others with
  a kernel driver for the thresholds.
- **App energy usage** is an estimate, because Linux doesn't measure power per app. CPU time is sampled
  from `/proc`. Processes are grouped into apps using the systemd scope GNOME launches each app in,
  falling back to the parent process for apps that move their processes elsewhere (such as Chromium-based
  browsers). On battery, the measured power above the idle level is shared between apps by CPU use.
- **Power mode** uses the power-profiles-daemon D-Bus API, the same one GNOME's quick settings use.
- **History** is saved to `~/.local/share/battery-health-center/history.csv` once a minute while the app
  runs. Turn on **Record History in the Background** to start it hidden at login.
