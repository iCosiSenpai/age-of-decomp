//! `aod-cli` — headless Age of Decomp (console program, no window). Used by RecompLauncher's `prepare` and for
//! scripting:
//!
//!   aod-cli verify  --game <file.nup> [--update <file.nup>]
//!   aod-cli prepare --game <file.nup> --out <dir> [--update <nup>] [--dlc <nup>] [--jobs N] [--resolution WxH] [--unlock]
//!   aod-cli play    [--out <dir>] [--game <nup>] [--dlss <nvngx_dlss.dll>]
//!   aod-cli info
//!
//! `prepare` prints `progress <0..1> <text>` lines on stdout (RecompLauncher protocol) and the engine's own output on
//! stderr; exit 0 = built. Exit codes: 2 bad arguments, 3 wrong game file, 4 build failed, 5 cancelled.

use aod_launcher_lib::{config::Config, paths::Paths, prepare};
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
    eprintln!("usage: aod-cli verify --game <nup> [--update <nup>] | prepare --game <nup> --out <dir> [--update <nup>] [--dlc <nup>] [--jobs N] [--resolution WxH] [--unlock] | play [--out <dir>] [--game <nup>] [--dlss <nvngx_dlss.dll>] | info");
    2
}

fn verify(game: &str, update: Option<&str>) -> Result<aod_nup::profile::Report, String> {
    let nup = aod_nup::Nup::open(game).map_err(|e| e.to_string())?;
    let upd = update.map(aod_nup::Nup::open).transpose().map_err(|e| e.to_string())?;
    aod_nup::profile::check(&nup, upd.as_ref(), &aod_nup::profile::GameProfile::hwaoi()).map_err(|e| e.to_string())
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--apply-update") {
        let result = args.get(1).ok_or("Missing update staging directory".to_string())
            .and_then(|path| aod_launcher_lib::update::apply(std::path::Path::new(path)));
        if let Err(e) = result { eprintln!("{e}"); std::process::exit(1); }
        return;
    }
    // The Linux AppImage dispatches `cli …` to this program; the same RecompLauncher arguments then work everywhere.
    if args.first().is_some_and(|a| a == "cli") {
        args.remove(0);
    }
    let code = match args.first().map(String::as_str) {
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
        Some("play") => aod_launcher_lib::play_headless(arg(&args, "--out").map(PathBuf::from),
                                                  arg(&args, "--game").filter(|g| !g.is_empty()).map(PathBuf::from),
                                                  arg(&args, "--dlss").filter(|d| !d.is_empty()).map(PathBuf::from)),
        Some("info") => {
            let p = Paths::detect();
            println!("version {}\nengine  {:?}\ndata    {}", aod_launcher_lib::VERSION, p.engine, p.data.display());
            0
        }
        _ => usage(),
    };
    std::process::exit(code);
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
    println!("progress 0.00 Checking your game file");
    match verify(&game, update.as_deref()) {
        Ok(r) if r.ok => {}
        Ok(r) => {
            for s in r.steps.iter().filter(|s| !s.ok) {
                eprintln!("check {}: {}", s.id, s.detail);
            }
            eprintln!("This is not the supported build: the base game and update 3.0.0 (NCM 196608) are needed.");
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
        resolution: arg(args, "--resolution").unwrap_or_else(|| "1920x1080".into()),
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
            cfg.output = Some(out);
            let _ = cfg.save(&paths);
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
