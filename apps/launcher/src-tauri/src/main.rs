// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--play") {
        // RecompLauncher's `start`: play directly, no launcher window.
        let game_dir = args.iter().position(|a| a == "--game-dir").and_then(|i| args.get(i + 1)).map(std::path::PathBuf::from);
        std::process::exit(aod_launcher_lib::play_headless(game_dir));
    }
    aod_launcher_lib::run()
}
