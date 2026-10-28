# Maintaining and packaging

For the people who build, publish or package Battery Health Center. Back to the [README](../README.md).

## For distribution maintainers and bundlers

If you build or maintain a Linux distribution, a derivative or a package repository, you are welcome to
include Battery Health Center in it. I would be honored. It is free software under the
GPL-3.0-or-later, so you don't need to ask for permission, and you can package it as it is.

- **What to package:** the app, the root helper `bhc-helper` (installed in a libexec folder), the
  PolicyKit policy and the restore service in [`data/`](../data/), the desktop file and icons, and the
  translations (`scripts/build-locale.sh <dir>` compiles `po/*.po` into `<dir>/<lang>/LC_MESSAGES/`;
  the app finds them in `share/locale` next to its `bin` folder, or in `$BHC_LOCALEDIR`). The
  [`.deb` packaging](../data/deb/) (built with `cargo deb`) and [`install.sh`](../install.sh) show where each
  file goes and which maintainer scripts are needed.
- **Needs:** GTK 4.14 or newer, libadwaita 1.5 or newer, UPower and PolicyKit (`pkexec`). systemd
  re-applies a saved charge limit at boot. Power modes need `power-profiles-daemon` (recommended), and
  the Power page uses GNOME's settings schemas.
- **Hardware support:** the charge limit works on laptops whose kernel exposes
  `charge_control_*_threshold` (or `charge_start/stop_threshold`) in sysfs.

If you need a change to make it fit your distribution (paths, branding, dependencies), open an
[issue](https://github.com/arios4k/battery-health-center/issues) or a pull request and I will gladly
help. A short note when it is included is always welcome, and I'm happy to link to your package.

## Publishing new versions

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
