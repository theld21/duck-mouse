// Chuột giả để thử, theo lệnh: wheel:N  click:BTN  drag:BTN:DX:DY  sleep:MS   (BTN: 274 giữa, 275 hông sau, 276 hông trước)
use evdev::{uinput::VirtualDeviceBuilder, AttributeSet, EventType, InputEvent, Key, RelativeAxisType};
use std::{thread::sleep, time::Duration};
fn main() {
    let mut k = AttributeSet::<Key>::new(); for c in 0x110u16..=0x114 { k.insert(Key::new(c)); }
    let mut r = AttributeSet::<RelativeAxisType>::new(); for c in [0u16, 1, 6, 8, 11, 12] { r.insert(RelativeAxisType(c)); }
    let mut d = VirtualDeviceBuilder::new().unwrap().name("fake test mouse").with_keys(&k).unwrap().with_relative_axes(&r).unwrap().build().unwrap();
    let rel = |c, v| InputEvent::new(EventType::RELATIVE, c, v);
    let key = |c, v| InputEvent::new(EventType::KEY, c, v);
    let ms = |m| sleep(Duration::from_millis(m));
    for cmd in std::env::args().skip(1) {
        let p: Vec<i32> = cmd.split(':').skip(1).map(|x| x.parse().unwrap()).collect();
        eprintln!("{cmd}");
        match cmd.split(':').next().unwrap() {
            "sleep" => ms(p[0] as u64),
            "wheel" => for _ in 0..p[0].abs() { d.emit(&[rel(8, p[0].signum()), rel(11, 120 * p[0].signum())]).unwrap(); ms(40) },
            "click" => { d.emit(&[key(p[0] as u16, 1)]).unwrap(); ms(60); d.emit(&[key(p[0] as u16, 0)]).unwrap() }
            "drag" => {
                d.emit(&[key(p[0] as u16, 1)]).unwrap(); ms(250);
                for _ in 0..20 { d.emit(&[rel(0, p[1] / 20), rel(1, p[2] / 20)]).unwrap(); ms(8) }
                d.emit(&[key(p[0] as u16, 0)]).unwrap()
            }
            "flick" => {   // ấn rồi kéo ngay thật nhanh (~50 ms) rồi nhả
                d.emit(&[key(p[0] as u16, 1)]).unwrap(); ms(5);
                for _ in 0..10 { d.emit(&[rel(0, p[1] / 10), rel(1, p[2] / 10)]).unwrap(); ms(5) }
                d.emit(&[key(p[0] as u16, 0)]).unwrap()
            }
            "holdwheel" => {   // giữ nút, lăn N nấc (âm = xuống), nhả
                d.emit(&[key(p[0] as u16, 1)]).unwrap(); ms(100);
                for _ in 0..p[1].abs() { d.emit(&[rel(8, p[1].signum()), rel(11, 120 * p[1].signum())]).unwrap(); ms(40) }
                ms(50); d.emit(&[key(p[0] as u16, 0)]).unwrap()
            }
            "chord" => {   // giữ nút A, bấm nút B, nhả A
                d.emit(&[key(p[0] as u16, 1)]).unwrap(); ms(150);
                d.emit(&[key(p[1] as u16, 1)]).unwrap(); ms(60); d.emit(&[key(p[1] as u16, 0)]).unwrap();
                ms(100); d.emit(&[key(p[0] as u16, 0)]).unwrap()
            }
            _ => panic!("lệnh lạ"),
        }
    }
}
