//! The exact game build the engine supports, and the check of a player's NUP against it. Mirrors the engine's own
//! `verified_game` (tools/hwaoi_romfs.py): newest update shadows the base by path; ExeFS module hashes and the RomFS
//! inventory must match. The RomFS inventory is computed from the TOC's per-file SHA-256, so no resource is read.

use crate::{Entry, Kind, Nup, Package, Result};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Engine profile JSON (copy of upstream `config/games/hwaoi-update-196608.json`).
pub const HWAOI_PROFILE_JSON: &str = include_str!("../../../engine/profile/hwaoi-update-196608.json");

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Inventory {
    pub algorithm: String,
    pub files: usize,
    pub sha256: String,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct GameProfile {
    pub title: String,
    pub title_id: String,
    pub ncm_version: u32,
    pub npdm_sha256: String,
    pub romfs_inventory: Inventory,
    pub modules: BTreeMap<String, String>,
}

impl GameProfile {
    pub fn hwaoi() -> GameProfile {
        serde_json::from_str(HWAOI_PROFILE_JSON).expect("bundled profile is valid JSON")
    }
    pub fn title_id(&self) -> u64 {
        u64::from_str_radix(&self.title_id, 16).unwrap_or(0)
    }
}

/// One line of the check, shown to the player in the setup's "Checks" step.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Step {
    pub id: &'static str,
    pub ok: bool,
    pub detail: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Report {
    pub ok: bool,
    pub title_id: String,
    pub versions: Vec<u32>,
    pub has_base: bool,
    pub has_update: bool,
    pub addons: usize,
    pub build_id: Option<String>,
    pub romfs_files: usize,
    pub romfs_bytes: u64,
    pub steps: Vec<Step>,
}

type Pick<'a> = Option<(&'a Nup, &'a Package)>;

/// Packages chosen from one or two NUPs: (nup, package) for the base (if any) and the newest matching update.
fn choose<'a>(nups: &[&'a Nup], title: u64) -> (Pick<'a>, Pick<'a>) {
    let mut base = None;
    let mut update: Option<(&Nup, &Package)> = None;
    for nup in nups {
        for p in nup.packages() {
            let matches = p.title_id == title || p.title_id == (title | 0x800);
            match p.kind {
                Kind::Base if matches && base.is_none() => base = Some((*nup, p)),
                Kind::Update if matches && update.is_none_or(|(_, u)| p.version > u.version) => update = Some((*nup, p)),
                _ => {}
            }
        }
    }
    (base, update)
}

pub fn inventory_hash(files: &BTreeMap<Vec<u8>, (u64, [u8; 32])>) -> String {
    let mut h = Sha256::new();
    h.update(b"hwaoi-romfs-inventory-v1\0");
    h.update((files.len() as u64).to_le_bytes());
    for (name, (size, sha)) in files {
        h.update((name.len() as u32).to_le_bytes());
        h.update(name);
        h.update(size.to_le_bytes());
        h.update(sha);
    }
    hex::encode(h.finalize())
}

/// Checks `nup` (and an optional separate update NUP) against `profile`. Hashing the ExeFS modules decompresses about
/// 30 MB; the RomFS is checked from the TOC only.
pub fn check(nup: &Nup, update_nup: Option<&Nup>, profile: &GameProfile) -> Result<Report> {
    let title = profile.title_id();
    let mut nups = vec![nup];
    if let Some(u) = update_nup {
        nups.push(u);
    }
    let (base, update) = choose(&nups, title);
    let mut steps = Vec::new();
    let mut versions: Vec<u32> = nups
        .iter()
        .flat_map(|n| n.packages().iter().filter(|p| p.kind.is_program()).map(|p| p.version))
        .collect();
    versions.sort_unstable();
    versions.dedup();
    let addons = nups.iter().flat_map(|n| n.packages()).filter(|p| matches!(p.kind, Kind::Addon | Kind::AddonUpdate)).count();

    let title_ok = base.is_some() || update.is_some();
    steps.push(Step {
        id: "title",
        ok: title_ok,
        detail: if title_ok {
            format!("{} ({})", profile.title, profile.title_id)
        } else {
            let found: Vec<String> = nup.packages().iter().map(|p| format!("{:016X}", p.title_id)).collect();
            format!("expected title {}, found {}", profile.title_id, found.join(", "))
        },
    });

    let update_ok = update.is_some_and(|(_, u)| u.version == profile.ncm_version);
    steps.push(Step {
        id: "version",
        ok: update_ok,
        detail: match update {
            Some((_, u)) if u.version == profile.ncm_version => format!("update NCM {}", u.version),
            Some((_, u)) => format!("update NCM {} found, NCM {} needed", u.version, profile.ncm_version),
            None => format!("no update in the file: update NCM {} is needed", profile.ncm_version),
        },
    });
    let complete = base.is_some() || update.is_some_and(|(_, u)| u.standalone_update);
    steps.push(Step {
        id: "base",
        ok: complete,
        detail: if base.is_some() { "base game present".into() } else { "base game missing".into() },
    });

    // ExeFS of the program that runs: the update shadows the base by name.
    let mut exefs: BTreeMap<String, (&Nup, Entry)> = BTreeMap::new();
    for (n, p) in [base, update].into_iter().flatten() {
        if let Some(d) = p.exefs_dir {
            for (name, e) in n.walk(d)? {
                exefs.insert(name, (n, e));
            }
        }
    }
    let mut want: Vec<(String, String)> = profile.modules.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    want.push(("main.npdm".into(), profile.npdm_sha256.clone()));
    let mut bad = Vec::new();
    if update_ok {
        for (name, sha) in &want {
            match exefs.get(name) {
                None => bad.push(format!("{name} missing")),
                Some((n, e)) => {
                    // The TOC hash is the hash of the content; read_all re-verifies it while decompressing.
                    if hex::encode(e.sha256) != *sha {
                        bad.push(format!("{name} differs"));
                    } else if n.read_all(e).is_err() {
                        bad.push(format!("{name} is damaged"));
                    }
                }
            }
        }
    } else {
        bad.push("skipped (wrong version)".into());
    }
    steps.push(Step {
        id: "exefs",
        ok: bad.is_empty(),
        detail: if bad.is_empty() { format!("{} modules match", want.len()) } else { bad.join(", ") },
    });

    let mut romfs: BTreeMap<Vec<u8>, (u64, [u8; 32])> = BTreeMap::new();
    for (n, p) in [base, update].into_iter().flatten() {
        if let Some(d) = p.romfs_dir {
            for (path, e) in n.walk(d)? {
                romfs.insert(path.into_bytes(), (e.size, e.sha256));
            }
        }
    }
    let romfs_bytes = romfs.values().map(|(s, _)| *s).sum();
    let inv_ok = romfs.len() == profile.romfs_inventory.files && inventory_hash(&romfs) == profile.romfs_inventory.sha256;
    steps.push(Step {
        id: "romfs",
        ok: inv_ok,
        detail: format!("{} files, {:.1} GiB", romfs.len(), romfs_bytes as f64 / (1u64 << 30) as f64),
    });

    let newest = update.or(base);
    let build_id = match newest {
        Some((n, p)) => n.build_id(p)?,
        None => None,
    };
    Ok(Report {
        ok: steps.iter().all(|s| s.ok),
        title_id: profile.title_id.clone(),
        versions,
        has_base: base.is_some(),
        has_update: update.is_some(),
        addons,
        build_id,
        romfs_files: romfs.len(),
        romfs_bytes,
        steps,
    })
}
