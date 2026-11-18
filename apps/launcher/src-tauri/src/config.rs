//! The launcher's own settings (launcher.json in the data folder) and the game's build state.

use crate::paths::{self, Paths};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct UiPrefs {
    pub language: String,
    pub close_on_play: bool,
    pub check_updates: bool,
    pub jobs: u32,
    pub rate_unlock: bool,
}

impl Default for UiPrefs {
    fn default() -> Self {
        UiPrefs { language: "auto".into(), close_on_play: false, check_updates: true, jobs: 0, rate_unlock: false }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub nup: Option<String>,
    pub update_nup: Option<String>,
    pub dlc_nup: Option<String>,
    pub output: Option<String>,
    pub ui: UiPrefs,
}

impl Config {
    pub fn load(paths: &Paths) -> Config {
        let file = paths.config_file();
        let error = match std::fs::read_to_string(&file) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(config) => return config,
                Err(e) => e.to_string(),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Config::default(),
            Err(e) => e.to_string(),
        };
        let mut bad = file.with_file_name(format!("launcher.json.bad-{}", paths::stamp()));
        let mut n = 0;
        while bad.exists() {
            n += 1;
            bad = file.with_file_name(format!("launcher.json.bad-{}-{n}", paths::stamp()));
        }
        match std::fs::rename(&file, &bad) {
            Ok(()) => eprintln!("{} could not be read ({error}); kept as {}", file.display(), bad.display()),
            Err(e) => eprintln!("{} could not be read ({error}); could not preserve it ({e})", file.display()),
        }
        Config::default()
    }

    pub fn save(&self, paths: &Paths) -> Result<(), String> {
        std::fs::create_dir_all(&paths.data).map_err(|e| e.to_string())?;
        let file = paths.config_file();
        // A failed backup in load must never turn a later settings change into data loss.
        match std::fs::read_to_string(&file) {
            Ok(text) if serde_json::from_str::<Config>(&text).is_err() => {
                return Err(format!("{} is invalid; preserve or repair it before saving", file.display()));
            }
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
            _ => {}
        }
        let tmp = file.with_extension("json.part");
        std::fs::write(&tmp, serde_json::to_string_pretty(self).unwrap()).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &file).map_err(|e| e.to_string())
    }

    pub fn output(&self, paths: &Paths) -> PathBuf {
        self.output.as_ref().map(PathBuf::from).unwrap_or_else(|| paths.default_game_dir())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct GameState {
    pub nup: Option<String>,
    pub update_nup: Option<String>,
    pub dlc_nup: Option<String>,
    pub output: String,
    pub status: &'static str,
    pub built_at: Option<String>,
    pub nup_present: bool,
}

/// The engine's own state file in the output folder (compiler/backend.py `STATE`).
pub fn engine_state(output: &Path) -> Option<serde_json::Value> {
    let text = std::fs::read_to_string(output.join("hwaoi-compiler.json")).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn game_state(cfg: &Config, paths: &Paths) -> GameState {
    let output = cfg.output(paths);
    let st = engine_state(&output);
    let exe = paths::game_exe(&output);
    let status = match st.as_ref().and_then(|s| s.get("status")).and_then(|s| s.as_str()) {
        Some("complete") if exe.is_file() => "complete",
        Some(_) => "incomplete",
        None => "none",
    };
    let built_at = std::fs::metadata(&exe)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs().to_string());
    GameState {
        nup_present: cfg.nup.as_ref().is_some_and(|n| Path::new(n).is_file()),
        nup: cfg.nup.clone(),
        update_nup: cfg.update_nup.clone(),
        dlc_nup: cfg.dlc_nup.clone(),
        output: output.to_string_lossy().into_owned(),
        status,
        built_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_config_is_preserved_before_using_defaults() {
        let root = std::env::temp_dir().join(format!(
            "aod-config-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let paths = Paths { exe_dir: root.clone(), data: root.clone(), engine: None, portable: false };
        std::fs::write(paths.config_file(), b"{invalid").unwrap();
        let cfg = Config::load(&paths);
        assert!(cfg.nup.is_none() && cfg.output.is_none());
        assert!(!paths.config_file().exists());
        let bad: Vec<_> = std::fs::read_dir(&root).unwrap().map(|e| e.unwrap().path()).collect();
        assert_eq!(bad.len(), 1);
        assert!(bad[0].file_name().unwrap().to_string_lossy().starts_with("launcher.json.bad-"));
        assert_eq!(std::fs::read(&bad[0]).unwrap(), b"{invalid");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn save_refuses_to_overwrite_an_unpreserved_bad_config() {
        let root = std::env::temp_dir().join(format!("aod-config-save-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let paths = Paths { exe_dir: root.clone(), data: root.clone(), engine: None, portable: false };
        std::fs::write(paths.config_file(), b"{invalid").unwrap();
        assert!(Config::default().save(&paths).is_err());
        assert_eq!(std::fs::read(paths.config_file()).unwrap(), b"{invalid");
        std::fs::remove_dir_all(root).unwrap();
    }
}
