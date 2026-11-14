#!/usr/bin/env bash
set -euo pipefail

binary="${1:?usage: build-deb.sh <missal binary> <version> <output directory>}"
version="${2:?version}"
output="${3:?output directory}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT

install -Dm755 "$binary" "$stage/usr/bin/missal"
install -Dm644 "$root/packaging/linux/missal.desktop" "$stage/usr/share/applications/com.zohaibakber.missal.desktop"
install -Dm644 "$root/assets/icon.png" "$stage/usr/share/icons/hicolor/1024x1024/apps/com.zohaibakber.missal.png"
install -Dm644 "$root/assets/fonts/Inter-LICENSE.txt" "$stage/usr/share/doc/missal/Inter-LICENSE.txt"
install -Dm644 "$root/packaging/THIRD-PARTY-NOTICES.txt" "$stage/usr/share/doc/missal/THIRD-PARTY-NOTICES.txt"

size_kb="$(du -sk "$stage/usr" | cut -f1)"
mkdir -p "$stage/DEBIAN"
cat > "$stage/DEBIAN/control" <<CONTROL
Package: missal
Version: ${version}
Section: misc
Priority: optional
Architecture: amd64
Maintainer: Zohaib Akber
Installed-Size: ${size_kb}
Depends: libc6, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libvulkan1, libfontconfig1, libfreetype6, libx11-xcb1, libxcb1
Recommends: mesa-vulkan-drivers
Description: Missal, a desktop app for writing First Information Reports
CONTROL

mkdir -p "$output"
dpkg-deb --root-owner-group --build "$stage" "$output/missal_${version}_amd64.deb"
