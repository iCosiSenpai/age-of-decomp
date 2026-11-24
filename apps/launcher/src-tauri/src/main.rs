// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args = aod_launcher_lib::args().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    // RecompLauncher's setup step (`cli prepare …`) and its `start` (`--play …`): the same program players start.
    if let Some(code) = aod_launcher_lib::headless(&args) {
        std::process::exit(code);
    }
    aod_launcher_lib::run()
}
