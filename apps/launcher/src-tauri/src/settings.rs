//! Game settings backed by the shared catalogue (src/lib/settings/catalog.json): validation, and reads/writes of
//! `hwaoi.ini`. Before the game is built, changes go to a pending file in the launcher data folder and are merged into
//! `hwaoi.ini` once the build has written it.

use crate::config::Config;
use crate::ini::Ini;
use crate::paths::{self, Paths};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
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

/// A value that stays one value in the file: no control character and nothing the file's own syntax uses.
fn plain_value(value: &str) -> bool {
    !value.chars().any(|c| c < '\u{20}' || c == '\u{7f}' || matches!(c, '=' | '#' | ';' | '[' | ']' | '"'))
}

pub fn validate(key: &str, value: &str) -> Result<&'static Setting, String> {
    let s = catalog().iter().find(|s| s.key == key).ok_or_else(|| format!("unknown setting {key}"))?;
    if !plain_value(value) {
        return Err(format!("{value} is not a valid value for {key}"));
    }
    let ok = match s.kind.as_str() {
        "toggle" => value == "0" || value == "1",
        "choice" => s.choices.iter().any(|c| c.value == value),
        "slider" => plain_decimal(value)
            && value.parse::<f64>().is_ok_and(|v| {
                v.is_finite() && v >= s.min.unwrap_or(f64::MIN) && v <= s.max.unwrap_or(f64::MAX)
            }),
        "dynamic" => match s.source.as_deref() {
            Some("gpus") => value.is_empty() || value.bytes().all(|b| b.is_ascii_digit()),
            Some("pads") => {
                matches!(value, "auto" | "keyboard" | "none")
                    || value.strip_prefix("pad:").and_then(|r| r.split_once(':')).is_some_and(|(guid, index)| {
                        (1..=64).contains(&guid.len()) && guid.bytes().all(|b| b.is_ascii_hexdigit())
                            && (1..=3).contains(&index.len()) && index.bytes().all(|b| b.is_ascii_digit())
                    })
            }
            _ => false,
        },
        _ => false,
    };
    if ok { Ok(s) } else { Err(format!("{value} is not a valid value for {key}")) }
}

fn plain_decimal(value: &str) -> bool {
    let value = value.strip_prefix('-').unwrap_or(value);
    let mut parts = value.split('.');
    let whole = parts.next().unwrap_or("");
    !whole.is_empty() && whole.bytes().all(|b| b.is_ascii_digit())
        && parts.next().is_none_or(|fraction| !fraction.is_empty() && fraction.bytes().all(|b| b.is_ascii_digit()))
        && parts.next().is_none()
}

fn read_error(path: &Path, error: impl std::fmt::Display) -> String {
    format!("{} could not be read ({error}): nothing was changed", path.display())
}

pub(crate) fn load_ini(path: &Path) -> Result<Ini, String> {
    Ini::load(path).map_err(|e| read_error(path, e))
}

pub(crate) fn existing_ini(path: &Path) -> Result<Option<Ini>, String> {
    match std::fs::metadata(path) {
        Ok(_) => load_ini(path).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(read_error(path, e)),
    }
}

/// The file settings are read from and written to now.
pub fn ini_path(cfg: &Config, paths: &Paths) -> PathBuf {
    let dir = paths::game_bin_dir(&cfg.output(paths));
    if dir.is_dir() { dir.join("hwaoi.ini") } else { paths.pending_ini() }
}

pub fn read(cfg: &Config, paths: &Paths) -> BTreeMap<String, String> {
    let mut v = Ini::load(&paths.pending_ini()).map(|i| i.values()).unwrap_or_default();
    v.extend(Ini::load(&ini_path(cfg, paths)).map(|i| i.values()).unwrap_or_default());
    for (key, old, new) in RETIRED {
        if let Some(value) = v.get_mut(*key).filter(|value| old.contains(&value.as_str())) {
            *value = (*new).to_string();
        }
    }
    v
}

pub fn write(cfg: &Config, paths: &Paths, key: &str, value: &str) -> Result<(), String> {
    let s = validate(key, value)?;
    let path = ini_path(cfg, paths);
    let mut ini = load_ini(&path)?;
    ini.set(key, value, &s.section);
    ini.save(&path).map_err(|e| e.to_string())
}

pub fn reset(cfg: &Config, paths: &Paths, keys: &[String]) -> Result<(), String> {
    let mut files = Vec::new();
    for path in [ini_path(cfg, paths), paths.pending_ini()] {
        if files.iter().any(|(p, _): &(PathBuf, Ini)| p == &path) {
            continue;
        }
        let Some(mut ini) = existing_ini(&path)? else { continue };
        for k in keys {
            if catalog().iter().any(|s| &s.key == k) {
                ini.remove(k);
            }
        }
        files.push((path, ini));
    }
    for (path, ini) in files {
        ini.save(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Values earlier versions offered and this one no longer does, with the value that replaces them.
/// `temporal = dlaa` (0.1.2) is NVIDIA DLSS at its default quality, DLAA: the same picture under the newer name.
const RETIRED: &[(&str, &[&str], &str)] = &[
    ("fps_limit", &["90", "120"], "60"),
    ("temporal", &["dlaa"], "dlss"),
    // "Xbox prompts": this game's build has no overlay with Xbox letters, so the value only ever gave the
    // Nintendo button positions.
    ("pad_layout", &["xbox_prompts"], "nintendo"),
];

/// Before the game starts: rewrite retired values (from an older launcher or RecompLauncher) to their replacement.
pub fn retire_values(path: &Path) -> Result<(), String> {
    let Some(mut ini) = existing_ini(path)? else { return Ok(()) };
    let values = ini.values();
    let mut changed = false;
    for (key, old, new) in RETIRED {
        if values.get(*key).is_some_and(|v| old.contains(&v.as_str())) {
            let s = validate(key, new)?;
            ini.set(key, new, &s.section);
            changed = true;
        }
    }
    if changed { ini.save(path).map_err(|e| e.to_string()) } else { Ok(()) }
}

/// Carries the values of `from` into `to` (the newer choice wins): a setting of the catalogue when its value is one
/// the launcher would take, and any other key as it is, because the runtime reads keys the launcher has no row for
/// (a line the player wrote by hand, like `hdr = 1`). Comments are not carried.
pub(crate) fn carry(from: &Ini, to: &mut Ini) {
    for (k, v) in from.values() {
        match catalog().iter().find(|s| s.key == k) {
            Some(s) if validate(&k, &v).is_ok() => to.set(&k, &v, &s.section),
            Some(_) => {}
            None if plain_value(&v) => to.set(&k, &v, ""),
            None => {}
        }
    }
}

/// After a build: settings chosen before the game existed go into its hwaoi.ini.
pub fn merge_pending(cfg: &Config, paths: &Paths) -> Result<(), String> {
    let pending = paths.pending_ini();
    let Some(pending_ini) = existing_ini(&pending)? else { return Ok(()) };
    let target = paths::game_ini(&cfg.output(paths));
    if !target.parent().is_some_and(|p| p.is_dir()) {
        return Ok(());
    }
    let mut ini = load_ini(&target)?;
    carry(&pending_ini, &mut ini);
    ini.save(&target).map_err(|e| e.to_string())?;
    std::fs::remove_file(pending).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(name: &str) -> (PathBuf, Config, Paths) {
        let root = std::env::temp_dir().join(format!(
            "aod-settings-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let output = root.join("game");
        std::fs::create_dir_all(paths::game_bin_dir(&output)).unwrap();
        let cfg = Config { output: Some(output.to_string_lossy().into_owned()), ..Config::default() };
        let paths = Paths { exe_dir: root.clone(), data: root.join("data"), engine: None, portable: false };
        (root, cfg, paths)
    }

    #[test]
    fn rejects_invalid_values() {
        assert!(validate("player1_device", "pad:00ff\nfps_limit = 120:1").is_err());
        assert!(validate("volume", "1e2").is_err());
        assert!(validate("player1_device", "pad:030000005e0400008e02000000007200:1").is_ok());
        for value in ["pad::1", "pad:00ff:", "pad:00gg:1", "pad:00ff:1234", "pad:00ff:1:2"] {
            assert!(validate("player1_device", value).is_err(), "{value}");
        }
        for value in [".5", "1.", "+1", "NaN", "inf", "1e2", "1\n2"] {
            assert!(validate("volume", value).is_err(), "{value}");
        }
        assert!(validate("gpu_index", "123").is_ok());
        assert!(validate("gpu_index", "+1").is_err());
    }

    #[test]
    fn write_rejects_non_utf8_without_modifying_it() {
        let (root, cfg, paths) = setup("invalid");
        let path = ini_path(&cfg, &paths);
        let bytes = b"[audio]\nvolume=150\n# \xff\n";
        std::fs::write(&path, bytes).unwrap();
        let err = write(&cfg, &paths, "volume", "100").unwrap_err();
        assert!(err.contains("is not UTF-8 text") && err.contains("nothing was changed"));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn write_creates_a_missing_ini() {
        let (root, cfg, paths) = setup("missing");
        let path = ini_path(&cfg, &paths);
        write(&cfg, &paths, "volume", "100").unwrap();
        assert_eq!(Ini::load(&path).unwrap().values().get("volume").map(String::as_str), Some("100"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn reset_reads_all_files_before_writing() {
        let (root, cfg, paths) = setup("reset");
        let path = ini_path(&cfg, &paths);
        std::fs::create_dir_all(&paths.data).unwrap();
        std::fs::write(&path, "volume=150\n").unwrap();
        std::fs::write(paths.pending_ini(), b"\xff").unwrap();
        assert!(reset(&cfg, &paths, &["volume".to_string()]).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "volume=150\n");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn retired_frame_rates_become_60() {
        let (root, cfg, paths) = setup("retired");
        let path = ini_path(&cfg, &paths);
        retire_values(&path).unwrap();
        assert!(!path.exists());
        std::fs::write(&path, "[frame rate]\nTOTK_FPS_LIMIT = 120\n[audio]\nvolume=150\n").unwrap();
        assert_eq!(read(&cfg, &paths).get("fps_limit").map(String::as_str), Some("60"));
        assert!(std::fs::read_to_string(&path).unwrap().contains("TOTK_FPS_LIMIT = 120"));
        retire_values(&path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[frame rate]\nTOTK_FPS_LIMIT = 60\n[audio]\nvolume=150\n");
        std::fs::write(&path, "fps_limit = 30\n").unwrap();
        retire_values(&path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "fps_limit = 30\n");
        assert!(validate("fps_limit", "90").is_err() && validate("fps_limit", "120").is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn dlaa_of_0_1_2_becomes_dlss() {
        let (root, cfg, paths) = setup("dlaa");
        let path = ini_path(&cfg, &paths);
        std::fs::write(&path, "[display]\ntemporal = dlaa\n").unwrap();
        assert_eq!(read(&cfg, &paths).get("temporal").map(String::as_str), Some("dlss"));
        retire_values(&path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[display]\ntemporal = dlss\n");
        assert!(validate("temporal", "dlaa").is_err() && validate("dlss_quality", "native").is_ok());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn merge_rejects_unreadable_pending_or_target() {
        let (root, cfg, paths) = setup("merge");
        let target = ini_path(&cfg, &paths);
        std::fs::create_dir_all(&paths.data).unwrap();
        std::fs::write(&target, "volume=150\n").unwrap();
        std::fs::write(paths.pending_ini(), b"\xff").unwrap();
        assert!(merge_pending(&cfg, &paths).is_err());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "volume=150\n");
        std::fs::write(paths.pending_ini(), "volume=100\n").unwrap();
        std::fs::write(&target, b"\xff").unwrap();
        assert!(merge_pending(&cfg, &paths).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"\xff");
        assert!(paths.pending_ini().exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
