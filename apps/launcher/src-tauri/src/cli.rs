//! The command line: `AgeOfDecomp cli …` and `aod-cli …` run this (no window). RecompLauncher's setup step is
//! `AgeOfDecomp cli prepare …`, the program players start (its guide: "Ship one program"); aod-cli.exe stays for a
//! console and for the launcher's own updates (`--apply-update`).
//!
//!   cli verify  --game <file.nup> [--update <file.nup>]
//!   cli prepare --game <file.nup> --out <dir> [--update <nup>] [--dlc <nup>] [--jobs N] [--resolution auto|WxH] [--unlock]
//!               [--fetch-frame-gen --accept-nvidia-license]
//!   cli play    [--out <dir>] [--game <nup>] [--dlss <nvngx_dlss.dll>]
//!   cli info
//!
//! `prepare` prints `progress <0..1> <text>` lines on stdout (RecompLauncher protocol) and the engine's own output on
//! stderr; exit 0 = built. Exit codes: 2 bad arguments, 3 wrong game file, 4 build failed, 5 cancelled.
//! `--fetch-frame-gen` (RecompLauncher's setup option "NVIDIA frame generation") also downloads NVIDIA's
//! `nvngx_dlssg.dll` from NVIDIA's DLSS repository into `<data>/ngx`, and only together with
//! `--accept-nvidia-license`: ticking the option is the player accepting NVIDIA's licence. A download that fails does
//! not stop the setup: the game then plays without frame generation and its log says why.
//!
//! On Windows the launcher is a program without a console: its output reaches the pipes of whoever started it
//! (RecompLauncher reads them), not a console window; aod-cli.exe is the one to run in a console.

use crate::{config::Config, dlss, paths::Paths, prepare};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
        .or_else(|| args.iter().find_map(|a| a.strip_prefix(&format!("{name}=")).map(str::to_string)))
}

fn parse_jobs(args: &[String]) -> Result<u32, String> {
    let value = arg(args, "--jobs");
    if value.is_none() && !args.iter().any(|a| a == "--jobs") {
        return Ok(0);
    }
    value.and_then(|s| s.parse::<u32>().ok()).filter(|n| *n <= 8)
        .ok_or_else(|| "--jobs must be an integer from 0 to 8".into())
}

fn usage() -> i32 {
    eprintln!("usage: aod-cli|AgeOfDecomp cli verify --game <nup> [--update <nup>] | prepare --game <nup> --out <dir> [--update <nup>] [--dlc <nup>] [--jobs N] [--resolution auto|WxH] [--unlock] [--fetch-frame-gen --accept-nvidia-license] | play [--out <dir>] [--game <nup>] [--dlss <nvngx_dlss.dll>] | info");
    2
}

fn verify(game: &str, update: Option<&str>) -> Result<aod_nup::profile::Report, String> {
    let nup = aod_nup::Nup::open(game).map_err(|e| e.to_string())?;
    let upd = update.map(aod_nup::Nup::open).transpose().map_err(|e| e.to_string())?;
    aod_nup::profile::check(&nup, upd.as_ref(), &aod_nup::profile::GameProfile::hwaoi()).map_err(|e| e.to_string())
}

/// Runs one command (`args` without the program's name; a leading `cli` is skipped) and returns the exit code.
pub fn run(mut args: Vec<String>) -> i32 {
    // `AgeOfDecomp cli …`, `aod-cli cli …` and the Linux AppImage's `cli …` take the same arguments everywhere.
    if args.first().is_some_and(|a| a == "cli") {
        args.remove(0);
    }
    match args.first().map(String::as_str) {
        Some("verify") => match arg(&args, "--game") {
            None => usage(),
            Some(g) => match verify(&g, arg(&args, "--update").as_deref()) {
                Ok(r) => {
                    println!("{}", serde_json::to_string_pretty(&r).unwrap());
                    if r.ok { 0 } else { 3 }
                }
                Err(e) => {
                    eprintln!("{e}");
                    3
                }
            },
        },
        Some("prepare") => prepare_cmd(&args),
        Some("play") => crate::play_headless(arg(&args, "--out").map(PathBuf::from),
                                                  arg(&args, "--game").filter(|g| !g.is_empty()).map(PathBuf::from),
                                                  arg(&args, "--dlss").filter(|d| !d.is_empty()).map(PathBuf::from)),
        Some("info") => {
            let p = Paths::detect();
            println!("version {}\nengine  {:?}\ndata    {}", crate::VERSION, p.engine, p.data.display());
            0
        }
        _ => usage(),
    }
}

/// `--fetch-frame-gen`: NVIDIA's frame generation file, from NVIDIA, into `<data>/ngx` (where `--play` looks for it).
/// Err: the flag came without `--accept-nvidia-license`, so nothing may be downloaded.
fn fetch_frame_gen(args: &[String], paths: &Paths) -> Result<(), String> {
    if !args.iter().any(|a| a == "--fetch-frame-gen") {
        return Ok(());
    }
    if !args.iter().any(|a| a == "--accept-nvidia-license") {
        return Err(format!("--fetch-frame-gen downloads NVIDIA's file only with --accept-nvidia-license ({})", dlss::LICENSE_URL));
    }
    if !cfg!(windows) || dlss::file(paths, dlss::Kind::FrameGen).is_file() {
        return Ok(());
    }
    println!("progress 0.00 Downloading NVIDIA's frame generation file");
    let _ = std::io::stdout().flush();
    if let Err(e) = dlss::download(paths, dlss::Kind::FrameGen, |_| {}) {
        eprintln!("{e}");
    }
    Ok(())
}

fn prepare_cmd(args: &[String]) -> i32 {
    let (Some(game), Some(out)) = (arg(args, "--game"), arg(args, "--out")) else { return usage() };
    let jobs = match parse_jobs(args) {
        Ok(jobs) => jobs,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };
    let absolute = |value: &str| std::path::absolute(value).map(|p| p.to_string_lossy().into_owned());
    let (game, out) = match (absolute(&game), absolute(&out)) {
        (Ok(game), Ok(out)) => (game, out),
        _ => { eprintln!("Could not resolve the game or output path"); return 2; }
    };
    let paths = Paths::detect();
    let update = arg(args, "--update").filter(|s| !s.is_empty()).map(|v| absolute(&v)).transpose();
    let dlc = arg(args, "--dlc").filter(|s| !s.is_empty()).map(|v| absolute(&v)).transpose();
    let (update, dlc) = match (update, dlc) {
        (Ok(update), Ok(dlc)) => (update, dlc),
        _ => { eprintln!("Could not resolve the update or DLC path"); return 2; }
    };
    if let Err(e) = fetch_frame_gen(args, &paths) {
        eprintln!("{e}");
        return 2;
    }
    println!("progress 0.00 Checking your game file");
    match verify(&game, update.as_deref()) {
        Ok(r) if r.ok => {}
        Ok(r) => {
            for s in r.steps.iter().filter(|s| !s.ok) {
                eprintln!("check {}: {}", s.id, s.detail);
            }
            eprintln!("This is not the supported build: the base game and update 1.0.3 (NCM 196608) are needed.");
            return 3;
        }
        Err(e) => {
            eprintln!("{e}");
            return 3;
        }
    }
    let job = prepare::Job {
        nup: PathBuf::from(&game),
        update: update.as_ref().map(PathBuf::from),
        dlc: dlc.as_ref().map(PathBuf::from),
        output: PathBuf::from(&out),
        jobs,
        resolution: arg(args, "--resolution").unwrap_or_else(|| "auto".into()),
        unlock: args.iter().any(|a| a == "--unlock"),
    };
    let control = Arc::new(prepare::Control::default());
    let mut last = -1.0f64;
    let result = prepare::run(&paths, &job, control, move |e| match e.kind {
        "progress" => {
            let f = e.fraction.unwrap_or(0.0);
            if f - last >= 0.005 || e.stage.is_some() && f == 0.0 {
                last = f;
                println!("progress {:.3} {}", f, e.text.unwrap_or_default());
                let _ = std::io::stdout().flush();
            }
        }
        "log" => eprintln!("{}", e.line.unwrap_or_default()),
        "done" => println!("progress 1.000 Ready to play"),
        _ => {}
    });
    match result {
        Ok(()) => {
            // Remember the build, so the Age of Decomp window shows it as ready too.
            let mut cfg = Config::load(&paths);
            cfg.nup = Some(game);
            cfg.update_nup = update;
            cfg.dlc_nup = dlc;
            cfg.set_output(out);
            // The build itself is done and plays: this is said and not counted as a failed setup.
            if let Err(e) = cfg.save(&paths) {
                eprintln!("The game is built, but the Age of Decomp window could not be told where it is ({e}).");
            }
            0
        }
        Err(e) if e == "cancelled" => 5,
        Err(e) => {
            eprintln!("{e}");
            4
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvidias_file_is_fetched_only_with_the_licence_accepted() {
        let args = |values: &[&str]| values.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let dir = std::env::temp_dir().join(format!("aod-cli-fg-{}", std::process::id()));
        let paths = Paths { exe_dir: dir.clone(), data: dir.join("data"), engine: None };
        // without the option nothing happens; with it alone nothing is downloaded and the step stops
        assert_eq!(fetch_frame_gen(&args(&["prepare"]), &paths), Ok(()));
        assert!(fetch_frame_gen(&args(&["prepare", "--fetch-frame-gen"]), &paths).is_err());
        assert!(!dlss::file(&paths, dlss::Kind::FrameGen).exists());
        // a file that is already there is kept as it is (no download)
        let file = dlss::file(&paths, dlss::Kind::FrameGen);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, b"kept").unwrap();
        assert_eq!(fetch_frame_gen(&args(&["prepare", "--fetch-frame-gen", "--accept-nvidia-license"]), &paths), Ok(()));
        assert_eq!(std::fs::read(&file).unwrap(), b"kept");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn jobs_must_be_in_range() {
        let args = |values: &[&str]| values.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(parse_jobs(&args(&[])), Ok(0));
        assert_eq!(parse_jobs(&args(&["--jobs", "0"])), Ok(0));
        assert_eq!(parse_jobs(&args(&["--jobs=8"])), Ok(8));
        for values in [&["--jobs", "9"][..], &["--jobs", "-1"], &["--jobs", "x"], &["--jobs"], &["--jobs="]] {
            assert!(parse_jobs(&args(values)).is_err(), "{values:?}");
        }
    }
}
