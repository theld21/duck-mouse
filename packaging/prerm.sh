#!/bin/sh
# Trước khi gỡ hẳn: dừng dịch vụ (khi nâng cấp thì để nguyên)
case "$1" in upgrade|1) exit 0 ;; esac
for u in $(loginctl list-users --no-legend 2>/dev/null | awk '$1 >= 1000 {print $2}'); do
    systemctl --user -M "$u@" stop duckmouse.service 2>/dev/null || true
done
systemctl --global disable duckmouse.service 2>/dev/null || true
exit 0
