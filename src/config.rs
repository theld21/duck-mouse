// Cấu hình: ~/.config/duckmouse/config.toml – lưu file là tự áp dụng (không cần khởi động lại)
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const TEMPLATE: &str = include_str!("../config.example.toml");

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    /// Đường dẫn chuột (để trống = tự tìm /dev/input/by-id/*-event-mouse)
    pub device: String,
    pub scroll: Scroll,
    pub gesture: Gesture,
    pub buttons: std::collections::BTreeMap<String, Button>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(default)]
pub struct Scroll {
    pub enabled: bool,
    pub reverse: bool,
    /// Quãng cuộn mỗi nấc (1.0 = như mặc định Linux)
    pub speed: f64,
    /// Độ trôi kiểu Mos: 0.5 ngắn, 4.35 mặc định, 5 rất dài
    pub duration: f64,
    /// Số lần phát cuộn mỗi giây
    pub hz: f64,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(default)]
pub struct Gesture {
    /// Quãng kéo (đơn vị thô của chuột) để nhận 1 hướng
    pub distance: f64,
    /// Giữ con trỏ đứng yên khi đang giữ nút cử chỉ
    pub freeze_cursor: bool,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(default)]
pub struct Button {
    /// Bật / tắt toàn bộ chức năng gán cho nút này (tắt = nút như bình thường)
    pub enabled: bool,
    /// Giữ bao lâu (giây) rồi mới bắt đầu tính kéo
    pub wait: f64,
    pub click: String,
    pub left: String,
    pub right: String,
    pub up: String,
    pub down: String,
    /// Giữ nút rồi lăn bánh xe lên / xuống (mỗi nấc chạy 1 lần)
    pub wheel_up: String,
    pub wheel_down: String,
    /// Giữ nút (hông) rồi bấm nút giữa
    pub middle_click: String,
}

impl Default for Config {
    fn default() -> Self {
        Self { device: String::new(), scroll: Scroll::default(), gesture: Gesture::default(), buttons: Default::default() }
    }
}
impl Default for Scroll {
    fn default() -> Self { Self { enabled: true, reverse: false, speed: 1.0, duration: 4.35, hz: 144.0 } }
}
impl Default for Gesture {
    fn default() -> Self { Self { distance: 120.0, freeze_cursor: false } }
}
impl Default for Button {
    fn default() -> Self {
        let d = || "default".to_string();
        Self { enabled: true, wait: 0.12, click: d(), left: d(), right: d(), up: d(), down: d(), wheel_up: d(), wheel_down: d(), middle_click: d() }
    }
}

impl Button {
    pub fn has_wheel(&self) -> bool { !is_default(&self.wheel_up) || !is_default(&self.wheel_down) }
    pub fn has_gesture(&self) -> bool {
        [&self.left, &self.right, &self.up, &self.down, &self.wheel_up, &self.wheel_down, &self.middle_click].iter().any(|a| !is_default(a))
    }
}

pub fn is_default(a: &str) -> bool { a.trim().is_empty() || a.trim() == "default" }

/// Tên nút trong file cấu hình → mã nút evdev (không cho gán nút trái / phải để khỏi khoá chuột)
pub fn button_code(name: &str) -> Option<u16> {
    Some(match name {
        "middle" => 0x112,
        "side" | "back" => 0x113,
        "extra" | "forward" => 0x114,
        "btn_forward" => 0x115,
        "btn_back" => 0x116,
        "task" => 0x117,
        _ => return None,
    })
}

pub fn path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config"));
    base.join("duckmouse").join("config.toml")
}

/// Đọc cấu hình; chưa có file thì tạo từ mẫu
pub fn load() -> Result<Config, String> {
    let p = path();
    if !p.exists() {
        if let Some(dir) = p.parent() { let _ = std::fs::create_dir_all(dir); }
        std::fs::write(&p, TEMPLATE).map_err(|e| format!("không ghi được {}: {e}", p.display()))?;
    }
    let text = std::fs::read_to_string(&p).map_err(|e| format!("không đọc được {}: {e}", p.display()))?;
    let cfg: Config = toml::from_str(&text).map_err(|e| format!("lỗi cấu hình: {e}"))?;
    for name in cfg.buttons.keys() {
        if button_code(name).is_none() { eprintln!("bỏ qua nút lạ [buttons.{name}] (dùng: middle, back, forward, task)"); }
    }
    Ok(cfg)
}

/// Ghi cấu hình (dùng cho cửa sổ cài đặt) – ghi file tạm rồi đổi tên để daemon không đọc phải file dở
pub fn save(cfg: &Config) -> Result<(), String> {
    let p = path();
    if let Some(dir) = p.parent() { let _ = std::fs::create_dir_all(dir); }
    let body = toml::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    let text = format!("# duckmouse – sửa bằng app DuckMouse hoặc sửa tay (lưu là tự áp dụng)\n# Chức năng: xem `duckmouse --print-config`\n\n{body}");
    let tmp = p.with_extension("toml.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
}

/// Danh sách chức năng có sẵn: (mã, tên hiện trong app)
pub const ACTIONS: &[(&str, &str)] = &[
    ("default", "Giữ mặc định"),
    ("none", "Không làm gì"),
    ("copy", "Copy  (Ctrl+C)"),
    ("paste", "Dán  (Ctrl+V)"),
    ("cut", "Cắt  (Ctrl+X)"),
    ("select-all", "Chọn hết  (Ctrl+A)"),
    ("undo", "Hoàn tác  (Ctrl+Z)"),
    ("redo", "Làm lại  (Ctrl+Shift+Z)"),
    ("overview", "Overview – xem mọi cửa sổ"),
    ("show-apps", "Danh sách ứng dụng"),
    ("workspace-left", "Sang workspace trái"),
    ("workspace-right", "Sang workspace phải"),
    ("switch-app", "Chuyển app  (Alt+Tab)"),
    ("close-window", "Đóng cửa sổ  (Alt+F4)"),
    ("minimize", "Thu nhỏ cửa sổ"),
    ("maximize", "Phóng to cửa sổ"),
    ("fullscreen", "Toàn màn hình  (F11)"),
    ("back", "Quay lại trang  (Alt+←)"),
    ("forward", "Đi tiếp trang  (Alt+→)"),
    ("new-tab", "Tab mới  (Ctrl+T)"),
    ("close-tab", "Đóng tab  (Ctrl+W)"),
    ("reopen-tab", "Mở lại tab vừa đóng"),
    ("reload", "Tải lại trang  (Ctrl+R)"),
    ("prev-tab", "Tab trước"),
    ("next-tab", "Tab sau"),
    ("screenshot", "Chụp màn hình"),
    ("lock", "Khoá màn hình"),
    ("play-pause", "Phát / dừng nhạc"),
    ("next-track", "Bài tiếp"),
    ("prev-track", "Bài trước"),
    ("volume-up", "Tăng âm lượng"),
    ("volume-down", "Giảm âm lượng"),
    ("mute", "Tắt / bật tiếng"),
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mau_doc_duoc_va_ghi_lai_giu_nguyen() {
        let c: Config = toml::from_str(TEMPLATE).unwrap();
        assert_eq!(c.buttons["middle"].left, "workspace-left");
        assert!(c.buttons["middle"].has_gesture());
        let text = toml::to_string_pretty(&c).unwrap();
        let c2: Config = toml::from_str(&text).unwrap();
        assert_eq!(c2.buttons["back"].right, "paste");
        assert_eq!(c2.scroll.duration, c.scroll.duration);
        for (id, _) in ACTIONS { assert!(!id.is_empty()); }
    }
}
