// duckmouse – cuộn mượt + giữ nút kéo 4 hướng cho Linux (Wayland / X11), ý tưởng từ DuckMouse (macOS) của Vduck.
// Đọc chuột thật (evdev, chiếm riêng) → xử lý → phát lại qua chuột ảo + bàn phím ảo (uinput).
mod actions;
#[allow(dead_code)]
mod config;
mod scroll;

use config::Config;
use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AttributeSet, BusType, Device, EventType, InputEvent, InputId, Key, RelativeAxisType,
};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime};

const BTN_MIDDLE: u16 = 0x112;
const REL_X: u16 = 0x00;
const REL_Y: u16 = 0x01;
const REL_HWHEEL: u16 = 0x06;
const REL_WHEEL: u16 = 0x08;
const REL_WHEEL_HI_RES: u16 = 0x0b;
const REL_HWHEEL_HI_RES: u16 = 0x0c;
const VNAME: &str = "duckmouse virtual";

type Shared = Arc<RwLock<Config>>;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--print-config") { print!("{}", config::TEMPLATE); return; }
    if args.iter().any(|a| a == "--check") {
        match config::load() { Ok(_) => println!("cấu hình OK: {}", config::path().display()), Err(e) => { eprintln!("{e}"); std::process::exit(1) } }
        return;
    }

    let cfg = match config::load() {
        Ok(c) => c,
        Err(e) => { eprintln!("{e} → dùng cấu hình mặc định"); Config::default() }
    };
    eprintln!("duckmouse: cấu hình {}", config::path().display());
    let cfg: Shared = Arc::new(RwLock::new(cfg));
    watch_config(cfg.clone());

    let mouse = Arc::new(Mutex::new(make_mouse().expect("không tạo được chuột ảo (/dev/uinput) – kiểm tra quyền")));
    let kbd = Arc::new(Mutex::new(make_kbd().expect("không tạo được bàn phím ảo (/dev/uinput)")));

    let scroller = scroll::Scroller::new();
    { let (s, m) = (scroller.clone(), mouse.clone()); std::thread::spawn(move || s.run(m)); }

    let (tx, rx) = mpsc::channel::<String>();
    {
        let runner = actions::Runner::new(kbd);
        std::thread::spawn(move || for a in rx { runner.run(&a) });
    }

    let mut warned = false;
    loop {
        let path = { let c = cfg.read().unwrap(); find_mouse(&c.device) };
        match path.map(|p| (Device::open(&p), p)) {
            Some((Ok(mut dev), p)) => {
                if let Err(e) = dev.grab() { eprintln!("không chiếm được {}: {e}", p.display()); std::thread::sleep(Duration::from_secs(2)); continue; }
                eprintln!("đang dùng chuột: {} ({})", dev.name().unwrap_or("?"), p.display());
                warned = false;
                let err = run_device(&mut dev, &cfg, &mouse, &scroller, &tx);
                eprintln!("mất chuột ({err}) – chờ cắm lại…");
                scroller.reset();
            }
            Some((Err(e), p)) => if !warned { eprintln!("không mở được {}: {e}", p.display()); warned = true },
            None => if !warned { eprintln!("chưa thấy chuột nào – chờ…"); warned = true },
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// Chuột: theo cấu hình, hoặc chuột đầu tiên trong /dev/input/by-id (chuột ảo không có ở đây → không tự bắt chính mình)
fn find_mouse(dev: &str) -> Option<PathBuf> {
    if !dev.trim().is_empty() { return Some(PathBuf::from(dev.trim())); }
    let mut v: Vec<PathBuf> = std::fs::read_dir("/dev/input/by-id").ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().ends_with("-event-mouse"))
        .collect();
    v.sort();
    v.into_iter().next()
}

fn make_mouse() -> std::io::Result<VirtualDevice> {
    let mut keys = AttributeSet::<Key>::new();
    for c in 0x110u16..=0x117 { keys.insert(Key::new(c)); }
    let mut rel = AttributeSet::<RelativeAxisType>::new();
    for c in [REL_X, REL_Y, REL_HWHEEL, REL_WHEEL, REL_WHEEL_HI_RES, REL_HWHEEL_HI_RES] { rel.insert(RelativeAxisType(c)); }
    VirtualDeviceBuilder::new()?
        .name(&format!("{VNAME} mouse"))
        .input_id(InputId::new(BusType::BUS_USB, 0x1d6b, 0x4d4f, 1))
        .with_keys(&keys)?
        .with_relative_axes(&rel)?
        .build()
}

fn make_kbd() -> std::io::Result<VirtualDevice> {
    let mut keys = AttributeSet::<Key>::new();
    for k in actions::all_keys() { keys.insert(k); }
    VirtualDeviceBuilder::new()?
        .name(&format!("{VNAME} keyboard"))
        .input_id(InputId::new(BusType::BUS_USB, 0x1d6b, 0x4b42, 1))
        .with_keys(&keys)?
        .build()
}

/// Lưu file cấu hình là tự áp dụng
fn watch_config(cfg: Shared) {
    std::thread::spawn(move || {
        let p = config::path();
        let mtime = || std::fs::metadata(&p).and_then(|m| m.modified()).ok();
        let mut last: Option<SystemTime> = mtime();
        loop {
            std::thread::sleep(Duration::from_millis(300));
            let now = mtime();
            if now.is_some() && now != last {
                last = now;
                match config::load() {
                    Ok(c) => { *cfg.write().unwrap() = c; eprintln!("đã nạp lại cấu hình"); }
                    Err(e) => eprintln!("{e} (giữ cấu hình cũ)"),
                }
            }
        }
    });
}

/// Nút đang giữ: chờ xem click hay giữ rồi kéo
struct Hold {
    code: u16,
    start: Instant,
    dx: f64,
    dy: f64,
    fired: bool,
    wheel: f64, // cộng dồn lăn lúc giữ nút (hi-res, 120 = 1 nấc)
}

/// Cài đặt của nút (None = nút không gán hoặc đã tắt → để nguyên như chuột thường)
fn button_cfg<'a>(c: &'a Config, code: u16) -> Option<&'a config::Button> {
    c.buttons.iter().find(|(n, b)| b.enabled && config::button_code(n) == Some(code)).map(|(_, b)| b)
}

/// Quãng kéo (dx, dy) đủ xa theo 1 hướng đã gán → chức năng của hướng đó
fn gesture_action(b: &config::Button, distance: f64, dx: f64, dy: f64) -> Option<&String> {
    let d = distance.max(5.0);
    let a = if dx.abs() >= d && dx.abs() > dy.abs() {
        if dx < 0.0 { &b.left } else { &b.right }
    } else if dy.abs() >= d && dy.abs() > dx.abs() {
        if dy < 0.0 { &b.up } else { &b.down }
    } else {
        return None;
    };
    (!config::is_default(a)).then_some(a)
}

/// Rê quá quãng này rồi nhả (mà chưa thành cử chỉ) thì coi là kéo dở, không phải bấm
fn cancel_distance(distance: f64) -> f64 { (distance * 0.3).max(20.0) }

fn run_device(dev: &mut Device, cfg: &Shared, mouse: &Arc<Mutex<VirtualDevice>>, scroller: &Arc<scroll::Scroller>, tx: &mpsc::Sender<String>) -> String {
    let has_hi = dev.supported_relative_axes().is_some_and(|a| a.contains(RelativeAxisType::REL_WHEEL_HI_RES));
    let mut frame: Vec<InputEvent> = Vec::with_capacity(8);
    let mut hold: Option<Hold> = None;
    let mut swallow_up: Vec<u16> = Vec::new(); // nút đã chạy chức năng lúc ấn → nuốt cú nhả
    let emit = |evs: &[InputEvent]| { if let Ok(mut m) = mouse.lock() { let _ = m.emit(evs); } };

    loop {
        let events: Vec<InputEvent> = match dev.fetch_events() {
            Ok(it) => it.collect(),
            Err(e) => return e.to_string(),
        };
        let c = cfg.read().unwrap();
        for ev in events {
            match ev.event_type() {
                EventType::SYNCHRONIZATION => {
                    if ev.code() == 0 && !frame.is_empty() { emit(&frame); frame.clear(); }
                }
                EventType::MISC => {}
                EventType::RELATIVE => {
                    let code = ev.code();
                    let v = ev.value();
                    match code {
                        REL_X | REL_Y => {
                            if let Some(h) = hold.as_mut().filter(|h| !h.fired) {
                                // đếm quãng kéo ngay từ lúc ấn (kéo nhanh vẫn ăn); chờ đủ «wait» mới chạy
                                if code == REL_X { h.dx += v as f64 } else { h.dy += v as f64 }
                                if let Some(b) = button_cfg(&c, h.code) {
                                    if h.start.elapsed().as_secs_f64() >= b.wait {
                                        if let Some(a) = gesture_action(b, c.gesture.distance, h.dx, h.dy) {
                                            h.fired = true;
                                            let _ = tx.send(a.clone());
                                        }
                                    }
                                }
                                if c.gesture.freeze_cursor { continue; }
                            }
                            frame.push(ev);
                        }
                        REL_WHEEL | REL_HWHEEL | REL_WHEEL_HI_RES | REL_HWHEEL_HI_RES => {
                            // đang giữ nút có gán «lăn lên / lăn xuống» → mỗi nấc chạy 1 lần, không cuộn trang
                            if let Some(h) = hold.as_mut() {
                                if let Some(b) = button_cfg(&c, h.code).filter(|b| b.has_wheel()) {
                                    let step = match (code, has_hi) {
                                        (REL_WHEEL_HI_RES, true) => v as f64,
                                        (REL_WHEEL, false) => v as f64 * 120.0,
                                        _ => continue,
                                    };
                                    h.wheel += step;
                                    while h.wheel.abs() >= 120.0 {
                                        let up = h.wheel > 0.0;
                                        h.wheel -= if up { 120.0 } else { -120.0 };
                                        h.fired = true; // đã dùng nút → nhả ra không tính là bấm
                                        let a = if up { &b.wheel_up } else { &b.wheel_down };
                                        if !config::is_default(a) { let _ = tx.send(a.clone()); }
                                    }
                                    continue;
                                }
                            }
                            let s = &c.scroll;
                            let sign = if s.reverse { -1 } else { 1 };
                            if !s.enabled {
                                frame.push(InputEvent::new(EventType::RELATIVE, code, v * sign));
                                continue;
                            }
                            let (axis, amount) = match (code, has_hi) {
                                (REL_WHEEL_HI_RES, true) => (0, v as f64),
                                (REL_HWHEEL_HI_RES, true) => (1, v as f64),
                                (REL_WHEEL, false) => (0, v as f64 * 120.0),
                                (REL_HWHEEL, false) => (1, v as f64 * 120.0),
                                _ => continue, // chuột có cả 2 loại → chỉ lấy hi-res, khỏi tính đôi
                            };
                            scroller.push(axis, amount * sign as f64, s.speed, s.duration, s.hz);
                        }
                        _ => frame.push(ev),
                    }
                }
                EventType::KEY => {
                    let code = ev.code();
                    // đang giữ nút hông có gán «bấm nút giữa» → cú bấm nút giữa chạy chức năng, không lọt ra ngoài
                    if code == BTN_MIDDLE && ev.value() == 1 {
                        if let Some(h) = hold.as_mut().filter(|h| h.code != BTN_MIDDLE) {
                            if let Some(a) = button_cfg(&c, h.code).map(|b| &b.middle_click).filter(|a| !config::is_default(a)) {
                                h.fired = true;
                                let _ = tx.send(a.clone());
                                swallow_up.push(code);
                                continue;
                            }
                        }
                    }
                    if ev.value() == 0 {
                        if let Some(i) = swallow_up.iter().position(|&x| x == code) { swallow_up.remove(i); continue; }
                    }
                    let Some(b) = button_cfg(&c, code) else { frame.push(ev); continue };
                    match ev.value() {
                        1 => {
                            if hold.is_some() { frame.push(ev); continue; }
                            if b.has_gesture() {
                                hold = Some(Hold { code, start: Instant::now(), dx: 0.0, dy: 0.0, fired: false, wheel: 0.0 });
                            } else if !config::is_default(&b.click) {
                                let _ = tx.send(b.click.clone());
                                swallow_up.push(code);
                            } else {
                                frame.push(ev);
                            }
                        }
                        0 => {
                            if hold.as_ref().is_some_and(|h| h.code == code) {
                                let h = hold.take().unwrap();
                                if h.fired { continue; }
                                // kéo đủ xa nhưng nhả trước khi hết thời gian chờ → vẫn chạy cử chỉ
                                if let Some(a) = gesture_action(b, c.gesture.distance, h.dx, h.dy) { let _ = tx.send(a.clone()); continue; }
                                // đã kéo dở (chưa đủ xa / hướng chưa gán) → huỷ, KHÔNG trả lại cú bấm (khỏi dán nhầm)
                                if h.dx.hypot(h.dy) > cancel_distance(c.gesture.distance) { continue; }
                                if config::is_default(&b.click) {
                                    // ấn nhanh → trả lại nút gốc (dán chuột giữa, Quay lại / Đi tiếp trình duyệt…)
                                    if !frame.is_empty() { emit(&frame); frame.clear(); }
                                    emit(&[InputEvent::new(EventType::KEY, code, 1)]);
                                    emit(&[InputEvent::new(EventType::KEY, code, 0)]);
                                } else {
                                    let _ = tx.send(b.click.clone());
                                }
                            } else if let Some(i) = swallow_up.iter().position(|&x| x == code) {
                                swallow_up.remove(i);
                            } else {
                                frame.push(ev);
                            }
                        }
                        _ => {}
                    }
                }
                _ => frame.push(ev),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mid() -> config::Button {
        config::Button { left: "workspace-left".into(), right: "workspace-right".into(), ..Default::default() }
    }
    #[test]
    fn cu_chi() {
        let b = mid();
        assert_eq!(gesture_action(&b, 120.0, -130.0, 10.0).map(|s| s.as_str()), Some("workspace-left"));
        assert_eq!(gesture_action(&b, 120.0, 130.0, 0.0).map(|s| s.as_str()), Some("workspace-right"));
        assert!(gesture_action(&b, 120.0, 50.0, 0.0).is_none());      // chưa đủ xa
        assert!(gesture_action(&b, 120.0, 0.0, -200.0).is_none());    // hướng lên chưa gán
        assert!(cancel_distance(120.0) < 120.0 && cancel_distance(120.0) >= 20.0);
    }
    #[test]
    fn tat_nut() {
        let mut c = Config::default();
        c.buttons.insert("middle".into(), mid());
        assert!(button_cfg(&c, 0x112).is_some());
        c.buttons.get_mut("middle").unwrap().enabled = false;
        assert!(button_cfg(&c, 0x112).is_none());
    }
}
