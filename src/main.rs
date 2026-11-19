#![windows_subsystem = "windows"]

mod console;
mod jiggle;
mod settings;
mod ui;

use clap::Parser;
use jiggle::Mode;
use settings::Settings;
use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
use windows_sys::Win32::System::Threading::CreateMutexW;

fn num(s: &str) -> Result<i32, String> {
    s.parse().map_err(|_| format!("'{s}' is not a valid number."))
}

fn period(s: &str) -> Result<i32, String> {
    match num(s)? {
        v if v < 1 => Err("Period cannot be shorter than 1 second.".into()),
        v if v > 10800 => Err("Period cannot be longer than 10800 seconds.".into()),
        v => Ok(v),
    }
}

fn distance(s: &str) -> Result<i32, String> {
    match num(s)? {
        v if v < 1 => Err("Distance multiplier cannot be less than 1.".into()),
        v if v > 120 => Err("Distance multiplier cannot be greater than 120.".into()),
        v => Ok(v),
    }
}

/// Virtually jiggles the mouse, making the computer seem not idle.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Start with jiggling enabled.
    #[arg(short, long)]
    jiggle: bool,
    /// Start minimized.
    #[arg(short, long)]
    minimized: bool,
    /// Start with the specified jiggle mode enabled.
    #[arg(short = 'o', long, value_enum, ignore_case = true)]
    mode: Option<Mode>,
    /// Start with random variation enabled.
    #[arg(short, long)]
    random: bool,
    /// Set X number of seconds for the jiggle interval.
    #[arg(short, long, value_parser = period)]
    seconds: Option<i32>,
    /// Set the multiplier for the jiggle distance.
    #[arg(short, long, value_parser = distance)]
    distance: Option<i32>,
    /// Start with settings panel displayed.
    #[arg(short = 'g', long)]
    settings: bool,
}

fn main() {
    let attached = console::attach();
    let cli = Cli::try_parse().unwrap_or_else(|e| {
        if e.kind() == clap::error::ErrorKind::DisplayHelp {
            println!("\n"); // original prints a blank-line gap before help
        }
        e.exit()
    });

    // The handle is deliberately leaked: the OS releases the mutex on exit.
    let name: Vec<u16> = "single instance: ArkaneSystems.MouseJiggler\0".encode_utf16().collect();
    unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        println!("Mouse Jiggler is already running. Aborting.");
        std::process::exit(1);
    }

    jiggle::lower_priority();
    let s = Settings::load();
    if attached {
        console::free();
    }
    ui::run(ui::Init {
        jiggle: cli.jiggle,
        minimized: cli.minimized || s.minimize,
        mode: cli.mode.unwrap_or(s.mode),
        random: cli.random || s.random,
        period: cli.seconds.unwrap_or(s.period),
        distance: cli.distance.unwrap_or(s.distance),
        show_settings: cli.settings,
    });
}
