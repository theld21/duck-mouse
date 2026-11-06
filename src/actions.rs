// Chức năng gán cho nút / cử chỉ: tên có sẵn, "key:ctrl+c", "cmd:lệnh shell"
use evdev::{uinput::VirtualDevice, EventType, InputEvent, Key};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Chức năng có sẵn → phím tắt (theo mặc định GNOME / trình duyệt)
fn builtin(name: &str) -> Option<&'static str> {
    Some(match name {
        "copy" => "ctrl+c",
        "paste" => "ctrl+v",
        "cut" => "ctrl+x",
        "select-all" => "ctrl+a",
        "undo" => "ctrl+z",
        "redo" => "ctrl+shift+z",
        "overview" => "super",
        "show-apps" => "super+a",
        "workspace-left" => "super+pageup",
        "workspace-right" => "super+pagedown",
        "switch-app" => "alt+tab",
        "close-window" => "alt+f4",
        "minimize" => "super+h",
        "maximize" => "super+up",
        "fullscreen" => "f11",
        "back" => "alt+left",
        "forward" => "alt+right",
        "new-tab" => "ctrl+t",
        "close-tab" => "ctrl+w",
        "reopen-tab" => "ctrl+shift+t",
        "reload" => "ctrl+r",
        "prev-tab" => "ctrl+pageup",
        "next-tab" => "ctrl+pagedown",
        "screenshot" => "print",
        "lock" => "super+l",
        "play-pause" => "playpause",
        "next-track" => "nextsong",
        "prev-track" => "previoussong",
        "volume-up" => "volumeup",
        "volume-down" => "volumedown",
        "mute" => "mute",
        _ => return None,
    })
}

pub fn key_code(name: &str) -> Option<Key> {
    let n = name.trim().to_ascii_lowercase();
    let code: u16 = match n.as_str() {
        "ctrl" | "control" => 29, "shift" => 42, "alt" => 56, "super" | "meta" | "win" => 125,
        "rctrl" => 97, "rshift" => 54, "altgr" | "ralt" => 100,
        "esc" | "escape" => 1, "tab" => 15, "enter" | "return" => 28, "space" => 57, "backspace" => 14,
        "delete" | "del" => 111, "insert" => 110, "home" => 102, "end" => 107,
        "pageup" => 104, "pagedown" => 109, "up" => 103, "down" => 108, "left" => 105, "right" => 106,
        "print" => 99, "minus" => 12, "equal" => 13, "leftbrace" => 26, "rightbrace" => 27,
        "semicolon" => 39, "apostrophe" => 40, "grave" => 41, "backslash" => 43, "comma" => 51, "dot" => 52, "slash" => 53,
        "mute" => 113, "volumedown" => 114, "volumeup" => 115, "playpause" => 164, "nextsong" => 163, "previoussong" => 165,
        "stopcd" => 166, "calc" => 140, "www" => 150, "mail" => 155, "search" => 217, "brightnessdown" => 224, "brightnessup" => 225,
        _ => {
            let b = n.as_bytes();
            if b.len() == 1 {
                let row = |s: &str, base: u16| s.find(b[0] as char).map(|i| base + i as u16);
                row("1234567890", 2).or_else(|| row("qwertyuiop", 16)).or_else(|| row("asdfghjkl", 30))
                    .or_else(|| row("zxcvbnm", 44))?
            } else if let Some(f) = n.strip_prefix('f').and_then(|x| x.parse::<u16>().ok()) {
                match f { 1..=10 => 58 + f, 11 => 87, 12 => 88, 13..=24 => 183 + f - 13, _ => return None }
            } else if let Some(c) = n.strip_prefix("code").and_then(|x| x.parse::<u16>().ok()) {
                c
            } else {
                return None;
            }
        }
    };
    Some(Key::new(code))
}

/// Mọi phím bàn phím ảo cần khai báo
pub fn all_keys() -> Vec<Key> {
    let mut v: Vec<Key> = (1u16..=248).map(Key::new).collect();
    v.retain(|k| k.code() != 0);
    v
}

pub fn parse_chord(s: &str) -> Option<Vec<Key>> {
    s.split('+').map(key_code).collect()
}

pub struct Runner {
    kbd: Arc<Mutex<VirtualDevice>>,
}

impl Runner {
    pub fn new(kbd: Arc<Mutex<VirtualDevice>>) -> Self { Self { kbd } }

    pub fn run(&self, action: &str) {
        let a = action.trim();
        if a.is_empty() || a == "default" || a == "none" { return; }
        if let Some(cmd) = a.strip_prefix("cmd:") {
            match Command::new("sh").arg("-c").arg(cmd.trim()).stdin(Stdio::null()).spawn() {
                Ok(mut c) => { std::thread::spawn(move || { let _ = c.wait(); }); }
                Err(e) => eprintln!("không chạy được «{cmd}»: {e}"),
            }
            return;
        }
        let chord = a.strip_prefix("key:").or_else(|| builtin(a));
        match chord.and_then(parse_chord) {
            Some(keys) => self.tap(&keys),
            None => eprintln!("chức năng lạ: «{a}»"),
        }
    }

    /// Ấn lần lượt các phím rồi nhả ngược lại
    fn tap(&self, keys: &[Key]) {
        let ev = |k: &Key, v| InputEvent::new(EventType::KEY, k.code(), v);
        let Ok(mut kbd) = self.kbd.lock() else { return };
        for k in keys { let _ = kbd.emit(&[ev(k, 1)]); }
        drop(kbd);
        std::thread::sleep(Duration::from_millis(12));
        let Ok(mut kbd) = self.kbd.lock() else { return };
        for k in keys.iter().rev() { let _ = kbd.emit(&[ev(k, 0)]); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn moi_chuc_nang_trong_app_deu_chay_duoc() {
        for (id, _) in crate::config::ACTIONS.iter().filter(|a| a.0 != "default" && a.0 != "none") {
            let chord = builtin(id).unwrap_or_else(|| panic!("thiếu {id}"));
            assert!(parse_chord(chord).is_some(), "phím sai: {chord}");
        }
        // tên phím mà nút «Ghi phím» trong app tạo ra
        for k in ["ctrl+shift+t", "super+pageup", "alt+f4", "ctrl+dot", "leftbrace", "f12", "print", "home", "delete", "backspace", "space", "tab"] {
            assert!(parse_chord(k).is_some(), "không hiểu {k}");
        }
    }
}
