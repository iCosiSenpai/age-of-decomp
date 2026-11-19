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
pub fn check(cfg: &Config, paths: &Paths) -> Result<(), LaunchError> {
    let output = cfg.output(paths);
    let state = config::engine_state(&output)
        .ok_or_else(|| LaunchError::NotBuilt("The game is not built yet: open Setup".into()))?;
    if state.get("status").and_then(|s| s.as_str()) != Some("complete") || !paths::game_exe(&output).is_file() {
        return Err(LaunchError::NotBuilt("The game build is incomplete: open Setup and continue the build".into()));
    }
    let nup = state.get("nup").and_then(|s| s.as_str())
        .ok_or_else(|| LaunchError::NotBuilt("The build does not record its game file".into()))?;
    let recorded = state.get("nup_identity").cloned().unwrap_or_default();
    let opened = aod_nup::Nup::open(nup).map_err(|e| LaunchError::GameFile(
        format!("Your game file cannot be read ({e}): {nup}. Put it back where it was, or set up again.")))?;
    if !same_identity(&recorded, &opened.identity()) {
        return Err(LaunchError::GameFile("Your game file changed since the build. Set up again with it.".into()));
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
}

pub fn launch(cfg: &Config, paths: &Paths, bigpicture: bool) -> Result<Launched, LaunchError> {
    check(cfg, paths)?;
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
        ("HWAOI_PIPE_PREBUILD", "1".into()),
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
    cmd.stdin(Stdio::null()).stdout(Stdio::from(log)).stderr(Stdio::from(log2));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console; the game opens its own window
    }
    let child = cmd.spawn().map_err(|e| LaunchError::Other(format!("Could not start the game: {e}")))?;
    Ok(Launched { child, log: log_path })
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
