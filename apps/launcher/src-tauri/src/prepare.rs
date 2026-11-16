//! The one-time build of the game from the player's NUP, driven through the engine bundle's own workflow
//! (`source/compiler/backend.py build`). Its stage headers and progress lines become one overall fraction:
//!   [1/6] identity + tools → [2/6] ExeFS → [3/6] RomFS cache (`RomFS n/N`) → [3/6] DLC → [4/6] ARM64→C
//!   (`emitted n/N chunks`) → [5/6] compile + link (Ninja `[n/N]`) → [6/6] translator + settings → BUILD COMPLETE.
//! Re-running reuses every verified stage, so "stop" and "continue" are free.

use crate::config::Config;
use crate::paths::{self, Paths};
use serde::Serialize;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Share of the total time per stage, measured on a Core Ultra 7 265K (sync 43 s, RomFS 21 s, C 30 s, compile 298 s).
const WEIGHTS: [f64; 7] = [0.11, 0.01, 0.06, 0.005, 0.08, 0.73, 0.005];
pub const STAGE_TEXT: [&str; 7] = [
    "Checking the game files",
    "Extracting the game code",
    "Preparing the game resources",
    "Installing DLC",
    "Translating the ARM64 code to C",
    "Compiling for your PC",
    "Shaders and settings",
];

#[derive(Clone, Debug, Serialize)]
pub struct Event {
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fraction: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_s: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta_s: Option<f64>,
}

impl Event {
    fn new(kind: &'static str) -> Event {
        Event { kind, stage: None, fraction: None, text: None, line: None, elapsed_s: None, eta_s: None }
    }
}

pub struct Job {
    pub nup: PathBuf,
    pub update: Option<PathBuf>,
    pub dlc: Option<PathBuf>,
    pub output: PathBuf,
    pub jobs: u32,
    pub resolution: String,
    pub unlock: bool,
}

impl Job {
    pub fn from_config(cfg: &Config, paths: &Paths, resolution: String) -> Result<Job, String> {
        Ok(Job {
            nup: cfg.nup.as_ref().map(PathBuf::from).ok_or("Choose your game file first")?,
            update: cfg.update_nup.as_ref().map(PathBuf::from),
            dlc: cfg.dlc_nup.as_ref().map(PathBuf::from),
            output: cfg.output(paths),
            jobs: cfg.ui.jobs.min(8),
            resolution,
            unlock: cfg.ui.rate_unlock,
        })
    }
}

#[derive(Default)]
pub struct Control {
    pub cancel: AtomicBool,
    pub pid: AtomicU32,
}

fn stage_of(line: &str) -> Option<u32> {
    let rest = line.strip_prefix('[')?;
    let (n, tail) = rest.split_once("/6]")?;
    let n: u32 = n.parse().ok()?;
    Some(match n {
        1 | 2 => n,
        3 if tail.contains("DLC") => 4,
        3 => 3,
        4 => 5,
        5 => 6,
        6 => 7,
        _ => return None,
    })
}

/// "RomFS 201/2153, …", "emitted 200/362 chunks, …", "[12/485] CC …" → done/total.
fn sub_progress(stage: u32, line: &str) -> Option<f64> {
    let ratio = |s: &str| -> Option<f64> {
        let (a, b) = s.split_once('/')?;
        let a: f64 = a.trim().parse().ok()?;
        let b: f64 = b.trim().parse().ok()?;
        (b > 0.0).then(|| (a / b).clamp(0.0, 1.0))
    };
    match stage {
        3 => ratio(line.strip_prefix("RomFS ")?.split(',').next()?),
        5 => ratio(line.strip_prefix("emitted ")?.split(' ').next()?),
        6 => ratio(line.strip_prefix('[')?.split(']').next()?),
        _ => None,
    }
}

fn overall(stage: u32, sub: f64) -> f64 {
    let i = (stage.clamp(1, 7) - 1) as usize;
    let before: f64 = WEIGHTS[..i].iter().sum();
    (before + WEIGHTS[i] * sub).clamp(0.0, 0.999)
}

/// Runs the build and calls `sink` with progress, log lines and the final result. Blocking.
pub fn run(paths: &Paths, job: &Job, control: Arc<Control>, mut sink: impl FnMut(Event) + Send) -> Result<(), String> {
    let engine = paths.engine.clone().ok_or("The engine is missing: reinstall Age of Decomp")?;
    if !cfg!(windows) {
        return Err("Building on Linux is not available in this version yet (see docs/BUILDING.md).".into());
    }
    if !job.nup.is_file() {
        return Err(format!("Game file not found: {}", job.nup.display()));
    }
    std::fs::create_dir_all(paths.logs()).map_err(|e| e.to_string())?;
    let log_path = paths.logs().join(format!("prepare-{}.log", paths::stamp()));
    let log = Arc::new(Mutex::new(std::fs::File::create(&log_path).map_err(|e| e.to_string())?));

    let python = engine.join("python").join("python.exe");
    let mut cmd = Command::new(&python);
    cmd.arg("-E").arg("-s").arg(engine.join("source/compiler/backend.py")).arg("build");
    cmd.arg("--bundle").arg(&engine).arg("--nup").arg(&job.nup).arg("--output").arg(&job.output);
    cmd.arg("--jobs").arg(job.jobs.to_string()).arg("--resolution").arg(&job.resolution);
    if job.unlock {
        cmd.arg("--unlock");
    }
    if let Some(u) = &job.update {
        cmd.arg("--update-nup").arg(u);
    }
    if let Some(d) = &job.dlc {
        cmd.arg("--dlc-nup").arg(d);
    }
    cmd.current_dir(&engine).env("PYTHONUTF8", "1").env("PYTHONNOUSERSITE", "1");
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().map_err(|e| format!("Could not start the engine: {e}"))?;
    control.pid.store(child.id(), Ordering::SeqCst);

    // stderr carries `ERROR: …`; read it on its own thread so neither pipe can fill up.
    let err_lines = Arc::new(Mutex::new(Vec::<String>::new()));
    let stderr = child.stderr.take().unwrap();
    let (err_store, err_log) = (err_lines.clone(), log.clone());
    let err_thread = std::thread::spawn(move || {
        for line in BufReader::new(stderr).split(b'\n').map_while(Result::ok) {
            let line = String::from_utf8_lossy(&line).trim_end().to_string();
            let _ = writeln!(err_log.lock().unwrap(), "{line}");
            err_store.lock().unwrap().push(line);
        }
    });

    let started = Instant::now();
    let mut stage = 1u32;
    let mut last_emit = Instant::now() - std::time::Duration::from_secs(1);
    let mut emit_progress = |stage: u32, frac: f64, sink: &mut dyn FnMut(Event), force: bool| {
        if !force && last_emit.elapsed().as_millis() < 150 {
            return;
        }
        last_emit = Instant::now();
        let elapsed = started.elapsed().as_secs_f64();
        let mut e = Event::new("progress");
        e.stage = Some(stage);
        e.fraction = Some(frac);
        e.text = Some(STAGE_TEXT[(stage - 1) as usize].to_string());
        e.elapsed_s = Some(elapsed);
        e.eta_s = (frac > 0.04).then(|| elapsed / frac * (1.0 - frac));
        sink(e);
    };
    emit_progress(1, 0.0, &mut sink, true);

    let stdout = child.stdout.take().unwrap();
    let mut complete = false;
    for raw in BufReader::new(stdout).split(b'\n').map_while(Result::ok) {
        // Engine output is read as bytes: dump names can hold characters a code page cannot show.
        let line = String::from_utf8_lossy(&raw).trim_end().to_string();
        let _ = writeln!(log.lock().unwrap(), "{line}");
        if let Some(s) = stage_of(&line) {
            stage = s;
            emit_progress(stage, overall(stage, 0.0), &mut sink, true);
        } else if let Some(sub) = sub_progress(stage, &line) {
            emit_progress(stage, overall(stage, sub), &mut sink, false);
        }
        if line.starts_with("BUILD COMPLETE") {
            complete = true;
        }
        let mut e = Event::new("log");
        e.line = Some(line);
        sink(e);
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let _ = err_thread.join();
    control.pid.store(0, Ordering::SeqCst);
    if control.cancel.swap(false, Ordering::SeqCst) {
        sink(Event::new("cancelled"));
        return Err("cancelled".into());
    }
    if status.success() && complete {
        let mut e = Event::new("done");
        e.fraction = Some(1.0);
        e.elapsed_s = Some(started.elapsed().as_secs_f64());
        sink(e);
        Ok(())
    } else {
        let errs = err_lines.lock().unwrap();
        let msg = errs
            .iter()
            .rev()
            .find_map(|l| l.strip_prefix("ERROR: ").map(str::to_string))
            .unwrap_or_else(|| format!("The engine stopped (exit {}). Log: {}", status.code().unwrap_or(-1), log_path.display()));
        let mut e = Event::new("error");
        e.text = Some(msg.clone());
        e.stage = Some(stage);
        sink(e);
        Err(msg)
    }
}

/// Stops a running build (the whole process tree: Python, Ninja, clang).
pub fn cancel(control: &Control) {
    let pid = control.pid.load(Ordering::SeqCst);
    if pid == 0 {
        return;
    }
    control.cancel.store(true, Ordering::SeqCst);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill").args(["/T", "/F", "/PID", &pid.to_string()]).creation_flags(0x0800_0000).status();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill").args(["-TERM", &pid.to_string()]).status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stages_and_progress() {
        assert_eq!(stage_of("[3/6] Preparing the read-only game resource cache..."), Some(3));
        assert_eq!(stage_of("[3/6] Installing DLC (add-on content)..."), Some(4));
        assert_eq!(stage_of("[5/6] Compiling and linking"), Some(6));
        assert_eq!(sub_progress(3, "RomFS 1001/2153, cache 2.40 GiB, 6.0s").map(|f| (f * 1000.0) as u32), Some(464));
        assert_eq!(sub_progress(5, "emitted 362/362 chunks, 26s"), Some(1.0));
        assert_eq!(sub_progress(6, "[1/485] CC work/generated/main_table.c").map(|f| f > 0.0), Some(true));
        assert!(overall(7, 1.0) < 1.0 && overall(1, 0.0) == 0.0);
    }
}
