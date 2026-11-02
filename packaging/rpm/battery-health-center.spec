# RPM package for Fedora (built on COPR). The Rust crates are vendored into a second
# source archive (see packaging/rpm/make-sources.sh), so the build needs no network.
# Debuginfo is skipped: the release profile strips the binaries.
%global debug_package %{nil}

Name:           battery-health-center
Version:        0.4.4
Release:        1%{?dist}
Summary:        Laptop battery health, charge limits and energy usage in one place

License:        GPL-3.0-or-later
URL:            https://github.com/arios4k/battery-health-center
Source0:        %{url}/archive/v%{version}/%{name}-%{version}.tar.gz
Source1:        %{name}-%{version}-vendor.tar.xz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gcc
BuildRequires:  gettext
BuildRequires:  pkgconfig(gtk4) >= 4.14
BuildRequires:  pkgconfig(libadwaita-1) >= 1.5
BuildRequires:  desktop-file-utils
BuildRequires:  libappstream-glib
BuildRequires:  systemd-rpm-macros

Requires:       polkit
Requires:       upower
Recommends:     power-profiles-daemon

%description
Battery Health Center shows everything about a laptop battery in one GNOME app:
health and cycle count, charge limits and profiles to reduce wear, a charging
schedule and optimized charging, battery use during sleep, power modes and
power-saving settings, battery-saving tips, and which apps use the most energy
(with an hour-by-hour view for each app). History can be exported as CSV. It is
available in English, Spanish and German.

Charge limits need a laptop whose firmware exposes charge thresholds to the
kernel (as on ThinkPads and many others). Changing them asks for a password
through PolicyKit; the charging schedule is then carried out by a small root
service without asking again.

%prep
%autosetup -n %{name}-%{version}
tar -xJf %{SOURCE1}
mkdir -p .cargo
cat > .cargo/config.toml <<'CARGO'
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
CARGO

%build
scripts/build-locale.sh
cargo build --release --offline --locked

%install
install -Dm755 target/release/battery-health-center %{buildroot}%{_bindir}/battery-health-center
install -Dm755 target/release/bhc-helper %{buildroot}%{_libexecdir}/battery-health-center/bhc-helper
install -Dm644 data/deb/io.github.BatteryHealthCenter.policy %{buildroot}%{_datadir}/polkit-1/actions/io.github.BatteryHealthCenter.policy
install -Dm644 data/io.github.BatteryHealthCenter.desktop %{buildroot}%{_datadir}/applications/io.github.BatteryHealthCenter.desktop
install -Dm644 data/io.github.BatteryHealthCenter.metainfo.xml %{buildroot}%{_metainfodir}/io.github.BatteryHealthCenter.metainfo.xml
install -Dm644 data/icons/hicolor/scalable/apps/io.github.BatteryHealthCenter.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/io.github.BatteryHealthCenter.svg
install -Dm644 data/deb/battery-health-center.1 %{buildroot}%{_mandir}/man1/battery-health-center.1
install -Dm644 data/deb/battery-health-center-restore.service %{buildroot}%{_unitdir}/battery-health-center-restore.service
install -Dm644 data/deb/battery-health-center-scheduler.service %{buildroot}%{_unitdir}/battery-health-center-scheduler.service
install -Dm644 data/deb/battery-health-center-scheduler.path %{buildroot}%{_unitdir}/battery-health-center-scheduler.path
for mo in locale/*/LC_MESSAGES/%{name}.mo; do
    lang=${mo#locale/}
    install -Dm644 "$mo" "%{buildroot}%{_datadir}/locale/$lang"
done
%find_lang %{name}

%check
desktop-file-validate %{buildroot}%{_datadir}/applications/io.github.BatteryHealthCenter.desktop
appstream-util validate-relax --nonet %{buildroot}%{_metainfodir}/io.github.BatteryHealthCenter.metainfo.xml
cargo test --release --offline --locked

%post
%systemd_post battery-health-center-restore.service battery-health-center-scheduler.path
# Fedora's presets leave new services off. The restore service re-applies a saved charge limit at boot
# and after sleep, and the path unit starts the schedule service only while a schedule exists. Turn both
# on for a new install, and start the path unit now, so nothing needs a reboot or a manual step.
if [ $1 -eq 1 ]; then
    systemctl enable battery-health-center-restore.service >/dev/null 2>&1 || :
    systemctl enable --now battery-health-center-scheduler.path >/dev/null 2>&1 || :
fi

%preun
%systemd_preun battery-health-center-restore.service battery-health-center-scheduler.path

%postun
%systemd_postun battery-health-center-restore.service battery-health-center-scheduler.path

%files -f %{name}.lang
%license LICENSE
%doc README.md
%{_bindir}/battery-health-center
%dir %{_libexecdir}/battery-health-center
%{_libexecdir}/battery-health-center/bhc-helper
%{_datadir}/polkit-1/actions/io.github.BatteryHealthCenter.policy
%{_datadir}/applications/io.github.BatteryHealthCenter.desktop
%{_metainfodir}/io.github.BatteryHealthCenter.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/io.github.BatteryHealthCenter.svg
%{_mandir}/man1/battery-health-center.1*
%{_unitdir}/battery-health-center-restore.service
%{_unitdir}/battery-health-center-scheduler.service
%{_unitdir}/battery-health-center-scheduler.path

%changelog
* Sun Nov 01 2026 Alexis Rios-Negron <182820807+arios4k@users.noreply.github.com> - 0.4.4-1
- Ask on the first launch whether to record history in the background

* Sun Nov 01 2026 Alexis Rios-Negron <182820807+arios4k@users.noreply.github.com> - 0.4.3-1
- Package for Fedora (COPR); see the project's changelog for the release notes
