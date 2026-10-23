use clap::ValueEnum;
use std::fmt;
use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_MOVE, MOUSEINPUT};
use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Mode {
    #[default]
    Normal,
    Zen,
    Circle,
    Linear,
}

impl Mode {
    pub const ALL: [Mode; 4] = [Mode::Normal, Mode::Zen, Mode::Circle, Mode::Linear];

    pub fn parse(s: &str) -> Option<Mode> {
        Mode::ALL.into_iter().find(|m| m.to_string().eq_ignore_ascii_case(s))
    }

    /// Relative mouse moves for one full cycle, scaled by the distance multiplier `d`.
    pub fn pattern(self, d: i32) -> Vec<(i32, i32)> {
        let p: &[(i32, i32)] = match self {
            Mode::Normal => &[(4, 4), (-4, -4)],
            Mode::Zen => &[(0, 0)],
            Mode::Circle => &[(3, 2), (2, 3), (-2, 3), (-3, 2), (-3, -2), (-2, -3), (2, -3), (3, -2)],
            Mode::Linear => &[(4, 0), (-4, 0)],
        };
        p.iter().map(|&(x, y)| (x * d, y * d)).collect()
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Mode::Normal => "Normal",
            Mode::Zen => "Zen",
            Mode::Circle => "Circle",
            Mode::Linear => "Linear",
        })
    }
}

pub fn stay_awake() {
    unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED) };
}

pub fn allow_sleep() {
    unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
}

pub fn jiggle(dx: i32, dy: i32) {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 { mi: MOUSEINPUT { dx, dy, mouseData: 0, dwFlags: MOUSEEVENTF_MOVE, time: 0, dwExtraInfo: 0 } },
    };
    unsafe { SendInput(1, &input, std::mem::size_of::<INPUT>() as i32) };
}

pub fn cursor_pos() -> Option<(i32, i32)> {
    let mut p = POINT { x: 0, y: 0 };
    (unsafe { GetCursorPos(&mut p) } != 0).then_some((p.x, p.y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_scale_and_cycle_to_zero() {
        for m in Mode::ALL {
            let p = m.pattern(7);
            assert_eq!(p.iter().map(|v| v.0).sum::<i32>(), 0);
            assert_eq!(p.iter().map(|v| v.1).sum::<i32>(), 0);
        }
        assert_eq!(Mode::Circle.pattern(2)[0], (6, 4));
        assert_eq!(Mode::parse("cIrCle"), Some(Mode::Circle));
    }
}
