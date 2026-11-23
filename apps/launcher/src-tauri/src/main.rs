// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // RecompLauncher's setup step (`cli prepare …`) and the other commands: the same program players start.
    if args.first().is_some_and(|a| a == "cli") {
        std::process::exit(aod_launcher_lib::cli::run(args));
    }
    if args.iter().any(|a| a == "--play") {
        // RecompLauncher's `start`: play directly, no launcher window.
        let game_dir = args.iter().position(|a| a == "--game-dir").and_then(|i| args.get(i + 1)).map(std::path::PathBuf::from);
        // The player's copy (RecompLauncher's {input:nup}): `--game=<nup>` or `--game <nup>`.
        let copy = args.iter().find_map(|a| a.strip_prefix("--game=").map(str::to_string))
            .or_else(|| args.iter().position(|a| a == "--game").and_then(|i| args.get(i + 1)).cloned())
            .filter(|c| !c.is_empty()).map(std::path::PathBuf::from);
        // The player's NVIDIA DLSS file (RecompLauncher's {input:dlss}, only on RTX PCs): `--dlss=<file>`.
        let dlss = args.iter().find_map(|a| a.strip_prefix("--dlss=").map(str::to_string))
            .or_else(|| args.iter().position(|a| a == "--dlss").and_then(|i| args.get(i + 1)).cloned())
            .filter(|d| !d.is_empty()).map(std::path::PathBuf::from);
        std::process::exit(aod_launcher_lib::play_headless(game_dir, copy, dlss));
    }
    aod_launcher_lib::run()
}
