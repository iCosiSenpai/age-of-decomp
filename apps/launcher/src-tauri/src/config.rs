//! The launcher's own settings (launcher.json in the data folder) and the game's build state.

use crate::paths::{self, Paths};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct UiPrefs {
    pub language: String,
    pub close_on_play: bool,
    pub jobs: u32,
    pub rate_unlock: bool,
}

impl Default for UiPrefs {
    fn default() -> Self {
        UiPrefs { language: "auto".into(), close_on_play: false, jobs: 0, rate_unlock: false }
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
        std::fs::read_to_string(paths.config_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, paths: &Paths) -> Result<(), String> {
        std::fs::create_dir_all(&paths.data).map_err(|e| e.to_string())?;
        let file = paths.config_file();
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
