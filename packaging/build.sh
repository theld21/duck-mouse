#!/usr/bin/env bash
# Đóng gói DuckMouse → dist/
#   packaging/build.sh rpm   – Fedora (build trên máy này)
#   packaging/build.sh deb   – Ubuntu 24.04+ / Debian 13+ (build trong Docker ubuntu:24.04)
#   packaging/build.sh       – cả hai
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$PWD
VER=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
mkdir -p dist

# Bày file vào cây thư mục cài đặt: stage <thư mục đích> <thư mục chứa 2 file chạy>
stage() {
    local d=$1 b=$2
    rm -rf "$d"
    install -Dm755 "$b/duckmouse"                     "$d/usr/bin/duckmouse"
    install -Dm755 "$b/duckmouse-settings"            "$d/usr/bin/duckmouse-settings"
    install -Dm644 packaging/70-duckmouse.rules       "$d/usr/lib/udev/rules.d/70-duckmouse.rules"
    install -Dm644 /dev/null                          "$d/usr/lib/modules-load.d/duckmouse.conf"
    echo uinput >                                     "$d/usr/lib/modules-load.d/duckmouse.conf"
    install -Dm644 packaging/duckmouse.service        "$d/usr/lib/systemd/user/duckmouse.service"
    install -Dm644 packaging/vn.vduck.DuckMouse.desktop "$d/usr/share/applications/vn.vduck.DuckMouse.desktop"
    install -Dm644 packaging/vn.vduck.DuckMouse.svg   "$d/usr/share/icons/hicolor/scalable/apps/vn.vduck.DuckMouse.svg"
    install -Dm644 config.example.toml                "$d/usr/share/doc/duckmouse/config.example.toml"
    install -Dm644 README.md                          "$d/usr/share/doc/duckmouse/README.md"
}

build_rpm() {
    command -v rpmbuild >/dev/null || { echo "cần rpm-build: sudo dnf install rpm-build"; exit 1; }
    cargo build --release --workspace
    local top=$ROOT/target/rpmbuild
    stage "$top/stage" target/release
    mkdir -p "$top/SPECS"
    cat > "$top/SPECS/duckmouse.spec" <<EOF
Name:           duckmouse
Version:        $VER
Release:        1
Summary:        Cuộn mượt + giữ nút chuột kéo 4 hướng làm phím tắt
License:        CC-BY-NC-4.0
Requires:       gtk4 >= 4.10, libadwaita >= 1.4, systemd, systemd-udev
%description
DuckMouse cho Linux: cuộn chuột mượt kiểu Mos và cử chỉ giữ nút + kéo 4 hướng, có cửa sổ cài đặt GTK.
%install
cp -a $top/stage/. %{buildroot}/
%files
/usr/bin/duckmouse
/usr/bin/duckmouse-settings
/usr/lib/udev/rules.d/70-duckmouse.rules
/usr/lib/modules-load.d/duckmouse.conf
/usr/lib/systemd/user/duckmouse.service
/usr/share/applications/vn.vduck.DuckMouse.desktop
/usr/share/icons/hicolor/scalable/apps/vn.vduck.DuckMouse.svg
/usr/share/doc/duckmouse
%post
$(cat packaging/postinst.sh)
%preun
$(cat packaging/prerm.sh)
EOF
    rpmbuild --quiet -bb --define "_topdir $top" --define "debug_package %{nil}" "$top/SPECS/duckmouse.spec"
    cp "$top"/RPMS/*/duckmouse-"$VER"-1.*.rpm dist/
    ls -la dist/*.rpm
}

# Chạy bên trong container Ubuntu
deb_inner() {
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y -qq curl ca-certificates build-essential pkg-config libgtk-4-dev libadwaita-1-dev dpkg-dev >/dev/null
    if ! command -v cargo >/dev/null; then
        curl -sSf https://sh.rustup.rs | sh -s -- -y -q --profile minimal
    fi
    . "$HOME/.cargo/env"
    CARGO_TARGET_DIR=target/ubuntu cargo build --release --workspace
    local d=target/ubuntu/deb
    stage "$d" target/ubuntu/release
    mkdir -p "$d/DEBIAN"
    local arch; arch=$(dpkg --print-architecture)
    cat > "$d/DEBIAN/control" <<EOF
Package: duckmouse
Version: $VER
Architecture: $arch
Maintainer: duckmouse <noreply@localhost>
Depends: libgtk-4-1 (>= 4.10), libadwaita-1-0 (>= 1.4), systemd, udev
Section: utils
Priority: optional
Description: Cuộn mượt + giữ nút chuột kéo 4 hướng làm phím tắt
 DuckMouse cho Linux: cuộn chuột mượt kiểu Mos và cử chỉ giữ nút + kéo 4 hướng, có cửa sổ cài đặt GTK.
EOF
    install -m755 packaging/postinst.sh "$d/DEBIAN/postinst"
    install -m755 packaging/prerm.sh "$d/DEBIAN/prerm"
    dpkg-deb --root-owner-group --build "$d" "dist/duckmouse_${VER}_${arch}.deb"
}

build_deb() {
    mkdir -p target/ubuntu/cargo-home
    docker run --rm -v "$ROOT":/src -w /src -v "$ROOT/target/ubuntu/cargo-home":/root/.cargo ubuntu:24.04 \
        bash packaging/build.sh deb-inner
    docker run --rm -v "$ROOT":/src ubuntu:24.04 chown -R "$(id -u):$(id -g)" /src/target/ubuntu /src/dist
    ls -la dist/*.deb
}

case "${1:-all}" in
    rpm) build_rpm ;;
    deb) build_deb ;;
    deb-inner) deb_inner ;;
    all) build_rpm; build_deb ;;
    *) echo "dùng: $0 [rpm|deb|all]"; exit 1 ;;
esac
