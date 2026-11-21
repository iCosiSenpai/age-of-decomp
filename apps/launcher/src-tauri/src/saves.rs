//! The player's save when the game folder changes.
//!
//! The game keeps its save data in the game folder, `<game folder>/work/saves/hwaoi196608` (the engine's profile
//! folder, runtime/fs_host.c): `account.uid` is the local account's id, `save/<id>/` the committed save of that
//! account, `save-backup/` the three before it, `save-tx/ready-<n>/` a finished save not swapped in yet. A new game
//! folder has none of it, so a player who moves from the standalone launcher to RecompLauncher, or builds into another
//! folder, would find the game at its beginning, with the save left behind in the other folder.
//!
//! Before the game starts in a folder that has no save yet, the save of a folder this PC used before is copied in
//! (`adopt`). Nothing is deleted or overwritten: a folder with a save of its own is left as it is, the folder the save
//! comes from is only read, and an account id the new folder already had is kept beside the new one.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::paths::{self, Paths};

const UID_FILE: &str = "account.uid";

/// The folder name of an account's committed save: its id as 32 hex digits, high half first (fs_host.c save_root).
/// `account.uid` holds the id as two little-endian 64-bit halves, low first (ov_system.c Uid).
fn uid_name(uid: &[u8; 16]) -> String {
    let lo = u64::from_le_bytes(uid[..8].try_into().unwrap());
    let hi = u64::from_le_bytes(uid[8..].try_into().unwrap());
    format!("{hi:016x}{lo:016x}")
}

fn uid_of_name(name: &str) -> Option<[u8; 16]> {
    if name.len() != 32 || !name.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return None;
    }
    let hi = u64::from_str_radix(&name[..16], 16).ok()?;
    let lo = u64::from_str_radix(&name[16..], 16).ok()?;
    let mut uid = [0u8; 16];
    uid[..8].copy_from_slice(&lo.to_le_bytes());
    uid[8..].copy_from_slice(&hi.to_le_bytes());
    Some(uid)
}

fn read_uid(profile: &Path) -> Option<[u8; 16]> {
    fs::read(profile.join(UID_FILE)).ok()?.try_into().ok().filter(|uid: &[u8; 16]| uid.iter().any(|b| *b != 0))
}

/// A folder holds at least one file, at any depth. Links are not followed.
fn holds_a_file(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else { return false };
    entries.flatten().any(|e| match e.file_type() {
        Ok(t) if t.is_file() => true,
        Ok(t) if t.is_dir() => holds_a_file(&e.path()),
        _ => false,
    })
}

/// The committed saves of a profile folder: (account folder name, its path) for each account that has files.
fn committed(profile: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = fs::read_dir(profile.join("save")) else { return Vec::new() };
    let mut found: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            (uid_of_name(&name).is_some() && e.file_type().is_ok_and(|t| t.is_dir()) && holds_a_file(&e.path()))
                .then(|| (name, e.path()))
        })
        .collect();
    found.sort();
    found
}

/// The finished saves not swapped in yet (`save-tx/ready-<n>` with files): the game loads the newest at its next start.
fn waiting(profile: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(profile.join("save-tx")) else { return Vec::new() };
    entries
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.strip_prefix("ready-").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                && e.file_type().is_ok_and(|t| t.is_dir())
                && holds_a_file(&e.path())
        })
        .map(|e| e.path())
        .collect()
}

/// The profile folder has a save the game would load.
pub fn has_save(profile: &Path) -> bool {
    !committed(profile).is_empty() || !waiting(profile).is_empty()
}

fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        let kind = e.file_type()?;
        if kind.is_dir() {
            copy_tree(&e.path(), &to.join(e.file_name()))?;
        } else if kind.is_file() {
            fs::copy(e.path(), to.join(e.file_name()))?;
        } // links are left out: a save has none
    }
    Ok(())
}

/// Removes a folder that holds no file at any depth (the game makes an empty one for a new account). A folder with a
/// file in it is left alone and reported.
fn remove_empty(dir: &Path) -> io::Result<()> {
    if holds_a_file(dir) {
        return Err(io::Error::other(format!("{} is not empty", dir.display())));
    }
    fs::remove_dir_all(dir)
}

fn same_folder(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// The profile folders this PC may have a save in, other than `output`'s: the game folder the launcher used before
/// this one, the standalone launcher's own game folder (the one its settings name, and where it builds by default).
pub fn earlier_profiles(cfg: &Config, output: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    let mut add = |game_folder: PathBuf| {
        let profile = paths::saves_dir(&game_folder);
        if !same_folder(&profile, &paths::saves_dir(output)) && !found.iter().any(|f| same_folder(f, &profile)) {
            found.push(profile);
        }
    };
    if let Some(previous) = cfg.previous_output.as_deref().filter(|p| !p.is_empty()) {
        add(PathBuf::from(previous));
    }
    let standalone = paths::standalone_data_dir();
    let named = fs::read_to_string(standalone.join("launcher.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|json| json.get("output").and_then(|o| o.as_str()).map(PathBuf::from));
    if let Some(folder) = named.filter(|f| !f.as_os_str().is_empty()) {
        add(folder);
    }
    add(standalone.join("game"));
    found
}

/// Copies the save of the first of `sources` that has one into `profile`, when `profile` has none. Returns the folder
/// it came from, or None when nothing was copied (the folder has a save already, or no source has one).
///
/// The account goes first and the save after it, through a folder of another name renamed into place at the end: a
/// copy that fails halfway leaves `profile` without a save, and the next start tries again.
pub fn adopt(profile: &Path, sources: &[PathBuf]) -> Result<Option<PathBuf>, String> {
    if has_save(profile) {
        return Ok(None);
    }
    let text = |e: io::Error| e.to_string();
    for source in sources.iter().filter(|s| !same_folder(s, profile)) {
        let saves = committed(source);
        // The account `account.uid` names when it has a save, else the only account there is.
        let named = read_uid(source).map(|uid| uid_name(&uid));
        let pick = saves.iter().find(|(name, _)| Some(name) == named.as_ref()).or(if saves.len() == 1 { saves.first() } else { None });
        let Some((name, from)) = pick else { continue };
        let uid = uid_of_name(name).expect("committed() keeps account folder names only");

        fs::create_dir_all(profile).map_err(text)?;
        let uid_file = profile.join(UID_FILE);
        if uid_file.exists() && read_uid(profile) != Some(uid) {
            // The account this folder made for itself: kept, under another name.
            fs::rename(&uid_file, profile.join(format!("{UID_FILE}.before-{}", paths::stamp()))).map_err(text)?;
        }
        let pending = profile.join(format!("{UID_FILE}.new"));
        fs::write(&pending, uid).and_then(|_| fs::rename(&pending, &uid_file)).map_err(text)?;

        let save = profile.join("save");
        fs::create_dir_all(&save).map_err(text)?;
        let target = save.join(name);
        if target.exists() {
            remove_empty(&target).map_err(text)?;
        }
        let partial = save.join(format!("{name}.partial"));
        if partial.exists() {
            fs::remove_dir_all(&partial).map_err(text)?; // what an earlier try that failed left
        }
        copy_tree(from, &partial).and_then(|_| fs::rename(&partial, &target)).map_err(text)?;

        // The saves before it and a finished save not swapped in yet come along when this folder has neither; the
        // game starts fine without them, so a failure here is not one of the copy.
        let backups = source.join("save-backup");
        if backups.is_dir() && !profile.join("save-backup").exists() {
            let _ = copy_tree(&backups, &profile.join("save-backup"));
        }
        if profile.join("save-tx").read_dir().map_or(true, |mut d| d.next().is_none()) {
            for ready in waiting(source) {
                if let Some(leaf) = ready.file_name() {
                    let _ = copy_tree(&ready, &profile.join("save-tx").join(leaf));
                }
            }
        }
        return Ok(Some(source.clone()));
    }
    Ok(None)
}

/// `adopt` for the game folder `output`, with the folders `earlier_profiles` knows. For the callers that start the game.
pub fn adopt_for(cfg: &Config, _paths: &Paths, output: &Path) -> Result<Option<PathBuf>, String> {
    adopt(&paths::saves_dir(output), &earlier_profiles(cfg, output))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("aod-saves-{tag}-{}-{}", std::process::id(), paths::stamp()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    const UID: [u8; 16] = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x01];

    /// A profile folder with a committed save for `uid`, as the game leaves it.
    fn profile_with_save(dir: &Path, uid: &[u8; 16], data: &[u8]) {
        fs::create_dir_all(dir.join("save").join(uid_name(uid)).join("slot")).unwrap();
        fs::write(dir.join(UID_FILE), uid).unwrap();
        fs::write(dir.join("save").join(uid_name(uid)).join("DATA.BIN"), data).unwrap();
        fs::write(dir.join("save").join(uid_name(uid)).join("slot").join("a.bin"), b"slot").unwrap();
    }

    #[test]
    fn account_folder_names_follow_the_engine() {
        // fs_host.c: "%016llx%016llx" of the high half, then the low half; account.uid holds low, then high.
        assert_eq!(uid_name(&UID), "01ffeeddccbbaa998877665544332211");
        assert_eq!(uid_of_name("01ffeeddccbbaa998877665544332211"), Some(UID));
        assert_eq!(uid_of_name("01FFEEDDCCBBAA998877665544332211"), None); // the engine writes lower case
        assert_eq!(uid_of_name("01ffeeddccbbaa998877665544332211.partial"), None);
        assert_eq!(uid_of_name("save"), None);
    }

    #[test]
    fn a_folder_without_a_save_takes_the_one_of_an_earlier_folder() {
        let dir = root("adopt");
        let (old, new) = (dir.join("old"), dir.join("new"));
        profile_with_save(&old, &UID, b"progress");
        fs::create_dir_all(old.join("save-backup").join("20261120-101010")).unwrap();
        fs::write(old.join("save-backup").join("20261120-101010").join("DATA.BIN"), b"older").unwrap();
        fs::create_dir_all(old.join("save-tx").join("work")).unwrap();
        fs::write(old.join("save-tx").join("work").join("DATA.BIN"), b"uncommitted").unwrap();
        fs::create_dir_all(old.join("temp")).unwrap();
        fs::write(old.join("temp").join("t"), b"t").unwrap();

        assert!(!has_save(&new));
        assert_eq!(adopt(&new, &[dir.join("missing"), old.clone()]).unwrap(), Some(old.clone()));
        assert!(has_save(&new));
        assert_eq!(read_uid(&new), Some(UID));
        let save = new.join("save").join(uid_name(&UID));
        assert_eq!(fs::read(save.join("DATA.BIN")).unwrap(), b"progress");
        assert_eq!(fs::read(save.join("slot").join("a.bin")).unwrap(), b"slot");
        assert_eq!(fs::read(new.join("save-backup").join("20261120-101010").join("DATA.BIN")).unwrap(), b"older");
        // what no save commit took and the temporary files stay behind
        assert!(!new.join("save-tx").join("work").exists() && !new.join("temp").exists());
        // the folder it came from is as it was
        assert_eq!(fs::read(old.join("save").join(uid_name(&UID)).join("DATA.BIN")).unwrap(), b"progress");
        assert!(old.join("save-tx").join("work").join("DATA.BIN").is_file());
        // a second start copies nothing
        assert_eq!(adopt(&new, &[old]).unwrap(), None);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_folder_that_only_made_an_account_keeps_it_beside_the_adopted_one() {
        let dir = root("fresh");
        let (old, new) = (dir.join("old"), dir.join("new"));
        profile_with_save(&old, &UID, b"progress");
        // the game was started once in the new folder: a new account and its empty save folder
        let fresh = [7u8; 16];
        fs::create_dir_all(new.join("save").join(uid_name(&fresh))).unwrap();
        fs::write(new.join(UID_FILE), fresh).unwrap();

        assert_eq!(adopt(&new, std::slice::from_ref(&old)).unwrap(), Some(old));
        assert_eq!(read_uid(&new), Some(UID));
        let kept: Vec<_> = fs::read_dir(&new).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with("account.uid.before-")).collect();
        assert_eq!(kept.len(), 1);
        assert_eq!(fs::read(kept[0].path()).unwrap(), fresh);
        assert!(new.join("save").join(uid_name(&UID)).join("DATA.BIN").is_file());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_folder_with_a_save_of_its_own_is_left_alone() {
        let dir = root("own");
        let (old, new) = (dir.join("old"), dir.join("new"));
        profile_with_save(&old, &UID, b"old progress");
        let mine = [9u8; 16];
        profile_with_save(&new, &mine, b"new progress");

        assert_eq!(adopt(&new, &[old]).unwrap(), None);
        assert_eq!(read_uid(&new), Some(mine));
        assert_eq!(fs::read(new.join("save").join(uid_name(&mine)).join("DATA.BIN")).unwrap(), b"new progress");
        assert!(!new.join("save").join(uid_name(&UID)).exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_save_without_its_account_file_is_found_by_its_folder_and_two_accounts_are_not_guessed() {
        let dir = root("nouid");
        let (old, new, two, other) = (dir.join("old"), dir.join("new"), dir.join("two"), dir.join("other"));
        profile_with_save(&old, &UID, b"progress");
        fs::remove_file(old.join(UID_FILE)).unwrap();
        assert_eq!(adopt(&new, std::slice::from_ref(&old)).unwrap(), Some(old));
        assert_eq!(read_uid(&new), Some(UID));

        // two accounts with saves and no account file: which one is the player's is not known, nothing is copied
        profile_with_save(&two, &UID, b"a");
        profile_with_save(&two, &[3u8; 16], b"b");
        fs::remove_file(two.join(UID_FILE)).unwrap();
        assert_eq!(adopt(&other, &[two]).unwrap(), None);
        assert!(!has_save(&other));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_folder_itself_and_folders_without_a_save_are_not_sources() {
        let dir = root("self");
        let (empty, new) = (dir.join("empty"), dir.join("new"));
        fs::create_dir_all(empty.join("save")).unwrap();
        fs::create_dir_all(&new).unwrap();
        assert_eq!(adopt(&new, &[new.clone(), empty]).unwrap(), None);
        assert!(!new.join(UID_FILE).exists());
        let _ = fs::remove_dir_all(dir);
    }
}
