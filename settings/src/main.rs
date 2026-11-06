// DuckMouse – cửa sổ cài đặt (GTK4 + libadwaita). Ghi ~/.config/duckmouse/config.toml, daemon tự nạp lại.
#[path = "../../src/config.rs"]
#[allow(dead_code)]
mod config;

use adw::prelude::*;
use config::{Button, Config, ACTIONS};
use gtk::{gdk, gio, glib};
use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

const APP_ID: &str = "vn.vduck.DuckMouse";
const SERVICE: &str = "duckmouse.service";

struct State {
    cfg: RefCell<Config>,
    pending: RefCell<Option<glib::SourceId>>,
}
type St = Rc<State>;

/// Lưu sau 250 ms kể từ lần chỉnh cuối (kéo thanh trượt không ghi file liên tục)
fn save_soon(st: &St) {
    if let Some(id) = st.pending.borrow_mut().take() { id.remove(); }
    let s = st.clone();
    let id = glib::timeout_add_local_once(Duration::from_millis(250), move || {
        s.pending.borrow_mut().take();
        if let Err(e) = config::save(&s.cfg.borrow()) { eprintln!("không lưu được: {e}"); }
    });
    *st.pending.borrow_mut() = Some(id);
}

fn r2(v: f64) -> f64 { (v * 100.0).round() / 100.0 }

fn edit(st: &St, f: impl FnOnce(&mut Config)) {
    f(&mut st.cfg.borrow_mut());
    save_soon(st);
}

fn systemctl(args: &[&str]) -> bool {
    Command::new("systemctl").arg("--user").args(args).status().map(|s| s.success()).unwrap_or(false)
}
fn service_active() -> bool { systemctl(&["is-active", "--quiet", SERVICE]) }

fn main() -> glib::ExitCode {
    let mut flags = gio::ApplicationFlags::empty();
    if std::env::var_os("DUCKMOUSE_SNAP").is_some() { flags |= gio::ApplicationFlags::NON_UNIQUE; } // chụp thử không đụng cửa sổ đang mở
    let app = adw::Application::builder().application_id(APP_ID).flags(flags).build();
    app.connect_activate(build);
    app.run()
}

fn build(app: &adw::Application) {
    if let Some(w) = app.active_window() { w.present(); return; }
    // Mở app là chạy: chưa bật thì bật luôn (và tự chạy mỗi lần đăng nhập)
    if !service_active() { systemctl(&["enable", "--now", SERVICE]); }

    let cfg = config::load().unwrap_or_else(|e| { eprintln!("{e}"); Config::default() });
    let st: St = Rc::new(State { cfg: RefCell::new(cfg), pending: RefCell::new(None) });

    let win = adw::ApplicationWindow::builder().application(app).title("DuckMouse")
        .default_width(600).default_height(760).build();

    let stack = adw::ViewStack::new();
    stack.add_titled_with_icon(&scroll_page(&st), Some("scroll"), "Cuộn", "input-mouse-symbolic");
    stack.add_titled_with_icon(&buttons_page(&st, &win), Some("buttons"), "Nút & cử chỉ", "input-keyboard-symbolic");

    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::ViewSwitcher::builder().stack(&stack).policy(adw::ViewSwitcherPolicy::Wide).build()));

    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    if !perm_ok() {
        let banner = adw::Banner::new("Chưa có quyền đọc chuột – hãy đăng xuất rồi đăng nhập lại (hoặc cài lại gói)");
        banner.set_revealed(true);
        view.add_top_bar(&banner);
    }
    view.set_content(Some(&stack));
    win.set_content(Some(&view));
    win.present();
    gtk::prelude::GtkWindowExt::set_focus(&win, None::<&gtk::Widget>);
    // Tự chụp cửa sổ ra PNG để kiểm tra giao diện: DUCKMOUSE_SNAP=ảnh.png [DUCKMOUSE_PAGE=buttons]
    if let Ok(page) = std::env::var("DUCKMOUSE_PAGE") { stack.set_visible_child_name(&page); }
    if let Ok(path) = std::env::var("DUCKMOUSE_SNAP") {
        let w = win.clone();
        glib::timeout_add_local_once(Duration::from_millis(1200), move || {
            let p = gtk::WidgetPaintable::new(Some(&w));
            let snap = gtk::Snapshot::new();
            p.snapshot(&snap, w.width() as f64, w.height() as f64);
            if let (Some(node), Some(r)) = (snap.to_node(), w.native().and_then(|n| n.renderer())) {
                let _ = r.render_texture(node, None).save_to_png(&path);
            }
            w.close();
        });
    }
}

fn perm_ok() -> bool {
    std::fs::OpenOptions::new().write(true).open("/dev/uinput").is_ok()
}

// ───────────────────────── Trang Cuộn ─────────────────────────

fn scroll_page(st: &St) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    let c = st.cfg.borrow().clone();

    // Trạng thái
    let g = adw::PreferencesGroup::new();
    let on = adw::SwitchRow::builder().title("Bật DuckMouse")
        .subtitle("Tự chạy mỗi lần đăng nhập · tắt = chuột như bình thường").active(service_active()).build();
    on.connect_active_notify(|r| {
        let ok = if r.is_active() { systemctl(&["enable", "--now", SERVICE]) } else { systemctl(&["disable", "--now", SERVICE]) };
        if !ok { r.set_subtitle("Không bật / tắt được dịch vụ – xem: journalctl --user -u duckmouse"); }
    });
    g.add(&on);
    page.add(&g);

    let g = adw::PreferencesGroup::builder().title("Cuộn mượt").build();
    let smooth = adw::SwitchRow::builder().title("Cuộn mượt").subtitle("Mỗi nấc bánh xe trôi chậm dần như trackpad").active(c.scroll.enabled).build();
    let s = st.clone();
    smooth.connect_active_notify(move |r| { let v = r.is_active(); edit(&s, |c| c.scroll.enabled = v) });
    g.add(&smooth);
    let rev = adw::SwitchRow::builder().title("Đảo chiều cuộn").active(c.scroll.reverse).build();
    let s = st.clone();
    rev.connect_active_notify(move |r| { let v = r.is_active(); edit(&s, |c| c.scroll.reverse = v) });
    g.add(&rev);
    page.add(&g);

    let g = adw::PreferencesGroup::builder().title("Cảm giác cuộn").description("Chỉnh xong thử cuộn ngay, không cần lưu").build();
    let speed = spin("Tốc độ", "Quãng cuộn mỗi nấc · 1 = như Linux", 0.2, 6.0, 0.1, 1, c.scroll.speed);
    let s = st.clone();
    speed.connect_value_notify(move |r| { let v = r.value(); edit(&s, |c| c.scroll.speed = r2(v)) });
    g.add(&speed);
    let dur = spin("Độ trôi", "1 = dừng nhanh · 4.35 = như Mos · 5 = trôi rất dài", 0.5, 5.1, 0.05, 2, c.scroll.duration);
    let s = st.clone();
    dur.connect_value_notify(move |r| { let v = r.value(); edit(&s, |c| c.scroll.duration = r2(v)) });
    g.add(&dur);
    let reset = gtk::Button::builder().label("Mặc định").valign(gtk::Align::Center).build();
    let row = adw::ActionRow::builder().title("Khôi phục tốc độ và độ trôi").build();
    row.add_suffix(&reset);
    let (sp, du) = (speed.clone(), dur.clone());
    reset.connect_clicked(move |_| { let d = config::Scroll::default(); sp.set_value(d.speed); du.set_value(d.duration); });
    g.add(&row);
    page.add(&g);
    page
}

fn spin(title: &str, sub: &str, min: f64, max: f64, step: f64, digits: u32, v: f64) -> adw::SpinRow {
    let r = adw::SpinRow::with_range(min, max, step);
    r.set_title(title);
    r.set_subtitle(sub);
    r.set_digits(digits);
    r.set_value(v);
    r
}

// ───────────────────────── Trang Nút ─────────────────────────

const BUTTONS: &[(&str, &str)] = &[
    ("middle", "Nút giữa (bánh xe)"),
    ("back", "Nút hông sau"),
    ("forward", "Nút hông trước"),
];
const SLOTS: &[(&str, &str)] = &[
    ("click", "Bấm nhanh"),
    ("left", "← Giữ kéo trái"),
    ("right", "→ Giữ kéo phải"),
    ("up", "↑ Giữ kéo lên"),
    ("down", "↓ Giữ kéo xuống"),
    ("wheel_up", "⇡ Giữ nút + lăn lên"),
    ("wheel_down", "⇣ Giữ nút + lăn xuống"),
    ("middle_click", "◉ Giữ nút + bấm nút giữa"),
];

fn slot_mut<'a>(b: &'a mut Button, slot: &str) -> &'a mut String {
    match slot { "click" => &mut b.click, "left" => &mut b.left, "right" => &mut b.right, "up" => &mut b.up, "down" => &mut b.down, "wheel_up" => &mut b.wheel_up, "wheel_down" => &mut b.wheel_down, _ => &mut b.middle_click }
}

fn buttons_page(st: &St, win: &adw::ApplicationWindow) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    let c = st.cfg.borrow().clone();

    let g = adw::PreferencesGroup::builder().title("Cử chỉ")
        .description("Giữ nút rồi kéo chuột 1 hướng, hoặc giữ nút rồi lăn bánh xe (mỗi nấc chạy 1 lần). Bấm nhanh thì nút vẫn làm việc gốc nếu để «Giữ mặc định».").build();
    let dist = spin("Quãng kéo", "Kéo bao xa thì tính 1 hướng (nhỏ = nhạy)", 20.0, 600.0, 10.0, 0, c.gesture.distance);
    let s = st.clone();
    dist.connect_value_notify(move |r| { let v = r.value(); edit(&s, |c| c.gesture.distance = r2(v)) });
    g.add(&dist);
    let freeze = adw::SwitchRow::builder().title("Giữ con trỏ đứng yên khi kéo").active(c.gesture.freeze_cursor).build();
    let s = st.clone();
    freeze.connect_active_notify(move |r| { let v = r.is_active(); edit(&s, |c| c.gesture.freeze_cursor = v) });
    g.add(&freeze);
    if let Some(gs) = gio::SettingsSchemaSource::default()
        .and_then(|src| src.lookup("org.gnome.desktop.interface", true))
        .filter(|sc| sc.has_key("gtk-enable-primary-paste"))
        .map(|_| gio::Settings::new("org.gnome.desktop.interface"))
    {
        let paste = adw::SwitchRow::builder().title("Dán bằng chuột giữa")
            .subtitle("Của hệ thống: bấm chuột giữa dán chữ vừa bôi đen").build();
        gs.bind("gtk-enable-primary-paste", &paste, "active").build();
        g.add(&paste);
    }
    page.add(&g);

    for &(name, title) in BUTTONS {
        let b = c.buttons.get(name).cloned().unwrap_or_default();
        let g = adw::PreferencesGroup::builder().title(title).build();
        let on = adw::SwitchRow::builder().title("Dùng chức năng gán cho nút này")
            .subtitle("Tắt = nút hoạt động như bình thường").active(b.enabled).build();
        g.add(&on);
        let mut rows: Vec<gtk::Widget> = Vec::new();
        for &(slot, label) in SLOTS.iter().filter(|s| !(name == "middle" && s.0 == "middle_click")) {
            let mut bb = b.clone();
            let value = slot_mut(&mut bb, slot).clone();
            rows.extend(slot_rows(st, win, &g, name, slot, label, &value));
        }
        let wait = spin("Giữ bao lâu mới chạy", "Giây – kéo nhanh hơn thế vẫn ăn, chỉ chờ rồi mới chạy", 0.0, 1.0, 0.05, 2, b.wait);
        let s = st.clone();
        wait.connect_value_notify(move |r| { let v = r.value(); edit(&s, |c| c.buttons.entry(name.into()).or_default().wait = r2(v)) });
        g.add(&wait);
        rows.push(wait.upcast());
        for w in &rows { w.set_sensitive(b.enabled); }
        let s = st.clone();
        on.connect_active_notify(move |r| {
            let v = r.is_active();
            for w in &rows { w.set_sensitive(v); }
            edit(&s, |c| c.buttons.entry(name.into()).or_default().enabled = v);
        });
        page.add(&g);
    }
    page
}

/// 1 ô gán: danh sách chức năng + ô nhập (phím tắt / lệnh) hiện khi chọn kiểu tự chọn
fn slot_rows(st: &St, win: &adw::ApplicationWindow, g: &adw::PreferencesGroup, button: &'static str, slot: &'static str, label: &str, value: &str) -> [gtk::Widget; 2] {
    let n = ACTIONS.len() as u32;
    let mut labels: Vec<&str> = ACTIONS.iter().map(|a| a.1).collect();
    labels.push("Phím tắt tự chọn…");
    labels.push("Chạy lệnh / mở thư mục, app…");
    let combo = adw::ComboRow::builder().title(label).model(&gtk::StringList::new(&labels)).enable_search(true).build();

    let entry = adw::EntryRow::new();
    let rec = gtk::Button::builder().label("Ghi phím").valign(gtk::Align::Center).css_classes(["flat"]).build();
    let folder = gtk::Button::builder().icon_name("folder-open-symbolic").tooltip_text("Chọn thư mục để mở")
        .valign(gtk::Align::Center).css_classes(["flat"]).build();
    entry.add_suffix(&rec);
    entry.add_suffix(&folder);

    let (idx, text) = if let Some(k) = value.strip_prefix("key:") { (n, k.to_string()) }
        else if let Some(c) = value.strip_prefix("cmd:") { (n + 1, c.to_string()) }
        else { (ACTIONS.iter().position(|a| a.0 == value).unwrap_or(0) as u32, String::new()) };
    combo.set_selected(idx);
    entry.set_text(&text);
    let show = {
        let (entry, rec, folder) = (entry.clone(), rec.clone(), folder.clone());
        move |i: u32| {
            entry.set_visible(i >= n);
            rec.set_visible(i == n);
            folder.set_visible(i == n + 1);
            entry.set_title(if i == n { "Phím tắt, vd: ctrl+shift+t  (hoặc bấm «Ghi phím»)" } else { "Lệnh, vd: firefox  ·  xdg-open ~/Downloads" });
        }
    };
    show(idx);

    let commit = {
        let (st, combo, entry) = (st.clone(), combo.clone(), entry.clone());
        move || {
            let i = combo.selected();
            let t = entry.text().trim().to_string();
            let v = if i < n { ACTIONS[i as usize].0.to_string() } else if i == n { format!("key:{t}") } else { format!("cmd:{t}") };
            edit(&st, |c| *slot_mut(c.buttons.entry(button.into()).or_default(), slot) = v);
        }
    };
    let c1 = commit.clone();
    combo.connect_selected_notify(move |c| { show(c.selected()); c1(); });
    let c2 = commit.clone();
    entry.connect_changed(move |_| c2());

    // Ghi phím: bấm nút rồi bấm tổ hợp phím
    let (w, e) = (win.clone(), entry.clone());
    rec.connect_clicked(move |btn| {
        btn.set_label("Bấm tổ hợp phím…");
        let ctl = gtk::EventControllerKey::new();
        ctl.set_propagation_phase(gtk::PropagationPhase::Capture);
        let (w2, e2, b2) = (w.clone(), e.clone(), btn.clone());
        ctl.connect_key_pressed(move |ctl, key, _, mods| {
            let Some(chord) = chord_text(key, mods) else { return glib::Propagation::Stop };
            e2.set_text(&chord);
            b2.set_label("Ghi phím");
            w2.remove_controller(ctl);
            glib::Propagation::Stop
        });
        w.add_controller(ctl);
    });

    let (w, e) = (win.clone(), entry.clone());
    folder.connect_clicked(move |_| {
        let e = e.clone();
        gtk::FileDialog::builder().title("Chọn thư mục").build().select_folder(Some(&w), None::<&gio::Cancellable>, move |r| {
            if let Ok(f) = r { if let Some(p) = f.path() { e.set_text(&format!("xdg-open \"{}\"", p.display())); } }
        });
    });

    g.add(&combo);
    g.add(&entry);
    [combo.upcast(), entry.upcast()]
}

/// Phím bấm → chữ kiểu "ctrl+shift+t" (bỏ qua khi mới bấm phím bổ trợ)
fn chord_text(key: gdk::Key, mods: gdk::ModifierType) -> Option<String> {
    let name = key.to_lower().name()?.to_string().to_lowercase();
    if ["control_l", "control_r", "shift_l", "shift_r", "alt_l", "alt_r", "super_l", "super_r", "meta_l", "meta_r", "iso_level3_shift"]
        .contains(&name.as_str()) { return None; }
    let k = match name.as_str() {
        "page_up" | "prior" => "pageup", "page_down" | "next" => "pagedown", "return" | "kp_enter" => "enter",
        "escape" => "esc", "iso_left_tab" => "tab", "period" => "dot", "bracketleft" => "leftbrace",
        "bracketright" => "rightbrace", "audioraisevolume" => "volumeup", "audiolowervolume" => "volumedown",
        "audiomute" => "mute", "audioplay" => "playpause", "audionext" => "nextsong", "audioprev" => "previoussong",
        other => other,
    };
    let mut parts = Vec::new();
    if mods.contains(gdk::ModifierType::CONTROL_MASK) { parts.push("ctrl"); }
    if mods.contains(gdk::ModifierType::SHIFT_MASK) { parts.push("shift"); }
    if mods.contains(gdk::ModifierType::ALT_MASK) { parts.push("alt"); }
    if mods.contains(gdk::ModifierType::SUPER_MASK) { parts.push("super"); }
    parts.push(k);
    Some(parts.join("+"))
}
