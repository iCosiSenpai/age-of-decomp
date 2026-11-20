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
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JobObjectExtendedLimitInformation, SetInformationJobObject,
    };

    static JOB: OnceLock<Result<usize, String>> = OnceLock::new();
    let handle = *JOB.get_or_init(|| unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return Err(format!("Could not create the build job: {}", std::io::Error::last_os_error()));
        }
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
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

/// Settings written before the first build — RecompLauncher edits `hwaoi.ini` as soon as the player changes a setting
/// on the game's page, which creates the file in a game folder that does not exist yet. The engine only builds into an
/// empty folder, so when that file is the only thing there it moves into the pending settings (merged back by
/// `settings::merge_pending` after the build) and its now-empty folders go. Anything else in the folder is left alone,
/// and the engine refuses it as before.
pub fn stash_early_settings(paths: &Paths, output: &std::path::Path) -> Result<(), String> {
    if output.join("hwaoi-compiler.json").is_file() {
        return Ok(()); // an engine build folder: updates are the engine's business
    }
    let ini = paths::game_ini(output);
    // Links (symlinks, junctions) are never followed: the file read and then deleted must really be inside the folder.
    let Ok(inside) = ini.strip_prefix(output) else { return Ok(()) };
    let mut at = output.to_path_buf();
    for part in inside.components() {
        at.push(part);
        if std::fs::symlink_metadata(&at).is_ok_and(|m| m.file_type().is_symlink()) {
            return Ok(());
        }
    }
    let Some(early) = crate::settings::existing_ini(&ini)? else { return Ok(()) };
    let mut others = Vec::new();
    let mut stack = vec![output.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            let kind = e.file_type().map_err(|e| e.to_string())?;
            let p = e.path();
            if kind.is_dir() && !kind.is_symlink() {
                stack.push(p);
            } else if p != ini || kind.is_symlink() {
                others.push(p); // a link counts as something else in the folder and is left alone
            }
        }
    }
    if !others.is_empty() {
        return Ok(());
    }
    let mut pending = crate::settings::load_ini(&paths.pending_ini())?;
    for (k, v) in early.values() {
        if let Ok(s) = crate::settings::validate(&k, &v) {
            pending.set(&k, &v, &s.section); // the newer choice wins
        }
    }
    pending.save(&paths.pending_ini()).map_err(|e| e.to_string())?;
    std::fs::remove_file(&ini).map_err(|e| e.to_string())?;
    let mut dir = ini.parent().map(std::path::Path::to_path_buf);
    while let Some(d) = dir {
        if d == output || std::fs::remove_dir(&d).is_err() {
            break; // stops at the game folder itself, or at the first folder that is not empty
        }
        dir = d.parent().map(std::path::Path::to_path_buf);
    }
    Ok(())
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
    stash_early_settings(paths, &job.output)?;
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
        std::fs::write(&ini, "fps_limit=30\nunknown_key=1\n").unwrap();
        stash_early_settings(&p, &game).unwrap();
        assert!(!ini.exists() && !game.join("work").exists(), "the game folder is empty again");
        let pending = crate::ini::Ini::load(&p.pending_ini()).unwrap().values();
        assert_eq!(pending.get("fps_limit").map(String::as_str), Some("30"));
        assert!(!pending.contains_key("unknown_key"));
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
        stash_early_settings(&p, &game).unwrap();
        assert!(ini.exists() && game.join("notes.txt").exists());
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
        stash_early_settings(&p, &game).unwrap();
        assert!(outside.join(inside).is_file() && !p.pending_ini().exists());
        // A link anywhere else in the folder is something the player put there: also left alone.
        unlink_dir(&game.join("work"));
        std::fs::create_dir_all(ini.parent().unwrap()).unwrap();
        std::fs::write(&ini, "fps_limit=30\n").unwrap();
        assert!(dir_link(&outside, &game.join("extra")));
        stash_early_settings(&p, &game).unwrap();
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
        assert!(stash_early_settings(&p, &game).is_err());
        assert_eq!(std::fs::read(&ini).unwrap(), b"\xff");
        assert_eq!(std::fs::read_to_string(p.pending_ini()).unwrap(), "volume=100\n");
        std::fs::write(&ini, "volume=150\n").unwrap();
        std::fs::write(p.pending_ini(), b"\xff").unwrap();
        assert!(stash_early_settings(&p, &game).is_err());
        assert_eq!(std::fs::read_to_string(&ini).unwrap(), "volume=150\n");
        assert_eq!(std::fs::read(p.pending_ini()).unwrap(), b"\xff");
        let _ = std::fs::remove_dir_all(root);
    }
}
