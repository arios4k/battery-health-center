#!/usr/bin/env bash
# Builds and tests the RPM inside a throwaway Fedora container (LXD), so it can be done from any
# distribution. It builds the source RPM and the binary RPM, runs rpmlint, installs the package
# the way a user would, and checks that the services come up by themselves.
#
# Usage: [FEDORA=43] packaging/rpm/build-in-lxd.sh <sources folder> [output folder]
#   <sources folder> holds what packaging/rpm/make-sources.sh made. The .src.rpm (the file to
#   upload to COPR) and the .rpm are written to the output folder (default: ./rpm-out).
set -euo pipefail
# lxc waits for input that never comes when stdin is left open (for example under a CI runner or a
# tool that spawns this script), so give every lxc command an empty stdin.
exec </dev/null

cd "$(dirname "$0")/../.."
src=$(realpath "${1:?usage: build-in-lxd.sh <sources folder> [output folder]}")
out=$(realpath -m "${2:-rpm-out}")
release=${FEDORA:-43}
box=bhc-rpm-build
spec=packaging/rpm/battery-health-center.spec

lxc delete -f "$box" >/dev/null 2>&1 || true
trap 'lxc delete -f "$box" >/dev/null 2>&1 || true' EXIT
echo "Starting a Fedora $release container..."
# Launching straight from the remote image server can stall, so keep a local copy of the image.
if ! lxc image info "local:fedora-$release" >/dev/null 2>&1; then
    timeout 600 lxc image copy "images:fedora/$release" local: --alias "fedora-$release" >/dev/null
fi
lxc launch "local:fedora-$release" "$box" >/dev/null
for _ in $(seq 1 60); do
    lxc exec "$box" -- systemctl is-system-running 2>/dev/null | grep -qE 'running|degraded' && break
    sleep 3
done

lxc exec "$box" -- mkdir -p /root/rpmbuild/SOURCES /root/rpmbuild/SPECS
lxc file push "$src"/*.tar.* "$box/root/rpmbuild/SOURCES/" >/dev/null
lxc file push "$spec" "$box/root/rpmbuild/SPECS/" >/dev/null

echo "Installing the build tools..."
lxc exec "$box" -- bash -c "dnf install -y -q rpm-build rpmlint rust cargo gcc gettext desktop-file-utils libappstream-glib \
    systemd-rpm-macros 'pkgconfig(gtk4)' 'pkgconfig(libadwaita-1)' > /root/dnf-build.log 2>&1" || \
    { lxc exec "$box" -- tail -20 /root/dnf-build.log; exit 1; }

echo "Building (this runs the test suite too)..."
lxc exec "$box" -- bash -c 'cd /root/rpmbuild && rpmbuild -ba SPECS/battery-health-center.spec > /root/build.log 2>&1' || \
    { lxc exec "$box" -- tail -40 /root/build.log; exit 1; }

echo "rpmlint:"
lxc exec "$box" -- bash -c 'rpmlint /root/rpmbuild/RPMS/x86_64/*.rpm /root/rpmbuild/SRPMS/*.rpm 2>&1 | tail -25' || true

echo "Installing the package like a user would..."
lxc exec "$box" -- bash -c 'dnf install -y -q /root/rpmbuild/RPMS/x86_64/battery-health-center-[0-9]*.rpm > /root/install.log 2>&1' || \
    { lxc exec "$box" -- tail -20 /root/install.log; exit 1; }
echo "Right after the install (no reboot, no login):"
lxc exec "$box" -- bash -c '
for u in battery-health-center-restore.service battery-health-center-scheduler.path battery-health-center-scheduler.service; do
    printf "  %-44s enabled=%-9s active=%s\n" $u "$(systemctl is-enabled $u 2>&1)" "$(systemctl is-active $u 2>&1)"
done
echo "Files:"
rpm -ql battery-health-center | grep -v /locale/ | sed "s/^/  /"
echo "Locale files: $(rpm -ql battery-health-center | grep -c /locale/)"
battery-health-center --export-csv /tmp/csv 2>&1 | head -2 | sed "s/^/  --export-csv: /"
'

mkdir -p "$out"
lxc file pull -r "$box/root/rpmbuild/RPMS/x86_64" "$out/" >/dev/null
lxc file pull -r "$box/root/rpmbuild/SRPMS" "$out/" >/dev/null
echo "Packages are in $out:"
find "$out" -name '*.rpm' -printf '  %P (%s bytes)\n'
