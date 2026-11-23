//! The one-time build of the game from the player's NUP, driven through the engine bundle's own workflow
//! (`source/compiler/backend.py build`). Its stage headers and progress lines become one overall fraction:
//!   [1/6] identity + tools → [2/6] ExeFS → [3/6] RomFS cache (`RomFS n/N`) → [3/6] DLC → [4/6] ARM64→C
//!   (`emitted n/N chunks`) → [5/6] compile + link (Ninja `[n/N]`) → [6/6] translator + settings → BUILD COMPLETE.
//! Re-running reuses every verified stage, so "stop" and "continue" are free.

use crate::config::Config;
use crate::paths::{self, Paths};
use serde::Serialize;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
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

#[cfg(windows)]
fn assign_job(child: &std::process::Child) -> Result<(), String> {
    use std::os::windows::io::AsRawHandle;
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_PRIORITY_CLASS, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };
    use windows_sys::Win32::System::Threading::BELOW_NORMAL_PRIORITY_CLASS;

    static JOB: OnceLock<Result<usize, String>> = OnceLock::new();
    let handle = *JOB.get_or_init(|| unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return Err(format!("Could not create the build job: {}", std::io::Error::last_os_error()));
        }
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        // Below normal for the engine and everything it starts (Python, Ninja, clang, the shader preparation): an
        // automatic build uses all processors but one in eight (the engine's hostmem.build_threads), and the desktop,
        // a game or a call still gets the processor first.
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_PRIORITY_CLASS;
        limits.BasicLimitInformation.PriorityClass = BELOW_NORMAL_PRIORITY_CLASS;
        if SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        ) == 0 {
            let error = std::io::Error::last_os_error();
            CloseHandle(job);
            return Err(format!("Could not configure the build job: {error}"));
        }
        Ok(job as usize)
    }).as_ref().map_err(Clone::clone)?;
    if unsafe { AssignProcessToJobObject(handle as _, child.as_raw_handle() as _) } == 0 {
        return Err(format!("Could not assign the engine to the build job: {}", std::io::Error::last_os_error()));
    }
    Ok(())
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

/// The folder next to a game folder where its saves and logs wait while the engine builds: `<game folder>.kept`, on
/// the same disk, so they are moved there and never copied.
pub fn kept_dir(output: &Path) -> PathBuf {
    let mut name = output.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(".kept");
    output.with_file_name(name)
}

/// What may be in a game folder before its first build and is the player's, not a build's: settings (RecompLauncher
/// writes `hwaoi.ini` as soon as a setting changes on the game's page), saves (RecompLauncher puts a backup or the
/// cloud's saves into the folder its manifest names, also before the setup has run) and logs (a Play that stopped
/// before there was a game). The engine only builds into an empty folder and never overwrites what it finds, so these
/// are put aside before it runs: the settings go into the pending settings (merged back by `settings::merge_pending`
/// after the build), the saves and logs into `kept_dir` (put back by `put_back_kept_files`), and the folders they
/// leave empty go. Anything else in the folder, and any link, stops the setup here with its name and nothing moved.
pub fn set_aside_early_files(paths: &Paths, output: &Path) -> Result<(), String> {
    if output.join("hwaoi-compiler.json").is_file() || !output.is_dir() || output.parent().is_none() {
        return Ok(()); // an engine build folder (updates are the engine's business), or nothing there yet
    }
    let io = |e: std::io::Error| e.to_string();
    let ini = paths::game_ini(output);
    let saves = output.join("work").join("saves");
    let logs = paths::game_logs_dir(output);
    let (mut early_ini, mut has_saves, mut has_logs) = (false, false, false);
    let mut others = Vec::new();
    let mut stack = vec![output.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).map_err(io)? {
            let e = e.map_err(io)?;
            let kind = e.file_type().map_err(io)?;
            let p = e.path();
            if kind.is_symlink() {
                others.push(p); // a link is the player's own arrangement: never followed, never moved
            } else if kind.is_dir() {
                if p == saves {
                    has_saves = true; // the whole folder is the player's: it moves as it is
                } else if p == logs {
                    has_logs = true;
                } else {
                    stack.push(p);
                }
            } else if p == ini {
                early_ini = true;
            } else if !(d == output && e.file_name() == ".hwaoi-compiler.lock") {
                others.push(p); // the lock is the engine's own, which it allows in an empty folder
            }
        }
    }
    if !others.is_empty() {
        others.sort();
        let names: Vec<String> =
            others.iter().take(3).map(|p| p.strip_prefix(output).unwrap_or(p).display().to_string()).collect();
        return Err(format!(
            "The game folder {} has files that are not from a build ({}{}). Move them somewhere else, then run the \
             setup again: the game is only built into an empty folder.",
            output.display(),
            names.join(", "),
            if others.len() > 3 { ", ..." } else { "" }
        ));
    }
    // Both settings files are read before anything moves: an unreadable one stops here with everything in its place.
    let early = if early_ini { crate::settings::existing_ini(&ini)? } else { None };
    let mut pending = match early {
        Some(_) => Some(crate::settings::load_ini(&paths.pending_ini())?),
        None => None,
    };
    if has_saves || has_logs {
        let kept = kept_dir(output);
        fs::create_dir_all(&kept).map_err(io)?;
        for (from, name) in [(&saves, "saves"), (&logs, "logs")] {
            if !from.is_dir() {
                continue;
            }
            let to = kept.join(name);
            if to.exists() {
                // What a setup that never ended put aside: it stays under a name of its own, never overwritten.
                let older = kept.join(format!("{name}-{}-{}", paths::stamp(), std::process::id()));
                fs::rename(&to, older).map_err(io)?;
            }
            fs::rename(from, &to).map_err(io)?;
        }
        eprintln!(
            "Age of Decomp: the game folder had saves or logs before its first build; they wait in {} and go back \
             when the build is over",
            kept.display()
        );
    }
    if let (Some(early), Some(pending)) = (early, pending.as_mut()) {
        crate::settings::carry(&early, pending); // the newer choice wins; a key without a row here goes along
        pending.save(&paths.pending_ini()).map_err(|e| e.to_string())?;
        fs::remove_file(&ini).map_err(io)?;
    }
    prune_empty_dirs(output, output);
    Ok(())
}

/// Removes the folders below `root` that hold nothing; `root` itself stays.
fn prune_empty_dirs(dir: &Path, root: &Path) {
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            if e.file_type().is_ok_and(|t| t.is_dir() && !t.is_symlink()) {
                prune_empty_dirs(&e.path(), root);
            }
        }
    }
    if dir != root {
        let _ = fs::remove_dir(dir); // fails on a folder that is not empty: then it stays
    }
}

/// Whether a folder holds a file anywhere below it (a missing folder holds none).
fn holds_files(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else { return false };
    entries.flatten().any(|e| match e.file_type() {
        Ok(t) if t.is_dir() && !t.is_symlink() => holds_files(&e.path()),
        _ => true,
    })
}

/// Puts back what `set_aside_early_files` put aside: after the engine has run, however that ended, and before the
/// game starts (a setup that was killed leaves them waiting). Saves go back only into a folder that holds none: one
/// that has saves of its own keeps them, and the ones put aside stay where they wait. Logs go back file by file.
/// Returns what to tell the player.
pub fn put_back_kept_files(output: &Path) -> Vec<String> {
    let kept = kept_dir(output);
    if !kept.is_dir() {
        return Vec::new();
    }
    let mut told = Vec::new();
    let (from, to) = (kept.join("saves"), output.join("work").join("saves"));
    if from.is_dir() {
        if holds_files(&to) {
            told.push(format!(
                "the saves that were in the game folder before its first build stay in {}: the folder has saves of \
                 its own now",
                from.display()
            ));
        } else {
            let _ = fs::remove_dir_all(&to); // empty folders only: `holds_files` said so
            let moved = to.parent().map_or(Ok(()), fs::create_dir_all).and_then(|_| fs::rename(&from, &to));
            told.push(match moved {
                Ok(()) => format!("the saves that were in the game folder before its first build are back in {}", to.display()),
                Err(e) => format!("the saves put aside for the build could not go back ({e}): they are in {}", from.display()),
            });
        }
    }
    let (from, to) = (kept.join("logs"), paths::game_logs_dir(output));
    if from.is_dir() {
        if !to.exists() && fs::rename(&from, &to).is_ok() {
            // back as it was
        } else if fs::create_dir_all(&to).is_ok() {
            for e in fs::read_dir(&from).into_iter().flatten().flatten() {
                let target = to.join(e.file_name());
                if !target.exists() {
                    let _ = fs::rename(e.path(), target); // a name that is there already stays behind
                }
            }
            let _ = fs::remove_dir(&from);
        }
    }
    let _ = fs::remove_dir(&kept); // only when nothing waits in it any more
    told
}

/// Puts the saves and logs back when `run` is over, on every way out of it.
struct PutBack<'a>(&'a Path);

impl Drop for PutBack<'_> {
    fn drop(&mut self) {
        for line in put_back_kept_files(self.0) {
            eprintln!("Age of Decomp: {line}");
        }
    }
}

/// Runs the build and calls `sink` with progress, log lines and the final result. Blocking.
pub fn run(paths: &Paths, job: &Job, control: Arc<Control>, mut sink: impl FnMut(Event) + Send) -> Result<(), String> {
    control.cancel.store(false, Ordering::SeqCst);
    let absolute = |p: &Path| std::path::absolute(p).map_err(|e| e.to_string());
    let engine = absolute(paths.engine.as_deref().ok_or("The engine is missing: reinstall Age of Decomp")?)?;
    // The bundle's own Python (Windows: NuGet CPython, Linux: python-build-standalone); on Linux the compiler, Ninja
    // and the libraries are the system's, which the engine checks before it starts.
    let python = if cfg!(windows) { engine.join("python").join("python.exe") } else { engine.join("python/bin/python3") };
    if !python.is_file() {
        return Err(format!("The engine is incomplete ({} is missing): reinstall Age of Decomp", python.display()));
    }
    if !job.nup.is_file() {
        return Err(format!("Game file not found: {}", job.nup.display()));
    }
    let version = crate::config::prepare_version();
    let marker = job.output.join(format!("aod-ready-v{version}.json"));
    match fs::remove_file(&marker) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
        _ => {}
    }
    set_aside_early_files(paths, &job.output)?;
    let _put_back = PutBack(&job.output);
    std::fs::create_dir_all(paths.logs()).map_err(|e| e.to_string())?;
    let log_path = paths.logs().join(format!("prepare-{}.log", paths::stamp()));
    let log = Arc::new(Mutex::new(std::fs::File::create(&log_path).map_err(|e| e.to_string())?));

    let mut cmd = Command::new(&python);
    cmd.arg("-E").arg("-s").arg(engine.join("source/compiler/backend.py")).arg("build");
    cmd.arg("--bundle").arg(&engine).arg("--nup").arg(absolute(&job.nup)?).arg("--output").arg(absolute(&job.output)?);
    cmd.arg("--jobs").arg(job.jobs.to_string()).arg("--resolution").arg(&job.resolution);
    if job.unlock {
        cmd.arg("--unlock");
    }
    if let Some(u) = &job.update {
        cmd.arg("--update-nup").arg(absolute(u)?);
    }
    if let Some(d) = &job.dlc {
        cmd.arg("--dlc-nup").arg(absolute(d)?);
    }
    cmd.current_dir(&engine).env("PYTHONUTF8", "1").env("PYTHONNOUSERSITE", "1");
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0); // a group of its own, so `cancel` stops Python, Ninja and clang together
        #[cfg(target_os = "linux")]
        {
            let parent = unsafe { libc::getpid() };
            // SIGKILL of aod-cli still leaves Ninja / clang running: the process group outlives it.
            unsafe {
                cmd.pre_exec(move || {
                    // A lower priority for the engine and its children, as the Windows job object gives them: the
                    // build uses most processors and the desktop still comes first. A failure changes nothing else.
                    libc::nice(10);
                    if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM, 0, 0, 0) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    if libc::getppid() != parent {
                        return Err(std::io::Error::from_raw_os_error(libc::ESRCH));
                    }
                    Ok(())
                });
            }
        }
    }
    let mut child = cmd.spawn().map_err(|e| format!("Could not start the engine: {e}"))?;
    #[cfg(windows)]
    if let Err(e) = assign_job(&child) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(e);
    }
    control.pid.store(child.id(), Ordering::SeqCst);
    if control.cancel.load(Ordering::SeqCst) {
        cancel(&control); // Cancel was pressed before the engine had started
    }

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
    let cancelled = control.cancel.swap(false, Ordering::SeqCst);
    if status.success() && complete {
        let cfg = Config { output: Some(job.output.to_string_lossy().into_owned()), ..Config::default() };
        crate::settings::merge_pending(&cfg, paths)?;
        let part = marker.with_extension(format!("{}.part", std::process::id()));
        let result = serde_json::json!({ "prepare_version": version, "compiler_version": crate::VERSION, "built": paths::stamp() });
        fs::write(&part, result.to_string()).and_then(|_| fs::rename(part, marker)).map_err(|e| e.to_string())?;
        let mut e = Event::new("done");
        e.fraction = Some(1.0);
        e.elapsed_s = Some(started.elapsed().as_secs_f64());
        sink(e);
        Ok(())
    } else if cancelled {
        sink(Event::new("cancelled"));
        Err("cancelled".into())
    } else {
        crate::diagnostics::build_failed(paths, stage);
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
    // Kept also while the engine has not started yet: `run` stops it as soon as it has.
    control.cancel.store(true, Ordering::SeqCst);
    let pid = control.pid.load(Ordering::SeqCst);
    if pid == 0 {
        return;
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill").args(["/T", "/F", "/PID", &pid.to_string()]).creation_flags(0x0800_0000).status();
    }
    #[cfg(not(windows))]
    {
        // The build runs in its own process group (see `run`): signal the group, not only Python.
        let _ = Command::new("kill").args(["-TERM", "--", &format!("-{pid}")]).status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_clears_an_old_cancel_before_preflight() {
        let p = Paths { exe_dir: PathBuf::new(), data: PathBuf::new(), engine: None, portable: false };
        let job = Job {
            nup: PathBuf::new(), update: None, dlc: None, output: PathBuf::new(), jobs: 0,
            resolution: "1920x1080".into(), unlock: false,
        };
        let control = Arc::new(Control::default());
        control.cancel.store(true, Ordering::SeqCst);
        assert!(run(&p, &job, control.clone(), |_| {}).is_err());
        assert!(!control.cancel.load(Ordering::SeqCst));
    }

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

#[cfg(test)]
mod early_settings_tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("aod-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn paths_in(data: &std::path::Path) -> Paths {
        Paths { exe_dir: data.to_path_buf(), data: data.to_path_buf(), engine: None, portable: false }
    }

    #[test]
    fn moves_a_lone_settings_file_into_pending() {
        let root = temp("early");
        let (data, game) = (root.join("data"), root.join("game"));
        let p = paths_in(&data);
        let ini = paths::game_ini(&game);
        std::fs::create_dir_all(ini.parent().unwrap()).unwrap();
        std::fs::write(&ini, "fps_limit=30\nvolume=9999\nlod_guard = 1\n").unwrap();
        set_aside_early_files(&p, &game).unwrap();
        assert!(!ini.exists() && !game.join("work").exists(), "the game folder is empty again");
        let pending = crate::ini::Ini::load(&p.pending_ini()).unwrap().values();
        assert_eq!(pending.get("fps_limit").map(String::as_str), Some("30"));
        assert!(!pending.contains_key("volume"), "a value the launcher would refuse for its own setting is left out");
        // A key the runtime reads and the launcher has no row for is the player's own line: it goes along.
        assert_eq!(pending.get("lod_guard").map(String::as_str), Some("1"));
        // After the build both reach the game's settings file.
        std::fs::create_dir_all(ini.parent().unwrap()).unwrap();
        std::fs::write(&ini, "[display]\noutput_res = auto\n").unwrap();
        let cfg = Config { output: Some(game.to_string_lossy().into_owned()), ..Config::default() };
        crate::settings::merge_pending(&cfg, &p).unwrap();
        let merged = crate::ini::Ini::load(&ini).unwrap().values();
        assert_eq!(merged.get("fps_limit").map(String::as_str), Some("30"));
        assert_eq!(merged.get("lod_guard").map(String::as_str), Some("1"));
        assert_eq!(merged.get("output_res").map(String::as_str), Some("auto"));
        assert!(!p.pending_ini().exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn leaves_folders_with_other_files_alone() {
        let root = temp("early2");
        let (data, game) = (root.join("data"), root.join("game"));
        let p = paths_in(&data);
        let ini = paths::game_ini(&game);
        std::fs::create_dir_all(ini.parent().unwrap()).unwrap();
        std::fs::write(&ini, "fps_limit=30\n").unwrap();
        std::fs::write(game.join("notes.txt"), "player file").unwrap();
        std::fs::create_dir_all(paths::saves_dir(&game)).unwrap();
        std::fs::write(paths::saves_dir(&game).join("account.uid"), "0123456789abcdef").unwrap();
        // A file that is nobody's we know stops the setup with its name, and nothing has moved.
        let refused = set_aside_early_files(&p, &game).unwrap_err();
        assert!(refused.contains("notes.txt") && refused.contains("not from a build"), "{refused}");
        assert!(ini.exists() && game.join("notes.txt").exists() && paths::saves_dir(&game).join("account.uid").exists());
        assert!(!kept_dir(&game).exists() && !p.pending_ini().exists());
        let _ = std::fs::remove_dir_all(root);
    }

    /// A game folder as RecompLauncher can leave it before the first build: the cloud's saves, a log and a setting.
    fn early_folder(game: &std::path::Path) -> (PathBuf, PathBuf, PathBuf) {
        let ini = paths::game_ini(game);
        let save = paths::saves_dir(game).join("save").join("ed7e40e4").join("DATA.BIN");
        let log = paths::game_logs_dir(game).join("launch-1.log");
        for (file, text) in [(&ini, "fps_limit=30\n"), (&save, "a save"), (&log, "not built yet")] {
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        }
        std::fs::write(paths::saves_dir(game).join("account.uid"), "0123456789abcdef").unwrap();
        std::fs::create_dir_all(paths::saves_dir(game).join("cache")).unwrap();
        (ini, save, log)
    }

    #[test]
    fn saves_and_logs_wait_beside_the_folder_and_come_back() {
        let root = temp("early-saves");
        let (data, game) = (root.join("data"), root.join("game"));
        let p = paths_in(&data);
        let (ini, save, log) = early_folder(&game);
        set_aside_early_files(&p, &game).unwrap();
        assert!(game.is_dir() && std::fs::read_dir(&game).unwrap().next().is_none(), "the engine gets an empty folder");
        let kept = kept_dir(&game);
        assert_eq!(kept, root.join("game.kept"));
        assert_eq!(std::fs::read_to_string(kept.join("saves/hwaoi196608/save/ed7e40e4/DATA.BIN")).unwrap(), "a save");
        assert!(kept.join("saves/hwaoi196608/cache").is_dir() && kept.join("logs/launch-1.log").is_file());
        assert!(p.pending_ini().is_file() && !ini.exists());
        // The engine builds: its marker and its own folders are there when the saves come back.
        std::fs::create_dir_all(game.join("work/build")).unwrap();
        std::fs::write(game.join("hwaoi-compiler.json"), "{}").unwrap();
        let told = put_back_kept_files(&game);
        assert!(told.len() == 1 && told[0].contains("are back in"), "{told:?}");
        assert_eq!(std::fs::read_to_string(&save).unwrap(), "a save");
        assert!(paths::saves_dir(&game).join("account.uid").is_file() && paths::saves_dir(&game).join("cache").is_dir());
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "not built yet");
        assert!(!kept.exists(), "nothing waits any more");
        assert!(put_back_kept_files(&game).is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_setup_that_was_killed_loses_no_save() {
        let root = temp("early-killed");
        let (data, game) = (root.join("data"), root.join("game"));
        let p = paths_in(&data);
        let (_, save, _) = early_folder(&game);
        set_aside_early_files(&p, &game).unwrap(); // ... and the setup is killed here: nothing is put back
        // The next setup finds the folder empty and builds; RecompLauncher has put the saves there again meanwhile.
        std::fs::create_dir_all(save.parent().unwrap()).unwrap();
        std::fs::write(&save, "a newer save").unwrap();
        set_aside_early_files(&p, &game).unwrap();
        let kept = kept_dir(&game);
        let older: Vec<_> = std::fs::read_dir(&kept).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("saves-")).collect();
        assert_eq!(older.len(), 1, "the first set stays under a name of its own");
        assert_eq!(std::fs::read_to_string(kept.join(&older[0]).join("hwaoi196608/save/ed7e40e4/DATA.BIN")).unwrap(), "a save");
        put_back_kept_files(&game);
        assert_eq!(std::fs::read_to_string(&save).unwrap(), "a newer save");
        assert!(kept.join(&older[0]).is_dir(), "and is never deleted");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn saves_put_aside_never_replace_saves_that_are_there() {
        let root = temp("early-other-save");
        let (data, game) = (root.join("data"), root.join("game"));
        let p = paths_in(&data);
        let (_, save, log) = early_folder(&game);
        set_aside_early_files(&p, &game).unwrap();
        std::fs::create_dir_all(save.parent().unwrap()).unwrap();
        std::fs::write(&save, "played since").unwrap();
        std::fs::create_dir_all(log.parent().unwrap()).unwrap();
        std::fs::write(log.with_file_name("game-2.log"), "a start").unwrap();
        let told = put_back_kept_files(&game);
        assert!(told.len() == 1 && told[0].contains("stay in"), "{told:?}");
        assert_eq!(std::fs::read_to_string(&save).unwrap(), "played since");
        assert_eq!(std::fs::read_to_string(kept_dir(&game).join("saves/hwaoi196608/save/ed7e40e4/DATA.BIN")).unwrap(), "a save");
        assert!(log.is_file() && log.with_file_name("game-2.log").is_file(), "the logs are together again");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_folder_the_engine_built_is_left_to_the_engine() {
        let root = temp("early-built");
        let (data, game) = (root.join("data"), root.join("game"));
        let p = paths_in(&data);
        let (ini, save, log) = early_folder(&game);
        std::fs::write(game.join("hwaoi-compiler.json"), "{}").unwrap();
        set_aside_early_files(&p, &game).unwrap();
        assert!(ini.is_file() && save.is_file() && log.is_file() && !kept_dir(&game).exists());
        let _ = std::fs::remove_dir_all(root);
    }

    /// A directory link (junction on Windows, symlink elsewhere); false where this system cannot make one.
    fn dir_link(target: &std::path::Path, link: &std::path::Path) -> bool {
        #[cfg(windows)]
        return std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .is_ok_and(|o| o.status.success());
        #[cfg(unix)]
        return std::os::unix::fs::symlink(target, link).is_ok();
    }

    /// Removes the link itself, never what it points at.
    fn unlink_dir(link: &std::path::Path) {
        std::fs::remove_dir(link).or_else(|_| std::fs::remove_file(link)).unwrap();
    }

    #[test]
    fn settings_behind_a_link_are_left_alone() {
        let root = temp("early-link");
        let (data, game, outside) = (root.join("data"), root.join("game"), root.join("outside"));
        let p = paths_in(&data);
        let ini = paths::game_ini(&game);
        let inside = ini.strip_prefix(game.join("work")).unwrap();
        std::fs::create_dir_all(outside.join(inside).parent().unwrap()).unwrap();
        std::fs::write(outside.join(inside), "fps_limit=30\n").unwrap();
        std::fs::create_dir_all(&game).unwrap();
        if !dir_link(&outside, &game.join("work")) {
            let _ = std::fs::remove_dir_all(root);
            return; // no directory links on this system
        }
        assert!(ini.is_file(), "the link resolves");
        assert!(set_aside_early_files(&p, &game).is_err());
        assert!(outside.join(inside).is_file() && !p.pending_ini().exists());
        // A link anywhere else in the folder is something the player put there: also left alone.
        unlink_dir(&game.join("work"));
        std::fs::create_dir_all(ini.parent().unwrap()).unwrap();
        std::fs::write(&ini, "fps_limit=30\n").unwrap();
        assert!(dir_link(&outside, &game.join("extra")));
        assert!(set_aside_early_files(&p, &game).is_err());
        assert!(ini.is_file() && outside.join(inside).is_file() && !p.pending_ini().exists());
        unlink_dir(&game.join("extra"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn unreadable_settings_are_left_untouched() {
        let root = temp("early-invalid");
        let (data, game) = (root.join("data"), root.join("game"));
        let p = paths_in(&data);
        let ini = paths::game_ini(&game);
        std::fs::create_dir_all(ini.parent().unwrap()).unwrap();
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(&ini, b"\xff").unwrap();
        std::fs::write(p.pending_ini(), "volume=100\n").unwrap();
        assert!(set_aside_early_files(&p, &game).is_err());
        assert_eq!(std::fs::read(&ini).unwrap(), b"\xff");
        assert_eq!(std::fs::read_to_string(p.pending_ini()).unwrap(), "volume=100\n");
        std::fs::write(&ini, "volume=150\n").unwrap();
        std::fs::write(p.pending_ini(), b"\xff").unwrap();
        assert!(set_aside_early_files(&p, &game).is_err());
        assert_eq!(std::fs::read_to_string(&ini).unwrap(), "volume=150\n");
        assert_eq!(std::fs::read(p.pending_ini()).unwrap(), b"\xff");
        let _ = std::fs::remove_dir_all(root);
    }
}
