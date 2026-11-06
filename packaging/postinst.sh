#!/bin/sh
# Sau khi cài: nạp uinput, cấp quyền cho phiên đăng nhập, bật dịch vụ cho mọi user và chạy ngay
modprobe uinput 2>/dev/null || true
udevadm control --reload 2>/dev/null || true
udevadm trigger --subsystem-match=misc --subsystem-match=input 2>/dev/null || true
udevadm settle 2>/dev/null || true
systemctl --global enable duckmouse.service 2>/dev/null || true
for u in $(loginctl list-users --no-legend 2>/dev/null | awk '$1 >= 1000 {print $2}'); do
    systemctl --user -M "$u@" daemon-reload 2>/dev/null || true
    systemctl --user -M "$u@" restart duckmouse.service 2>/dev/null || true
done
exit 0
