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

There are two ways: push a tag and let GitHub Actions do it, or run the script on your laptop.

### Automated: push a tag

The [Release workflow](../.github/workflows/release.yml) builds the `.deb`, publishes a GitHub Release with
it, and adds the package to the signed apt repository (`arios4k/apt`). The [CI workflow](../.github/workflows/ci.yml)
checks every push and pull request (build, tests, clippy, the `.deb`, and that the translation template is
up to date).

1. Bump `version` in `Cargo.toml` and add a matching entry to `data/deb/changelog`, then merge it to `main`.
2. Tag and push: `git tag v0.4.2 && git push origin v0.4.2`.

The workflow stops with an error if the tag doesn't match `Cargo.toml` or the changelog has no entry for it.
Running the workflow by hand from the Actions tab only builds the `.deb` and attaches it to the run.

The apt step needs three repository secrets (*Settings → Secrets and variables → Actions*), and is skipped
with a notice until they exist. The GitHub Release is published either way. Run these once on your laptop; each
asks for what it needs:

```bash
gpg --armor --export-secret-keys F090D01D74F714C85A9523CA4773367EBD976CF5 | gh secret set APT_SIGNING_KEY
gh secret set APT_SIGNING_PASSPHRASE     # the key's passphrase; leave it out if the key has none
gh secret set APT_REPO_TOKEN             # a fine-grained token with "Contents: read and write" on arios4k/apt only
```

This stores the signing key in GitHub's encrypted secrets. If you'd rather keep it only on your laptop, skip
the secrets and use the script below; the Release workflow still publishes the GitHub Release.
`packaging/apt/publish.sh` is the part that adds the package and signs the index, and can be run by hand
against a clone of `arios4k/apt`.

### By hand: the script

Bump the version in `Cargo.toml`, add an entry to `data/deb/changelog`, then run
`scripts/publish-apt.sh`. It builds the package, adds it to the repository, signs the index and pushes
the site. `DRY_RUN=1 scripts/publish-apt.sh` shows the steps without doing anything. It needs
`reprepro`, the signing key in your GPG keyring, and the repository's private files in `~/apt-repo`
(configuration and database, never published) next to a clone of `arios4k/apt` in `~/apt-site`.
