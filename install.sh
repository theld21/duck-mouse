#!/usr/bin/env bash
# Cài DuckMouse 1 lệnh: tự nhận Fedora / Ubuntu, dùng gói có sẵn trong dist/ (chưa có thì build)
#   ./install.sh            cài
#   ./install.sh --uninstall gỡ
set -euo pipefail
cd "$(dirname "$0")"
if [[ "${1:-}" == "--uninstall" ]]; then
    if command -v dnf >/dev/null; then sudo dnf remove -y duckmouse; else sudo apt-get remove -y duckmouse; fi
    exit 0
fi
if command -v dnf >/dev/null; then
    ls dist/duckmouse-*.rpm >/dev/null 2>&1 || packaging/build.sh rpm
    sudo dnf install -y "$(ls -t dist/duckmouse-*.rpm | head -1)"
elif command -v apt-get >/dev/null; then
    ls dist/duckmouse_*.deb >/dev/null 2>&1 || packaging/build.sh deb
    sudo apt-get install -y "./$(ls -t dist/duckmouse_*.deb | head -1)"
else
    echo "Chưa hỗ trợ bản Linux này (cần dnf hoặc apt)"; exit 1
fi
echo "Xong – DuckMouse đã chạy. Mở «DuckMouse» trong danh sách ứng dụng để chỉnh."
