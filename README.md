# Battery Health Center

All your laptop battery information in one place, for GNOME. Written in Rust with GTK 4 and libadwaita.

- **Overview**: charge level, time remaining, power draw, charger type and wattage, battery use since
  the last charge, wireless device batteries (mouse, keyboard, headphones), a health summary and
  recommendations (for example, a weak charger, high drain during sleep, or an app draining the battery).
- **Health**: maximum capacity compared with design capacity, condition, cycle count, voltage,
  manufacturer, model, chemistry, capacity over time with a projected 80% date, and guided calibration.
  Laptops with two batteries are supported.
- **Charging**: pick a charge profile (Maximum Lifespan 60%, Balanced 80% or Full Capacity 100%) or set a
  maximum charge (and where charging resumes), keep it after a restart or sleep,
  charge to 100% once ("Charge to 100% Now"), and pause charging or run on battery while plugged in.
  A **charging schedule** charges to 100% just before the time you pick (for example, weekdays at 07:30)
  and keeps your limit the rest of the time; **optimized charging** learns those times from when you
  usually unplug.
- **Power**: the power mode (Power Saver, Balanced, Performance); **automatic power mode**, which
  switches it when you plug in or unplug the charger or when the charge gets low; GNOME's power-saving
  settings (dim screen, screen blank, automatic suspend and brightness, power saver on low battery);
  the power button and lid actions; and the battery percentage in the top bar. Only what your
  hardware supports is shown. ([Screenshot of the regular version](screenshots/regular/power-dark.png))
- **Usage**: battery level and power draw (24 hours / 7 days), battery use during sleep, and the apps
  using the most energy now, since opening, or over 24 hours / 7 days. Click an app to see its energy
  impact and estimated energy hour by hour over the last day or week.
- **Languages**: English, Spanish and German (see [Translations](#translations)).
- **Saving tips**: while on battery, the Overview suggests turning down a bright screen, switching off the
  keyboard backlight, or turning off Bluetooth when nothing is connected, with a button for each.
- **Notifications** (Preferences): charge limit reached, fully charged without a limit, low battery,
  apps draining the battery, and high drain during sleep.
- **Battery report**: export an HTML report from the menu, or run
  `battery-health-center --export-report report.html`.
- **CSV export**: **Export Data as CSV** in the menu (or `battery-health-center --export-csv <folder>`)
  writes the battery level and power history, capacity over time, sleep sessions and each app's energy
  use per hour as four CSV files for a spreadsheet.

> **Recommended: install with apt.** It gives you the full app (real app icons, the whole Power page, nothing to connect by hand) and updates with your system. Jump to [Install with apt](#install-with-apt-recommended). The snap is a limited, sandboxed edition that needs manual permissions.

## Install with apt (recommended)

**We recommend this way.** It is the best way to install Battery Health Center on Ubuntu, Debian, Zorin OS and Linux Mint (tested on Ubuntu 24.04, Zorin OS 18 and Linux Mint 22): four short commands, once, and then `apt` keeps it up to date. The snap is sandboxed, so it needs manual permissions and can't offer some features (see the [comparison](#apt-or-snap) below).

The `.deb` is the full, unsandboxed version: real app icons, per-app energy use, the complete Power
page, and charge limits through the normal PolicyKit password prompt. It needs libadwaita 1.5 or newer
(Ubuntu 24.04+, Zorin OS 18, Linux Mint 22, Debian 13).

### From the apt repository (easiest)

The package is published in a signed apt repository (amd64), so `apt` installs it and keeps it up to date:

```bash
sudo install -d -m 0755 /etc/apt/keyrings
```

```bash
curl -fsSL https://arios4k.github.io/apt/key.asc | sudo tee /etc/apt/keyrings/battery-health-center.asc > /dev/null
```

```bash
echo "deb [signed-by=/etc/apt/keyrings/battery-health-center.asc] https://arios4k.github.io/apt stable main" | sudo tee /etc/apt/sources.list.d/battery-health-center.list
```

```bash
sudo apt update && sudo apt install battery-health-center
```

The signing key's fingerprint is `F090 D01D 74F7 14C8 5A95 23CA 4773 367E BD97 6CF5`. The repository is
served from [arios4k/apt](https://github.com/arios4k/apt) through GitHub Pages.

### Build it yourself

Build it from a checkout (needs [`cargo-deb`](https://crates.io/crates/cargo-deb), installed with
`cargo install cargo-deb`, and `gettext` to compile the translations):

```bash
scripts/build-locale.sh && cargo deb
```

Install it, which also pulls in the dependencies:

```bash
sudo apt install ./target/debian/battery-health-center_*_amd64.deb
```

The package installs the app, the root helper in `/usr/libexec/battery-health-center/`, the PolicyKit
rule and a systemd service that re-applies a saved charge limit at boot and after sleep. Don't install
it together with the snap, since both would manage the same charge limit. If you switch from the snap,
back up its history first, because removing the snap deletes it: copy
`~/snap/battery-health-center/current/.local/share/battery-health-center/*.csv` to
`~/.local/share/battery-health-center/`.

### apt or snap?

| | apt / `.deb` (recommended) | Snap Store |
| --- | --- | --- |
| Permissions | Nothing to set up | Needs three `snap connect` commands after installing |
| Power page (power modes, automatic mode, power-saving settings, lid and power button) | Everything | Only Show Battery Percentage |
| Real app icons and per-app energy use | Yes | No icons; the sandbox limits it |
| Charge limit | Normal password prompt | Works, through a bundled service |
| Updates | With the rest of your system | Automatic, but a running app can delay them |
| Sandboxed | No | Yes |

## Install from the Snap Store (limited)

**We recommend [apt](#install-with-apt-recommended) instead** if your system supports it. The snap is sandboxed, so you have to connect its permissions by hand and some features are unavailable. Choose it if you want the sandbox or your system can't use the apt package.

```bash
sudo snap install battery-health-center
sudo snap connect battery-health-center:hardware-observe
sudo snap connect battery-health-center:system-observe
sudo snap connect battery-health-center:power-control
```

The snap is strictly confined. A bundled root service (`bhc-helper serve`) applies charge limits
through the `power-control` interface, which only allows the `charge_start_threshold` /
`charge_stop_threshold` files. Charging Mode and Power Mode are hidden in the snap because no
interface allows them. Calibration and Charge to 100% Now work in the snap. On the Power page, only
**Show Battery Percentage** works in the snap: the sandbox blocks the power mode service and can't see
GNOME's power settings, so the page says so and the rest is available in the regular (non-snap) version. To build it locally, run
`snapcraft pack`.

Store screenshots are in `screenshots/`. To retake them, run the app with
`BHC_SCREENSHOTS=<folder>` (and optionally `BHC_SCREENSHOTS_STYLE=light` or `dark`). It saves one
image per page and quits.

## Translations

The app is available in English, Spanish (`es`) and German (`de`). It uses your system language and
falls back to English for anything that isn't translated. The exported HTML battery report is still
English only.

To add a language, for example French:

```bash
msginit --locale=fr --input=po/battery-health-center.pot --output=po/fr.po
```

Translate the strings in `po/fr.po` (a PO editor such as Poedit or GNOME's Gtranslator helps), then add
`fr` to `po/LINGUAS` and a line for it next to the others in `[package.metadata.deb]` in `Cargo.toml`
(the `.mo` assets), and check it with the command above. Pull requests with new languages are very
welcome. `Comment[fr]` and `Keywords[fr]` in `data/io.github.BatteryHealthCenter.desktop` and a
`<summary xml:lang="fr">` in the metainfo file translate how the app is listed.

When the text in the app changes, update the template and the translations (strings that haven't been
translated yet show up as empty and are shown in English):

```bash
python3 scripts/update-pot.py
msgmerge --update --backup=none po/es.po po/battery-health-center.pot   # and the other languages
```

Strings in the code go through `tr("…")`, `trf("… {name}", &[("name", value)])` for text with values
in it, and `trn`/`trnf` for plurals; see `src/i18n.rs`.

## Publishing new versions (maintainers)

### apt repository and `.deb`

Bump the version in `Cargo.toml`, add an entry to `data/deb/changelog`, then run
`scripts/publish-apt.sh`. It builds the package, adds it to the repository, signs the index and pushes
the site. `DRY_RUN=1 scripts/publish-apt.sh` shows the steps without doing anything. It needs
`reprepro`, the signing key in your GPG keyring, and the repository's private files in `~/apt-repo`
(configuration and database, never published) next to a clone of `arios4k/apt` in `~/apt-site`.

### Snap Store

Bump the version in `Cargo.toml` and `snap/snapcraft.yaml` (they must match), then run
`scripts/publish-snap.sh`. It checks the versions and the store's 4096-character description limit,
builds the snap with `snapcraft pack`, releases it to the stable channel, checks the listing kept its
banner, and compares the store's description with `snap/snapcraft.yaml`. A new version does not change
the listing's description or images, so when they differ the script writes `store-description.txt`
for you to paste into the Description box on the store's listing page. `DRY_RUN=1 scripts/publish-snap.sh` shows the steps without doing anything. Don't use
`snapcraft upload-metadata`: it rewrites the listing's images. If the build fails with "Timed out
waiting for networking", a firewall is blocking LXD's bridge; the script prints the `ufw` commands.

## For distribution maintainers and bundlers

If you build or maintain a Linux distribution, a derivative or a package repository, you are welcome to
include Battery Health Center in it. I would be honored. It is free software under the
GPL-3.0-or-later, so you don't need to ask for permission, and you can package it as it is.

- **What to package:** the app, the root helper `bhc-helper` (installed in a libexec folder), the
  PolicyKit policy and the restore service in [`data/`](data/), the desktop file and icons, and the
  translations (`scripts/build-locale.sh <dir>` compiles `po/*.po` into `<dir>/<lang>/LC_MESSAGES/`;
  the app finds them in `share/locale` next to its `bin` folder, or in `$BHC_LOCALEDIR`). The
  [`.deb` packaging](data/deb/) (built with `cargo deb`) and [`install.sh`](install.sh) show where each
  file goes and which maintainer scripts are needed.
- **Needs:** GTK 4.14 or newer, libadwaita 1.5 or newer, UPower and PolicyKit (`pkexec`). systemd
  re-applies a saved charge limit at boot. Power modes need `power-profiles-daemon` (recommended), and
  the Power page uses GNOME's settings schemas.
- **Hardware support:** the charge limit works on laptops whose kernel exposes
  `charge_control_*_threshold` (or `charge_start/stop_threshold`) in sysfs.

If you need a change to make it fit your distribution (paths, branding, dependencies), open an
[issue](https://github.com/arios4k/battery-health-center/issues) or a pull request and I will gladly
help. A short note when it is included is always welcome, and I'm happy to link to your package.

## Requirements

GTK ≥ 4.14, libadwaita ≥ 1.5, and Rust, plus `gettext` to build the translations. On Ubuntu, Zorin OS or Debian:

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev pkg-config gettext
```

## Run from the source tree

```bash
cargo run --release
```

To try a translation, compile it first and point the app at it:

```bash
scripts/build-locale.sh && LANGUAGE=es BHC_LOCALEDIR=$PWD/locale cargo run --release
```

To try the Charging Schedule screens without touching your real settings, point the app at a folder
that holds a stand-in `battery-health-center.conf` (a line like `BAT0 75 80`), and optionally a
`battery-health-center.schedule` and an `unplug-log`:

```bash
BHC_HELPER_FILES=/path/to/folder cargo run --release
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
| Schedule service | `/etc/systemd/system/battery-health-center-scheduler.service` and `.path` |
| Launcher and icon | `/usr/local/share/applications`, `/usr/local/share/icons` |

To remove it, run `./install.sh --uninstall`.

## How it works

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
