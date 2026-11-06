# DuckMouse cho Linux

Cuộn chuột mượt và giữ nút chuột kéo 4 hướng làm phím tắt, có cửa sổ cài đặt giao diện GNOME. Viết bằng Rust:
- phần chạy nền khoảng 700 KB, ăn khoảng 3 MB RAM, 0% CPU khi rảnh;
- gói cài khoảng 400 KB.

Chạy trên Wayland lẫn X11, với GNOME, KDE và các desktop khác.

## Cài (1 phát là chạy)
| Máy | Lệnh |
|---|---|
| Fedora 40+ | `sudo dnf install ./duckmouse-0.1.0-1.x86_64.rpm` |
| Ubuntu 24.04+ / Debian 13+ | `sudo apt install ./duckmouse_0.1.0_amd64.deb` |

Bấm đúp file gói để mở bằng trình cài phần mềm cũng được. Cài xong là chạy ngay, và tự chạy mỗi lần đăng nhập.

Muốn chỉnh thì mở **DuckMouse** trong danh sách ứng dụng.

Gỡ: `sudo dnf remove duckmouse` hoặc `sudo apt remove duckmouse`.

## Dùng
- **Cuộn**: bật/tắt, đảo chiều, tốc độ, độ trôi.
- **Nút & cử chỉ**: mỗi nút (giữa, hông sau, hông trước) có 5 ô:
  - bấm nhanh;
  - giữ rồi kéo ← → ↑ ↓.

  Mỗi ô chọn được:
  - khoảng 30 chức năng có sẵn (copy/dán, workspace, Overview, tab, âm lượng…);
  - phím tắt tự ghi (bấm «Ghi phím»);
  - một lệnh hoặc mở thư mục.

Chỉnh xong là có tác dụng ngay. Cấu hình lưu ở `~/.config/duckmouse/config.toml` và sửa tay được.

## Cách hoạt động
- Phần chạy nền `duckmouse` (dịch vụ systemd của user) đọc chuột thật qua evdev và chiếm riêng con chuột đó.
- Nó phát lại sự kiện qua chuột ảo và bàn phím ảo (uinput).
- Cuộn mượt: mỗi nấc bánh xe được chia thành nhiều cú `REL_WHEEL_HI_RES` nhỏ ở 144 Hz, trôi chậm dần.
- Quyền dùng chuột cấp qua luật udev `uaccess` cho phiên đăng nhập, không cần chạy bằng root.

## Build từ mã nguồn
```bash
packaging/build.sh rpm   # → dist/*.rpm (cần rpm-build)
packaging/build.sh deb   # → dist/*.deb (build trong Docker ubuntu:24.04)
./install.sh             # tự nhận Fedora / Ubuntu và cài gói
cargo test --release -p duckmouse
```
Thử không cần chuột thật: `cargo build --release --example fake` tạo chuột giả theo lệnh (`wheel:3`, `drag:274:300:0`, `click:275`…).
