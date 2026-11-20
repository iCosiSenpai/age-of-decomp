//! Starting the built game, as the engine's own `tools/hwaoi.py launch` does (same environment and `--play-experimental`
//! argument), with two differences: rumble is on (upstream forces HWAOI_RUMBLE=0) and the log goes to `<game>/logs/`,
//! where RecompLauncher looks for it. RecompLauncher's `RECOMPLAUNCHER_MODE=bigpicture` starts it fullscreen.

use crate::config::{self, Config};
use crate::paths::{self, Paths};
use std::path::Path;
use std::process::{Child, Command, Stdio};

#[derive(Debug)]
pub enum LaunchError {
    NotBuilt(String),
    GameFile(String),
    Other(String),
}

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotBuilt(s) | Self::GameFile(s) | Self::Other(s) => f.write_str(s),
        }
    }
}

impl LaunchError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::NotBuilt(_) => 64,
            Self::GameFile(_) => 65,
            Self::Other(_) => 1,
        }
    }
}

fn same_identity(recorded: &serde_json::Value, id: &aod_nup::Identity) -> bool {
    recorded.get("header").and_then(|v| v.as_str()) == Some(id.header.as_str())
        && recorded.get("toc").and_then(|v| v.as_str()) == Some(id.toc.as_str())
        && recorded.get("size").and_then(|v| v.as_u64()) == Some(id.size)
}

/// Checks that the build is complete and that the player's NUP is still the one it was built from (the game reads
/// its raw resources from it). Identity = SHA-256 of the NUP header and TOC + file size, as the engine records it.
/// `copy`: where the player's NUP is now (RecompLauncher's `{input:nup}`), instead of where it was at the build; the same
/// identity is required, and the game then reads it there (HWAOI_NUP_PATH).
pub fn check(cfg: &Config, paths: &Paths, copy: Option<&Path>) -> Result<(), LaunchError> {
    let output = cfg.output(paths);
    let state = config::engine_state(&output)
        .ok_or_else(|| LaunchError::NotBuilt("The game is not built yet: open Setup".into()))?;
    if state.get("status").and_then(|s| s.as_str()) != Some("complete") || !paths::game_exe(&output).is_file() {
        return Err(LaunchError::NotBuilt("The game build is incomplete: open Setup and continue the build".into()));
    }
    if config::rebuild_required(&state) {
        return Err(LaunchError::NotBuilt("The engine was updated. Open Setup and rebuild the game before playing.".into()));
    }
    let recorded_nup = state.get("nup").and_then(|s| s.as_str())
        .ok_or_else(|| LaunchError::NotBuilt("The build does not record its game file".into()))?;
    let nup = copy.map(|c| c.to_string_lossy().into_owned()).unwrap_or_else(|| recorded_nup.to_string());
    let recorded = state.get("nup_identity").cloned().unwrap_or_default();
    let opened = aod_nup::Nup::open(&nup).map_err(|e| LaunchError::GameFile(if copy.is_some() {
        format!("Your game file cannot be read ({e}): {nup}.")
    } else {
        format!("Your game file cannot be read ({e}): {nup}. Put it back where it was, or set up again.")
    }))?;
    if !same_identity(&recorded, &opened.identity()) {
        return Err(LaunchError::GameFile(if copy.is_some() {
            "This .nup is not the one the game was built from. Pick that one, or set up again with this one.".into()
        } else {
            "Your game file changed since the build. Set up again with it.".into()
        }));
    }
    if let Some(update) = state.get("update_nup").and_then(|v| v.as_str()) {
        let opened = aod_nup::Nup::open(update).map_err(|e| LaunchError::GameFile(
            format!("Your update game file cannot be read ({e}): {update}. Put it back where it was, or set up again.")))?;
        if !same_identity(&recorded["update"], &opened.identity()) {
            return Err(LaunchError::GameFile("Your update game file changed since the build. Set up again with it.".into()));
        }
    }
    Ok(())
}

pub struct Launched {
    pub child: Child,
    pub log: std::path::PathBuf,
    pub started: std::time::Instant,
}

/// The game's exit code for the in-game menu's Restart (a setting read only at start was changed): the caller starts
/// the game again. The game shows Restart only when `launch` was told the caller does (`AOD_RESTART_EXIT`).
pub const RESTART_EXIT: i32 = 75;

impl Launched {
    /// Exited with [`RESTART_EXIT`] after running a few seconds: start it again. A game that ends that way at once is
    /// not started again, so nothing can loop.
    pub fn wants_restart(&self, code: Option<i32>) -> bool {
        code == Some(RESTART_EXIT) && self.started.elapsed() >= std::time::Duration::from_secs(3)
    }
}

pub fn launch(cfg: &Config, paths: &Paths, bigpicture: bool, handoff: Option<&crate::nintendo::Handoff>,
              restart: bool, copy: Option<&Path>, dlss: Option<&Path>) -> Result<Launched, LaunchError> {
    let copy = copy.map(std::path::absolute).transpose().map_err(|e| LaunchError::Other(e.to_string()))?;
    check(cfg, paths, copy.as_deref())?;
    // The child runs from its bin directory. Resolve CLI-relative paths before changing
    // its working directory, including resource and save paths passed in the environment.
    let output = std::path::absolute(cfg.output(paths)).map_err(|e| LaunchError::Other(e.to_string()))?;
    let exe = paths::game_exe(&output);
    let bin = paths::game_bin_dir(&output);
    let translator = bin.join("xlate-managed-irbanks").join(if cfg!(windows) { "totk_xlate.exe" } else { "totk_xlate" });
    if !translator.is_file() {
        return Err(LaunchError::NotBuilt("The shader translator is missing: open Setup and continue the build".into()));
    }
    let logs = paths::game_logs_dir(&output);
    std::fs::create_dir_all(&logs).map_err(|e| LaunchError::Other(e.to_string()))?;
    let saves = paths::saves_dir(&output);
    std::fs::create_dir_all(&saves).map_err(|e| LaunchError::Other(e.to_string()))?;
    crate::settings::retire_values(&paths::game_ini(&output)).map_err(LaunchError::Other)?;
    let log_path = logs.join(format!("game-{}.log", paths::stamp()));
    let log = std::fs::File::create(&log_path).map_err(|e| LaunchError::Other(e.to_string()))?;
    let log2 = log.try_clone().map_err(|e| LaunchError::Other(e.to_string()))?;
    let state = config::engine_state(&output).unwrap_or_default();
    let unlock = state.get("rate_unlock").and_then(|v| v.as_bool()).unwrap_or(false);

    let mut cmd = Command::new(&exe);
    cmd.arg("--play-experimental").current_dir(&bin);
    // Values that would lock a row of the in-game menu are left to hwaoi.ini, except fullscreen in Big Picture mode.
    for setting in ["INPUT_SCRIPT", "INPUT_RECORD", "INPUT_FEED", "XLATE_ID", "PIPE_SYNC", "PIPE_SKIP", "PIPE_PREBUILD"] {
        for prefix in ["HWAOI_", "DKB_", "TOTK_"] {
            cmd.env_remove(format!("{prefix}{setting}"));
        }
    }
    let p = |x: &Path| x.to_string_lossy().into_owned();
    let mut env: Vec<(&str, String)> = vec![
        ("HWAOI_GPU", "vulkan".into()),
        ("HWAOI_INPUT", "auto".into()),
        ("HWAOI_RUMBLE", "1".into()),
        ("HWAOI_PROGRESS", "1".into()),
        ("HWAOI_EXEFS", p(&output.join("work").join("game").join("exefs"))),
        ("HWAOI_PROFILE_DIR", p(&saves)),
        ("HWAOI_GSV_LOCAL_FRAME_POLICY", "1".into()),
        ("HWAOI_NATIVE_INFLATE", "1".into()),
        ("HWAOI_POOL_BACKPRESSURE", "1".into()),
        ("HWAOI_POOL_DIAG", "0".into()),
        ("HWAOI_IR_BANKS", "1".into()),
        ("HWAOI_XLATE", p(&translator)),
        ("HWAOI_TRACE", "perf".into()),
        ("HWAOI_CORE_STALL_DIAG", "0".into()),
        ("DKB_SHADER_FAILURE_DUMP", p(&output.join("work").join("reports").join("shader-failures"))),
        ("HWAOI_PIPE_SYNC", "0".into()),
        ("HWAOI_PIPE_SKIP", "strict".into()),
        // PIPE_PREBUILD is not set here: the runtime's default is on, and "Prepare shaders early" in hwaoi.ini decides.
        ("HWAOI_RATE_UNLOCK", if unlock { "1" } else { "0" }.into()),
    ];
    if cfg!(windows) {
        env.push(("HWAOI_ZLIB_DLL", p(&bin.join("zlib1.dll"))));
    } else {
        // The native inflate loads the system zlib by absolute path; without one the game decompresses as on Switch.
        match system_zlib() {
            Some(z) => env.push(("HWAOI_ZLIB_DLL", p(&z))),
            None => env.push(("HWAOI_NATIVE_INFLATE", "0".into())),
        }
        // As upstream's tools/linux/play.sh: XWayland, since native Wayland on NVIDIA ended the game when the window
        // size changed. A value the player set wins.
        if std::env::var_os("SDL_VIDEODRIVER").is_none() && std::env::var_os("DISPLAY").is_some() {
            env.push(("SDL_VIDEODRIVER", "x11".into()));
        }
    }
    if bigpicture {
        env.push(("HWAOI_FULLSCREEN", "1".into()));
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    if let Some(engine) = &paths.engine {
        let helper = engine.join("nintendo/AgeOfDecomp.Nintendo.exe");
        if cfg!(windows) && helper.is_file() { cmd.env("AOD_NINTENDO_BRIDGE", helper); }
        // DLAA (temporal = dlaa): NVIDIA Streamline's signed DLLs ship in the engine bundle.
        let streamline = engine.join("streamline");
        if cfg!(windows) && streamline.join("sl.interposer.dll").is_file() { cmd.env("HWAOI_DLSS_DIR", streamline); }
    }
    // The player's own nvngx_dlss.dll (never shipped by us): RecompLauncher's nvidia.dlss input, else <data>/ngx.
    let own = paths.data.join("ngx").join("nvngx_dlss.dll");
    if let Some(ngx) = dlss.filter(|d| d.is_file()).or_else(|| own.is_file().then_some(own.as_path())) {
        cmd.env("HWAOI_DLSS_NGX", ngx);
    }
    cmd.env_remove("AOD_NINTENDO_HANDOFF");
    if let Some(handoff) = handoff { handoff.pass_to(&mut cmd); }
    if restart { cmd.env("AOD_RESTART_EXIT", RESTART_EXIT.to_string()); } else { cmd.env_remove("AOD_RESTART_EXIT"); }
    // The copy where the player keeps it now (checked above to be the one of the build); else where it was.
    match &copy {
        Some(c) => { cmd.env("HWAOI_NUP_PATH", c); }
        None => { cmd.env_remove("HWAOI_NUP_PATH"); }
    }
    cmd.stdin(Stdio::null()).stdout(Stdio::from(log)).stderr(Stdio::from(log2));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console; the game opens its own window
    }
    let child = cmd.spawn().map_err(|e| LaunchError::Other(format!("Could not start the game: {e}")))?;
    #[cfg(windows)]
    hand_foreground(child.id());
    Ok(Launched { child, log: log_path, started: std::time::Instant::now() })
}

/// The game opens its window behind the others without taking focus, and reads no controller until that window is in
/// the foreground (runtime gfx_window_win.c, input.c `pad_focus_gate`): a player who pressed Play with a pad saw the
/// game ignore it until they clicked the window (player reports RL-10, RL-7). This process may set the foreground at
/// Play (the player just used our window, or RecompLauncher started us from its own), so it lets the game take it
/// (the runtime's own call in Big Picture) and brings the game's window up as soon as it shows.
#[cfg(windows)]
fn hand_foreground(pid: u32) {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AllowSetForegroundWindow, BringWindowToTop, EnumWindows, GW_OWNER, GetForegroundWindow, GetWindow,
        GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow,
    };
    struct Found {
        pid: u32,
        window: HWND,
    }
    // The game's main window: visible, top-level, of the game's process, with no owner (the software keyboard has one).
    unsafe extern "system" fn each(window: HWND, found: LPARAM) -> windows_sys::core::BOOL {
        let found = unsafe { &mut *(found as *mut Found) };
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(window, &mut pid) };
        if pid == found.pid && unsafe { IsWindowVisible(window) } != 0 && unsafe { GetWindow(window, GW_OWNER) }.is_null() {
            found.window = window;
            return 0;
        }
        1
    }
    unsafe { AllowSetForegroundWindow(pid) };
    // The window shows after the first pipelines are built, which takes longer on a first, cold start. Windows itself
    // refuses the foreground once the player has turned to another program meanwhile: the taskbar button flashes then.
    std::thread::spawn(move || {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(120);
        while std::time::Instant::now() < until {
            let mut found = Found { pid, window: std::ptr::null_mut() };
            unsafe { EnumWindows(Some(each), &mut found as *mut Found as LPARAM) };
            if !found.window.is_null() {
                unsafe {
                    if GetForegroundWindow() != found.window {
                        BringWindowToTop(found.window);
                        SetForegroundWindow(found.window);
                    }
                }
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });
}

/// Linux: the 64-bit system zlib (`libz.so.1`), from the usual library folders (multiarch first, so a 32-bit copy in
/// /usr/lib on Fedora is never picked), else from the loader cache.
fn system_zlib() -> Option<std::path::PathBuf> {
    for dir in ["/usr/lib/x86_64-linux-gnu", "/lib/x86_64-linux-gnu", "/usr/lib64", "/lib64", "/usr/lib", "/lib"] {
        let lib = Path::new(dir).join("libz.so.1");
        if lib.is_file() {
            return Some(lib);
        }
    }
    let out = Command::new("ldconfig").arg("-p").output().or_else(|_| Command::new("/sbin/ldconfig").arg("-p").output()).ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().find(|l| l.trim_start().starts_with("libz.so.1 ") && l.contains("x86-64"))?;
    let lib = std::path::PathBuf::from(line.rsplit("=> ").next()?.trim());
    lib.is_file().then_some(lib)
}

/// The mean FPS the runtime reports when it exits ("frametime: exit: … mean 59.9 fps"), if any.
pub fn session_fps(log: &Path) -> Option<f64> {
    let text = std::fs::read_to_string(log).ok()?;
    let line = text.lines().rev().find(|l| l.contains("frametime: exit:"))?;
    let at = line.find("mean ")? + 5;
    line[at..].split_whitespace().next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restart_only_on_its_code_after_a_few_seconds() {
        let mut cmd = if cfg!(windows) { Command::new("cmd") } else { Command::new("true") };
        if cfg!(windows) { cmd.args(["/C", "exit 0"]); }
        let mut child = cmd.spawn().unwrap();
        let _ = child.wait();
        let ago = |s| std::time::Instant::now().checked_sub(std::time::Duration::from_secs(s)).unwrap();
        let mut l = Launched { child, log: "game.log".into(), started: ago(10) };
        assert!(l.wants_restart(Some(RESTART_EXIT)));
        assert!(!l.wants_restart(Some(0)));
        assert!(!l.wants_restart(Some(64)));
        assert!(!l.wants_restart(None));
        l.started = ago(1);
        assert!(!l.wants_restart(Some(RESTART_EXIT)), "a game that asks again at once is not started again");
    }

    #[test]
    fn launch_error_exit_codes_do_not_depend_on_message() {
        assert_eq!(LaunchError::NotBuilt("anything".into()).exit_code(), 64);
        assert_eq!(LaunchError::GameFile("anything".into()).exit_code(), 65);
        assert_eq!(LaunchError::Other("game file incomplete".into()).exit_code(), 1);
    }

    #[test]
    fn identity_requires_header_toc_and_size() {
        let id = aod_nup::Identity { header: "a".into(), toc: "b".into(), size: 42 };
        assert!(same_identity(&serde_json::json!({"header": "a", "toc": "b", "size": 42}), &id));
        assert!(!same_identity(&serde_json::json!({"header": "a", "toc": "c", "size": 42}), &id));
        assert!(!same_identity(&serde_json::Value::Null, &id));
    }
}
