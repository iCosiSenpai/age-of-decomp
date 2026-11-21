//! Where things live. Release layout (Windows zip / Linux AppImage AppDir):
//!   AgeOfDecomp(.exe)  aod-cli(.exe)  engine/ (staged engine bundle)  recomplauncher.json  licenses/
//! Launcher data: AOD_DATA_DIR, else RecompLauncher's `{data}` folder (RECOMPLAUNCHER_DATA, which it sets for the game
//! and the setup step, so all we write stays in the game's folder), else `data/` next to the program when
//! `portable.txt` is there, else %LOCALAPPDATA%\AgeOfDecomp (Windows) / $XDG_DATA_HOME/AgeOfDecomp (Linux).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Paths {
    pub exe_dir: PathBuf,
    pub data: PathBuf,
    pub engine: Option<PathBuf>,
    pub portable: bool,
}

fn engine_ok(dir: &Path) -> bool {
    dir.join("bundle.json").is_file() || dir.join("source/tools/hwaoi.py").is_file()
}

impl Paths {
    pub fn detect() -> Paths {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        let exe_dir = exe.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
        let portable = exe_dir.join("portable.txt").is_file();
        let data = data_dir(std::env::var_os("AOD_DATA_DIR"), std::env::var_os("RECOMPLAUNCHER_DATA"), portable, &exe_dir);
        Paths { engine: find_engine(&exe_dir), exe_dir, data, portable }
    }

    pub fn config_file(&self) -> PathBuf {
        self.data.join("launcher.json")
    }
    pub fn default_game_dir(&self) -> PathBuf {
        self.data.join("game")
    }
    pub fn logs(&self) -> PathBuf {
        self.data.join("logs")
    }
    pub fn pending_ini(&self) -> PathBuf {
        self.data.join("hwaoi.pending.ini")
    }
}

/// The launcher's data folder from its sources, in order (an empty variable counts as unset).
fn data_dir(aod: Option<OsString>, recomplauncher: Option<OsString>, portable: bool, exe_dir: &Path) -> PathBuf {
    if let Some(d) = aod.filter(|d| !d.is_empty()) {
        PathBuf::from(d)
    } else if let Some(d) = recomplauncher.filter(|d| !d.is_empty()) {
        PathBuf::from(d)
    } else if portable {
        exe_dir.join("data")
    } else {
        platform_data_dir().join("AgeOfDecomp")
    }
}

/// The standalone launcher's data folder (its settings, and the game folder it builds into by default), wherever
/// this run keeps its own.
pub fn standalone_data_dir() -> PathBuf {
    platform_data_dir().join("AgeOfDecomp")
}

fn platform_data_dir() -> PathBuf {
    if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
    } else if let Some(x) = std::env::var_os("XDG_DATA_HOME") {
        PathBuf::from(x)
    } else {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")).unwrap_or_else(|| PathBuf::from("."))
    }
}

/// The engine bundle: AOD_ENGINE_DIR, `engine/` beside the program, the AppImage layout, or (development) the
/// repository's `out/engine/<os>` found by walking up from the program.
fn find_engine(exe_dir: &Path) -> Option<PathBuf> {
    find_engine_from(exe_dir, std::env::var_os("AOD_ENGINE_DIR").map(PathBuf::from))
}

fn find_engine_from(exe_dir: &Path, configured: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(d) = configured
        && engine_ok(&d)
    {
        return Some(d);
    }
    let os = if cfg!(windows) { "windows" } else { "linux" };
    let mut candidates = vec![exe_dir.join("engine"), exe_dir.join("../lib/age-of-decomp/engine")];
    for anc in exe_dir.ancestors().take(8) {
        if anc.join("engine/source/VERSION").is_file() && anc.join("apps/launcher").is_dir() {
            candidates.push(anc.join("out").join("engine").join(os));
        }
    }
    candidates.into_iter().find(|c| engine_ok(c)).map(|c| plain(c.canonicalize().unwrap_or(c)))
}

/// A canonical path without the Windows `\\?\` prefix (Python and child programs get ordinary paths).
pub fn plain(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy().into_owned();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => p,
    }
}

/// The built game's folder inside a game output directory (engine layout).
pub fn game_bin_dir(output: &Path) -> PathBuf {
    output.join("work").join("build").join("hwaoi196608")
}

pub fn game_exe(output: &Path) -> PathBuf {
    game_bin_dir(output).join(if cfg!(windows) { "hwaoi.exe" } else { "hwaoi" })
}

pub fn game_ini(output: &Path) -> PathBuf {
    game_bin_dir(output).join("hwaoi.ini")
}

pub fn saves_dir(output: &Path) -> PathBuf {
    output.join("work").join("saves").join("hwaoi196608")
}

pub fn game_logs_dir(output: &Path) -> PathBuf {
    output.join("logs")
}

/// A local timestamp-free, sortable stamp for file names (seconds since the Unix epoch).
pub fn stamp() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    secs.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_engine_requires_repository_markers() {
        let root = std::env::temp_dir().join(format!(
            "aod-engine-search-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let exe = root.join("apps/launcher/src-tauri/target/release");
        let engine = root.join("out/engine").join(if cfg!(windows) { "windows" } else { "linux" });
        std::fs::create_dir_all(&exe).unwrap();
        std::fs::create_dir_all(&engine).unwrap();
        std::fs::write(engine.join("bundle.json"), "{}").unwrap();
        assert!(find_engine_from(&exe, None).is_none());
        std::fs::create_dir_all(root.join("engine/source")).unwrap();
        std::fs::write(root.join("engine/source/VERSION"), "0\n").unwrap();
        assert_eq!(find_engine_from(&exe, None), Some(plain(engine.canonicalize().unwrap())));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn data_folder_follows_recomplauncher_after_our_own_override() {
        let exe = Path::new("release");
        let os = |s: &str| Some(OsString::from(s));
        assert_eq!(data_dir(os("mine"), os("recompdata"), true, exe), PathBuf::from("mine"));
        assert_eq!(data_dir(None, os("recompdata"), true, exe), PathBuf::from("recompdata"));
        assert_eq!(data_dir(os(""), os("recompdata"), false, exe), PathBuf::from("recompdata"));
        assert_eq!(data_dir(None, os(""), true, exe), exe.join("data"));
        assert_eq!(data_dir(None, None, false, exe), platform_data_dir().join("AgeOfDecomp"));
    }
}
