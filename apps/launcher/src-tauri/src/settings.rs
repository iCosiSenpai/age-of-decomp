//! Game settings backed by the shared catalogue (src/lib/settings/catalog.json): validation, and reads/writes of
//! `hwaoi.ini`. Before the game is built, changes go to a pending file in the launcher data folder and are merged into
//! `hwaoi.ini` once the build has written it.

use crate::config::Config;
use crate::ini::Ini;
use crate::paths::{self, Paths};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;

const CATALOG: &str = include_str!("../../src/lib/settings/catalog.json");

#[derive(Clone, Debug, Deserialize)]
pub struct Choice {
    pub value: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Setting {
    pub key: String,
    pub section: String,
    pub kind: String,
    pub default: String,
    #[serde(default)]
    pub choices: Vec<Choice>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub source: Option<String>,
}

#[derive(Deserialize)]
struct Catalog {
    settings: Vec<Setting>,
}

pub fn catalog() -> &'static [Setting] {
    static C: OnceLock<Vec<Setting>> = OnceLock::new();
    C.get_or_init(|| serde_json::from_str::<Catalog>(CATALOG).expect("catalog.json is valid").settings)
}

pub fn validate(key: &str, value: &str) -> Result<&'static Setting, String> {
    let s = catalog().iter().find(|s| s.key == key).ok_or_else(|| format!("unknown setting {key}"))?;
    let ok = match s.kind.as_str() {
        "toggle" => value == "0" || value == "1",
        "choice" => s.choices.iter().any(|c| c.value == value),
        "slider" => value
            .parse::<f64>()
            .is_ok_and(|v| v >= s.min.unwrap_or(f64::MIN) - 1e-9 && v <= s.max.unwrap_or(f64::MAX) + 1e-9),
        "dynamic" => match s.source.as_deref() {
            Some("gpus") => value.is_empty() || value.parse::<u32>().is_ok(),
            Some("pads") => {
                matches!(value, "auto" | "keyboard" | "none")
                    || value.strip_prefix("pad:").is_some_and(|r| r.split(':').count() == 2)
            }
            _ => false,
        },
        _ => false,
    };
    if ok { Ok(s) } else { Err(format!("{value} is not a valid value for {key}")) }
}

/// The file settings are read from and written to now.
pub fn ini_path(cfg: &Config, paths: &Paths) -> PathBuf {
    let dir = paths::game_bin_dir(&cfg.output(paths));
    if dir.is_dir() { dir.join("hwaoi.ini") } else { paths.pending_ini() }
}

pub fn read(cfg: &Config, paths: &Paths) -> BTreeMap<String, String> {
    let mut v = Ini::load(&paths.pending_ini()).values();
    v.extend(Ini::load(&ini_path(cfg, paths)).values());
    v
}

pub fn write(cfg: &Config, paths: &Paths, key: &str, value: &str) -> Result<(), String> {
    let s = validate(key, value)?;
    let path = ini_path(cfg, paths);
    let mut ini = Ini::load(&path);
    ini.set(key, value, &s.section);
    ini.save(&path).map_err(|e| e.to_string())
}

pub fn reset(cfg: &Config, paths: &Paths, keys: &[String]) -> Result<(), String> {
    for path in [ini_path(cfg, paths), paths.pending_ini()] {
        if !path.is_file() {
            continue;
        }
        let mut ini = Ini::load(&path);
        for k in keys {
            if catalog().iter().any(|s| &s.key == k) {
                ini.remove(k);
            }
        }
        ini.save(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// After a build: settings chosen before the game existed go into its hwaoi.ini.
pub fn merge_pending(cfg: &Config, paths: &Paths) -> Result<(), String> {
    let pending = paths.pending_ini();
    if !pending.is_file() {
        return Ok(());
    }
    let target = paths::game_ini(&cfg.output(paths));
    if !target.parent().is_some_and(|p| p.is_dir()) {
        return Ok(());
    }
    let mut ini = Ini::load(&target);
    for (k, v) in Ini::load(&pending).values() {
        if let Ok(s) = validate(&k, &v) {
            ini.set(&k, &v, &s.section);
        }
    }
    ini.save(&target).map_err(|e| e.to_string())?;
    std::fs::remove_file(pending).map_err(|e| e.to_string())
}
