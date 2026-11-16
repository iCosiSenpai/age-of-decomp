//! `aod-cli` — headless Age of Decomp (console program, no window). Used by RecompLauncher's `prepare` and for
//! scripting:
//!
//!   aod-cli verify  --game <file.nup> [--update <file.nup>]
//!   aod-cli prepare --game <file.nup> --out <dir> [--update <nup>] [--dlc <nup>] [--jobs N] [--resolution WxH] [--unlock]
//!   aod-cli play    [--out <dir>]
//!   aod-cli info
//!
//! `prepare` prints `progress <0..1> <text>` lines on stdout (RecompLauncher protocol) and the engine's own output on
//! stderr; exit 0 = built. Exit codes: 2 bad arguments, 3 wrong game file, 4 build failed, 5 cancelled.

use aod_launcher_lib::{config::Config, paths::Paths, prepare, settings};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
        .or_else(|| args.iter().find_map(|a| a.strip_prefix(&format!("{name}=")).map(str::to_string)))
}

fn usage() -> i32 {
    eprintln!("usage: aod-cli verify --game <nup> [--update <nup>] | prepare --game <nup> --out <dir> [--update <nup>] [--dlc <nup>] [--jobs N] [--resolution WxH] [--unlock] | play [--out <dir>] | info");
    2
}

fn verify(game: &str, update: Option<&str>) -> Result<aod_nup::profile::Report, String> {
    let nup = aod_nup::Nup::open(game).map_err(|e| e.to_string())?;
    let upd = update.map(aod_nup::Nup::open).transpose().map_err(|e| e.to_string())?;
    aod_nup::profile::check(&nup, upd.as_ref(), &aod_nup::profile::GameProfile::hwaoi()).map_err(|e| e.to_string())
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
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
        Some("play") => aod_launcher_lib::play_headless(arg(&args, "--out").map(PathBuf::from)),
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
    let paths = Paths::detect();
    let update = arg(args, "--update").filter(|s| !s.is_empty());
    let dlc = arg(args, "--dlc").filter(|s| !s.is_empty());
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
    let job_out = out.clone();
    let job = prepare::Job {
        nup: PathBuf::from(&game),
        update: update.as_ref().map(PathBuf::from),
        dlc: dlc.as_ref().map(PathBuf::from),
        output: PathBuf::from(&out),
        jobs: arg(args, "--jobs").and_then(|j| j.parse().ok()).unwrap_or(0),
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
            let _ = settings::merge_pending(&cfg, &paths);
            // RecompLauncher's `prepare.done` marker: present only after a complete build.
            let marker = serde_json::json!({ "version": aod_launcher_lib::VERSION, "built": aod_launcher_lib::paths::stamp() });
            let _ = std::fs::write(PathBuf::from(&job_out).join("aod-ready.json"), marker.to_string());
            0
        }
        Err(e) if e == "cancelled" => 5,
        Err(e) => {
            eprintln!("{e}");
            4
        }
    }
}
