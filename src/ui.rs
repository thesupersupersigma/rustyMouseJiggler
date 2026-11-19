#![allow(deprecated)] // nwg::Timer is fine for coarse ticks

use crate::jiggle::{self, Mode};
use crate::settings::Settings;
use native_windows_gui as nwg;
use nwg::NumberSelectData as Nsd;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct Init {
    pub jiggle: bool,
    pub minimized: bool,
    pub mode: Mode,
    pub random: bool,
    pub period: i32,
    pub distance: i32,
    pub show_settings: bool,
}

const ICON: &[u8] = include_bytes!("../app.ico");
const ABOUT: &str = "MouseJiggler\nVersion 3.0.0.0\nCopyright © Alistair J. R. Young 2007-2026\n\n\
Virtually jiggles the mouse, making the computer seem not idle.\n\n\
Command line: --jiggle, --minimized, --mode <normal|zen|circle|linear>, --random, --seconds <1-10800>, --distance <1-120>, --settings.";

#[derive(Default)]
struct App {
    window: nwg::Window,
    jiggling: nwg::CheckBox,
    show_settings: nwg::CheckBox,
    about: nwg::Button,
    tray_btn: nwg::Button,
    panel: nwg::Frame,
    mode: nwg::ComboBox<Mode>,
    random: nwg::CheckBox,
    lb_period: nwg::Label,
    lb_interval: nwg::Label,
    period: nwg::NumberSelect,
    lb_distance: nwg::Label,
    distance: nwg::NumberSelect,
    minimize: nwg::CheckBox,
    icon: nwg::Icon,
    tray: nwg::TrayNotification,
    menu: nwg::Menu,
    mi_open: nwg::MenuItem,
    mi_start: nwg::MenuItem,
    mi_stop: nwg::MenuItem,
    mi_exit: nwg::MenuItem,
    timer: nwg::Timer,
    poll: nwg::Timer,

    settings: RefCell<Settings>,
    pattern: RefCell<Vec<(i32, i32)>>,
    step: Cell<usize>,
    last: Cell<Option<(i32, i32)>>,
    first_shown: Cell<bool>,
}

/// NWG's NumberSelect only updates its value from the +/- buttons, so parse and commit what the user typed.
/// While the box has focus, out-of-range/partial input is left alone; once focus leaves it is clamped or reverted.
fn int(n: &nwg::NumberSelect) -> i32 {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetFocus;
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowExW, GetWindowTextW, SendMessageW};
    const EM_GETSEL: u32 = 0x00B0;
    const EM_SETSEL: u32 = 0x00B1;

    let Nsd::Int { value, step, max, min } = n.data() else { unreachable!() };
    let Some(parent) = n.handle.hwnd() else { return value as i32 };
    let edit = unsafe { FindWindowExW(parent as _, std::ptr::null_mut(), windows_sys::w!("Edit"), std::ptr::null()) };
    if edit.is_null() {
        return value as i32;
    }
    let mut buf = [0u16; 32];
    let len = unsafe { GetWindowTextW(edit, buf.as_mut_ptr(), buf.len() as i32) };
    let text = String::from_utf16_lossy(&buf[..len.max(0) as usize]);
    let focused = unsafe { GetFocus() } == edit;
    let new = match text.trim().parse::<i64>() {
        Ok(v) if (min..=max).contains(&v) => v,
        Ok(v) if !focused => v.clamp(min, max),
        _ if !focused => value,
        _ => return value as i32,
    };
    if new != value || text != new.to_string() {
        let sel = unsafe { SendMessageW(edit, EM_GETSEL, 0, 0) };
        n.set_data(Nsd::Int { value: new, step, max, min });
        unsafe { SendMessageW(edit, EM_SETSEL, sel as usize & 0xFFFF, (sel >> 16) & 0xFFFF) };
    }
    new as i32
}

impl App {
    fn is_jiggling(&self) -> bool {
        self.jiggling.check_state() == nwg::CheckBoxState::Checked
    }

    fn set_jiggling(&self, on: bool) {
        self.jiggling.set_check_state(if on { nwg::CheckBoxState::Checked } else { nwg::CheckBoxState::Unchecked });
        self.on_jiggling();
    }

    fn on_jiggling(&self) {
        let on = self.is_jiggling();
        if on { jiggle::stay_awake() } else { jiggle::allow_sleep() }
        self.step.set(0);
        if on {
            self.last.set(jiggle::cursor_pos());
            self.timer.start();
        } else {
            self.timer.stop();
        }
        self.mi_start.set_enabled(!on);
        self.mi_stop.set_enabled(on);
        self.update_tip();
    }

    fn update_tip(&self) {
        let s = self.settings.borrow();
        let tip = if self.is_jiggling() {
            let t = format!(
                "Jiggling mouse every {} s,{} mode: {} (Δ {}).",
                s.period,
                if s.random { " with random variation," } else { "" },
                s.mode,
                s.distance
            );
            // Tray tooltips are limited to 63 UTF-16 units.
            if t.chars().count() > 63 { format!("{}...", t.chars().take(60).collect::<String>()) } else { t }
        } else {
            "Not jiggling the mouse.".into()
        };
        self.tray.set_tip(&tip);
    }

    fn tick(&self) {
        let now = jiggle::cursor_pos();
        if now == self.last.get() {
            let p = self.pattern.borrow();
            let (dx, dy) = p[self.step.get() % p.len()];
            self.step.set((self.step.get() + 1) % p.len());
            jiggle::jiggle(dx, dy);
        }
        self.last.set(jiggle::cursor_pos());

        let s = self.settings.borrow();
        let secs = if s.random { 1 + nanos() % s.period as u32 } else { s.period as u32 };
        self.timer.set_interval(secs * 1000);
        self.timer.start();
        self.lb_period.set_text(&format!("{secs} s"));
    }

    /// Apply changes made through the settings controls (NumberSelect has no change event, so this is polled).
    fn sync(&self) {
        let (period, distance) = (int(&self.period), int(&self.distance));
        let mode = self.mode.selection().map_or(Mode::Normal, |i| Mode::ALL[i]);
        let random = self.random.check_state() == nwg::CheckBoxState::Checked;
        let minimize = self.minimize.check_state() == nwg::CheckBoxState::Checked;
        let mut s = self.settings.borrow_mut();
        if (s.period, s.distance, s.mode, s.random, s.minimize) == (period, distance, mode, random, minimize) {
            return;
        }
        let rebuild = (s.mode, s.distance) != (mode, distance);
        let reset = s.period != period || s.random != random;
        *s = Settings { minimize, period, random, mode, distance };
        s.save();
        drop(s);
        if rebuild {
            *self.pattern.borrow_mut() = mode.pattern(distance);
            self.step.set(0);
        }
        if reset {
            self.timer.set_interval(period as u32 * 1000);
            self.lb_period.set_text(&format!("{period} s"));
            if self.is_jiggling() {
                self.timer.start();
            }
        }
        self.update_tip();
    }

    fn to_tray(&self) {
        self.window.set_visible(false);
        self.tray.set_visibility(true);
        // The settings controls can't change while hidden, so stop polling them.
        self.poll.stop();
        jiggle::trim_memory();
    }

    fn restore(&self) {
        self.tray.set_visibility(false);
        self.window.set_visible(true);
        self.window.restore();
        self.window.set_focus();
        self.poll.start();
    }

    fn toggle_settings(&self) {
        let on = self.show_settings.check_state() == nwg::CheckBoxState::Checked;
        self.panel.set_visible(on);
        // NWG treats Window sizes as client sizes: 305x170 / 305x44 as in the original layout.
        self.window.set_size(305, if on { 170 } else { 44 });
    }
}

fn nanos() -> u32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos() / 1000)
}

fn num_select(parent: &nwg::Frame, out: &mut nwg::NumberSelect, pos: (i32, i32), v: i32, max: i64) {
    nwg::NumberSelect::builder()
        .parent(parent)
        .position(pos)
        .size((147, 23))
        .value_int(v as i64)
        .step_int(1)
        .min_int(1)
        .max_int(max)
        .build(out)
        .unwrap();
}

pub fn run(init: Init) {
    unsafe { nwg::set_dpi_awareness() }; // original is SystemAware high-DPI
    nwg::init().expect("failed to init GUI");
    nwg::enable_visual_styles();
    // Segoe UI 9pt, the WinForms default the original uses.
    let mut font = nwg::Font::default();
    if nwg::Font::builder().family("Segoe UI").size_absolute(12).build(&mut font).is_ok() {
        nwg::Font::set_global_default(Some(font));
    }

    let mut app = App::default();
    // The original clamps out-of-range values to 1 rather than rejecting them.
    let period = init.period.clamp(1, 10800);
    let distance = init.distance.clamp(1, 120);

    nwg::Icon::builder().source_bin(Some(ICON)).build(&mut app.icon).unwrap();
    nwg::Window::builder()
        .title("Mouse Jiggler")
        .size((305, 44))
        .flags(if init.minimized { nwg::WindowFlags::WINDOW } else { nwg::WindowFlags::WINDOW | nwg::WindowFlags::VISIBLE })
        .icon(Some(&app.icon))
        .center(true)
        .build(&mut app.window)
        .unwrap();

    let w = &app.window;
    nwg::CheckBox::builder().parent(w).text("Jiggling?").position((11, 13)).size((72, 19)).build(&mut app.jiggling).unwrap();
    nwg::CheckBox::builder().parent(w).text("Settings...").position((96, 13)).size((77, 19)).build(&mut app.show_settings).unwrap();
    nwg::Button::builder().parent(w).text("?").position((206, 10)).size((40, 23)).build(&mut app.about).unwrap();
    nwg::Button::builder().parent(w).text("🔽").position((252, 10)).size((40, 23)).build(&mut app.tray_btn).unwrap();
    nwg::Frame::builder().parent(w).position((8, 42)).size((289, 120)).flags(nwg::FrameFlags::VISIBLE).build(&mut app.panel).unwrap();

    let p = &app.panel;
    nwg::ComboBox::builder()
        .parent(p)
        .position((10, 7))
        .size((115, 23))
        .collection(Mode::ALL.to_vec())
        .selected_index(Mode::ALL.iter().position(|m| *m == init.mode))
        .build(&mut app.mode)
        .unwrap();
    nwg::CheckBox::builder().parent(p).text("Randomize?").position((138, 11)).size((90, 19)).build(&mut app.random).unwrap();
    nwg::Label::builder().parent(p).text(&format!("{period} s")).position((238, 12)).size((45, 15)).h_align(nwg::HTextAlign::Right).build(&mut app.lb_period).unwrap();
    nwg::Label::builder().parent(p).text("Interval (s):").position((10, 40)).size((65, 15)).build(&mut app.lb_interval).unwrap();
    num_select(p, &mut app.period, (136, 36), period, 10800);
    nwg::Label::builder().parent(p).text("Distance (multiplier):").position((10, 69)).size((117, 15)).build(&mut app.lb_distance).unwrap();
    num_select(p, &mut app.distance, (136, 65), distance, 120);
    nwg::CheckBox::builder().parent(p).text("Minimize on start?").position((138, 94)).size((123, 19)).build(&mut app.minimize).unwrap();

    nwg::TrayNotification::builder().parent(&app.window).icon(Some(&app.icon)).tip(Some("Not jiggling the mouse.")).build(&mut app.tray).unwrap();
    nwg::Menu::builder().parent(&app.window).popup(true).text("tray").build(&mut app.menu).unwrap();
    for (item, text) in [(&mut app.mi_open, "Open"), (&mut app.mi_start, "Start Jiggling"), (&mut app.mi_stop, "Stop Jiggling"), (&mut app.mi_exit, "Exit")] {
        nwg::MenuItem::builder().parent(&app.menu).text(text).build(item).unwrap();
    }
    nwg::Timer::builder().parent(&app.window).interval(period as u32 * 1000).stopped(true).build(&mut app.timer).unwrap();
    nwg::Timer::builder().parent(&app.window).interval(250).stopped(false).build(&mut app.poll).unwrap();

    *app.settings.borrow_mut() = Settings { minimize: init.minimized, period, random: init.random, mode: init.mode, distance };
    *app.pattern.borrow_mut() = init.mode.pattern(distance);
    if init.random {
        app.random.set_check_state(nwg::CheckBoxState::Checked);
    }
    if init.minimized {
        app.minimize.set_check_state(nwg::CheckBoxState::Checked);
    }
    if init.show_settings {
        app.show_settings.set_check_state(nwg::CheckBoxState::Checked);
    }
    app.panel.set_visible(init.show_settings);
    app.toggle_settings();
    app.mi_stop.set_enabled(false);
    app.update_tip();

    let app = Rc::new(app);
    let a = app.clone();
    let handler = nwg::full_bind_event_handler(&app.window.handle, move |evt, _data, handle| {
        use nwg::Event as E;
        match evt {
            E::OnWindowClose if handle == a.window.handle => nwg::stop_thread_dispatch(),
            E::OnTimerTick if handle == a.timer.handle => a.tick(),
            E::OnTimerTick if handle == a.poll.handle => a.sync(),
            E::OnButtonClick if handle == a.jiggling.handle => a.on_jiggling(),
            E::OnButtonClick if handle == a.show_settings.handle => a.toggle_settings(),
            E::OnButtonClick if handle == a.tray_btn.handle => a.to_tray(),
            E::OnButtonClick if handle == a.about.handle => {
                nwg::modal_info_message(&a.window, "About MouseJiggler", ABOUT);
            }
            // Tray: left click restores (the original used double-click), right click opens the menu.
            E::OnMousePress(nwg::MousePressEvent::MousePressLeftUp) if handle == a.tray.handle => a.restore(),
            E::OnContextMenu if handle == a.tray.handle => {
                let (x, y) = nwg::GlobalCursor::position();
                a.menu.popup(x, y);
            }
            E::OnMenuItemSelected if handle == a.mi_open.handle => a.restore(),
            E::OnMenuItemSelected if handle == a.mi_start.handle => a.set_jiggling(true),
            E::OnMenuItemSelected if handle == a.mi_stop.handle => a.set_jiggling(false),
            E::OnMenuItemSelected if handle == a.mi_exit.handle => nwg::stop_thread_dispatch(),
            _ => {}
        }
    });

    app.first_shown.set(true);
    if init.jiggle {
        app.set_jiggling(true);
    }
    if init.minimized {
        app.to_tray();
    }

    nwg::dispatch_thread_events();
    nwg::unbind_event_handler(&handler);
    jiggle::allow_sleep();
}
