// Cuộn mượt – thuật toán của Mos (Caldis, CC BY-NC 4.0) qua DuckMouse (Vduck), chuyển sang Linux:
// mỗi nấc bánh xe cộng vào "đích" → mỗi nhịp đi 1 phần quãng còn lại (chậm dần) → làm mềm đầu cú đẩy
// → phát REL_WHEEL_HI_RES nhỏ (1 nấc = 120), giữ phần lẻ để không mất quãng.
use evdev::{uinput::VirtualDevice, EventType, InputEvent};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

const REL_HWHEEL: u16 = 0x06;
const REL_WHEEL: u16 = 0x08;
const REL_WHEEL_HI_RES: u16 = 0x0b;
const REL_HWHEEL_HI_RES: u16 = 0x0c;
const DEAD: f64 = 0.5;

#[derive(Default, Clone, Copy)]
struct Axis {
    target: f64,
    current: f64,
    filt: [f64; 2],
    acc: f64, // phần lẻ chưa phát (đơn vị hi-res)
    low: i32, // cộng dồn hi-res để phát REL_WHEEL cũ mỗi 120
}

struct State {
    axes: [Axis; 2], // 0 = dọc, 1 = ngang
    trans: f64,
    alpha: f64,
    period: Duration,
    active: bool,
}

pub struct Scroller {
    st: Mutex<State>,
    cv: Condvar,
}

impl Scroller {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            st: Mutex::new(State { axes: [Axis::default(); 2], trans: 0.1, alpha: 0.23, period: Duration::from_millis(7), active: false }),
            cv: Condvar::new(),
        })
    }

    /// Nhận 1 cú lăn (đơn vị hi-res, 120 = 1 nấc). axis 0 = dọc, 1 = ngang
    pub fn push(&self, axis: usize, v: f64, speed: f64, duration: f64, hz: f64) {
        let hz = hz.clamp(30.0, 500.0);
        // Công thức Mos (ở 60 Hz): trans = 1 − √(duration / 5,2) → đổi sang nhịp hz để cảm giác như nhau
        let t60 = (1.0 - (duration.clamp(0.0, 5.1) / 5.2).sqrt()).max(0.001);
        let k = 60.0 / hz;
        let mut s = self.st.lock().unwrap();
        s.trans = 1.0 - (1.0 - t60).powf(k);
        s.alpha = 1.0 - (1.0f64 - 0.23).powf(k);
        s.period = Duration::from_secs_f64(1.0 / hz);
        let a = &mut s.axes[axis];
        let v = v * speed;
        let remaining = a.target - a.current;
        if remaining != 0.0 && (remaining > 0.0) != (v > 0.0) {
            *a = Axis::default(); // đổi chiều → bỏ quãng cũ
        }
        a.target += v;
        s.active = true;
        self.cv.notify_one();
    }

    /// Dừng ngay (khi tắt cuộn mượt / mất chuột)
    pub fn reset(&self) {
        let mut s = self.st.lock().unwrap();
        s.axes = [Axis::default(); 2];
        s.active = false;
    }

    /// Luồng phát cuộn: ngủ khi không có gì để cuộn
    pub fn run(self: Arc<Self>, mouse: Arc<Mutex<VirtualDevice>>) {
        let mut next = Instant::now();
        loop {
            let mut out: Vec<InputEvent> = Vec::with_capacity(4);
            let period;
            {
                let mut s = self.st.lock().unwrap();
                if !s.active {
                    while !s.active { s = self.cv.wait(s).unwrap(); }
                    next = Instant::now();
                }
                period = s.period;
                let (trans, alpha) = (s.trans, s.alpha);
                let mut busy = false;
                for (i, a) in s.axes.iter_mut().enumerate() {
                    let step = (a.target - a.current) * trans;
                    a.current += step;
                    // ScrollFilter của Mos: chia cú đẩy đầu ra vài nhịp để không giật
                    let first = a.filt[1];
                    a.filt = [first, first + alpha * (step - first)];
                    a.acc += first;
                    let n = a.acc.trunc();
                    a.acc -= n;
                    let n = n as i32;
                    if n != 0 {
                        let (hi, lo) = if i == 0 { (REL_WHEEL_HI_RES, REL_WHEEL) } else { (REL_HWHEEL_HI_RES, REL_HWHEEL) };
                        out.push(InputEvent::new(EventType::RELATIVE, hi, n));
                        a.low += n;
                        let notches = a.low / 120;
                        if notches != 0 {
                            a.low -= notches * 120;
                            out.push(InputEvent::new(EventType::RELATIVE, lo, notches));
                        }
                    }
                    if (a.target - a.current).abs() > DEAD || a.filt[1].abs() > DEAD / 4.0 { busy = true; }
                }
                if !busy {
                    s.axes = [Axis::default(); 2];
                    s.active = false;
                }
            }
            if !out.is_empty() {
                if let Ok(mut m) = mouse.lock() { let _ = m.emit(&out); }
            }
            next += period;
            let now = Instant::now();
            if next > now { std::thread::sleep(next - now); } else { next = now; }
        }
    }
}
