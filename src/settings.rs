use crate::jiggle::Mode;
use std::path::PathBuf;

/// Persisted user settings (same names/defaults as the original app), stored as `Key=Value` lines.
pub struct Settings {
    pub minimize: bool,
    pub period: i32,
    pub random: bool,
    pub mode: Mode,
    pub distance: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { minimize: false, period: 60, random: false, mode: Mode::Normal, distance: 1 }
    }
}

fn path() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("MouseJiggler").join("settings.ini"))
}

impl Settings {
    pub fn load() -> Settings {
        let mut s = Settings::default();
        let text = path().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
        for (k, v) in text.lines().filter_map(|l| l.split_once('=')) {
            let v = v.trim();
            match k.trim() {
                "MinimizeOnStartup" => s.minimize = v.eq_ignore_ascii_case("true"),
                "RandomTimer" => s.random = v.eq_ignore_ascii_case("true"),
                "JigglePeriod" => s.period = v.parse().ok().filter(|p| (1..=10800).contains(p)).unwrap_or(s.period),
                "JiggleDistance" => s.distance = v.parse().ok().filter(|d| (1..=120).contains(d)).unwrap_or(s.distance),
                "JiggleMode" => s.mode = Mode::parse(v).unwrap_or(s.mode),
                _ => {}
            }
        }
        s
    }

    pub fn save(&self) {
        let Some(p) = path() else { return };
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let _ = std::fs::write(
            p,
            format!(
                "MinimizeOnStartup={}\nJigglePeriod={}\nRandomTimer={}\nJiggleMode={}\nJiggleDistance={}\n",
                self.minimize, self.period, self.random, self.mode, self.distance
            ),
        );
    }
}
