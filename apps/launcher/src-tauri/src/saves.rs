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

fn is_ready(name: &str) -> bool {
    name.strip_prefix("ready-").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// The folders of `save-tx` with files in them whose name `wanted` accepts.
fn transactions(profile: &Path, wanted: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(profile.join("save-tx")) else { return Vec::new() };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| {
            wanted(&e.file_name().to_string_lossy()) && e.file_type().is_ok_and(|t| t.is_dir()) && holds_a_file(&e.path())
        })
        .map(|e| e.path())
        .collect();
    found.sort();
    found
}

/// The finished saves not swapped in yet (`save-tx/ready-<n>` with files): the game loads the newest at its next start.
fn waiting(profile: &Path) -> Vec<PathBuf> {
    transactions(profile, is_ready)
}

/// The profile folder has a save the game would load: a committed one, or what a save commit left in `save-tx` that
/// the game takes at its next start, a finished save (`ready-<n>`), its staging copy (`new`) or the save it was
/// replacing (`old`). The same rule as the runtime's (save_identity.c `save_identity_pending`, fs_host.c).
pub fn has_save(profile: &Path) -> bool {
    !committed(profile).is_empty() || !transactions(profile, |name| is_ready(name) || name == "new" || name == "old").is_empty()
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
    remove_empty_tree(dir).map_err(|e| io::Error::other(format!("{} is not empty ({e})", dir.display())))
}

/// Removes `dir` and the folders below it when they hold nothing but folders: bottom up, each with remove_dir, so a
/// file or a link that is there (holds_a_file looks at files alone) or turns up stops it and stays where it is.
pub(crate) fn remove_empty_tree(dir: &Path) -> io::Result<()> {
    for e in fs::read_dir(dir)? {
        let e = e?;
        if e.file_type()?.is_dir() {   // the entry's own type: a link to a folder is not walked into
            remove_empty_tree(&e.path())?;
        }
    }
    fs::remove_dir(dir)
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

/// The folder one attempt of `adopt` copies into before anything goes into place: a name of that attempt's own.
fn staging_name() -> String {
    format!("adopt-{}-{}.partial", std::process::id(), paths::stamp())
}

fn is_staging(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("adopt-").and_then(|n| n.strip_suffix(".partial")) else { return false };
    rest.split_once('-').is_some_and(|(pid, stamp)| {
        [pid, stamp].iter().all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
    })
}

/// Removes what an attempt that never finished left (a crash, the power going): staging folders of our own naming
/// that nothing has written to for an hour. A newer one may be another launcher at work and is left alone.
fn clear_old_staging(profile: &Path) {
    let Ok(entries) = fs::read_dir(profile) else { return };
    for e in entries.flatten() {
        let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age >= std::time::Duration::from_secs(3600));
        if old && is_staging(&e.file_name().to_string_lossy()) && e.file_type().is_ok_and(|t| t.is_dir()) {
            let _ = fs::remove_dir_all(e.path());
        }
    }
}

/// Copies the save of the first of `sources` that has one into `profile`, when `profile` has none. Returns the folder
/// it came from, or None when nothing was copied (the folder has a save already, or no source has one).
///
/// Everything is copied into a folder of this attempt's own first and renamed into place only when all of it is
/// there: a copy that fails halfway leaves `profile` without a save and without a half-copied one the game could take
/// for a finished save, and the next start tries again.
pub fn adopt(profile: &Path, sources: &[PathBuf]) -> Result<Option<PathBuf>, String> {
    adopt_with(profile, sources, &copy_tree)
}

fn adopt_with(profile: &Path, sources: &[PathBuf], copy: &dyn Fn(&Path, &Path) -> io::Result<()>) -> Result<Option<PathBuf>, String> {
    if has_save(profile) {
        return Ok(None);
    }
    for source in sources.iter().filter(|s| !same_folder(s, profile)) {
        let saves = committed(source);
        let ready = waiting(source);
        // The account `account.uid` names when it has a save, else the only account there is. With no committed save
        // at all, the named account when a finished save waits in save-tx (a first save not swapped in yet).
        let named = read_uid(source).map(|uid| uid_name(&uid));
        let pick = saves.iter().find(|(name, _)| Some(name) == named.as_ref()).or(if saves.len() == 1 { saves.first() } else { None });
        let (name, from) = match (pick, named) {
            (Some((name, from)), _) => (name.clone(), Some(from.clone())),
            (None, Some(name)) if saves.is_empty() && !ready.is_empty() => (name, None),
            _ => continue,
        };
        let uid = uid_of_name(&name).expect("an account folder name, or one made from an account id");

        fs::create_dir_all(profile).map_err(|e| e.to_string())?;
        clear_old_staging(profile);
        let staging = profile.join(staging_name());
        let done = stage(source, from.as_deref(), &ready, profile, &staging, copy)
            .and_then(|_| publish(profile, &staging, &name, &uid, from.is_some()));
        let _ = fs::remove_dir_all(&staging); // empty after a publish; what a failed copy left otherwise
        done.map_err(|e| e.to_string())?;
        return Ok(Some(source.clone()));
    }
    Ok(None)
}

/// Copies what `adopt` takes from `source` into `staging`: the committed save as `save`, the finished saves not
/// swapped in yet under `tx` (when `profile` has nothing in its own save-tx) and the saves before it as `save-backup`
/// (when `profile` has none). The last are only a comfort: the game starts fine without them, so their copy may fail.
fn stage(source: &Path, from: Option<&Path>, ready: &[PathBuf], profile: &Path, staging: &Path,
         copy: &dyn Fn(&Path, &Path) -> io::Result<()>) -> io::Result<()> {
    fs::create_dir_all(staging)?;
    if let Some(from) = from {
        copy(from, &staging.join("save"))?;
    }
    let mut waiting = 0;
    if profile.join("save-tx").read_dir().map_or(true, |mut d| d.next().is_none()) {
        for folder in ready {
            if let Some(leaf) = folder.file_name() {
                copy(folder, &staging.join("tx").join(leaf))?;
                waiting += 1;
            }
        }
    }
    if from.is_none() && waiting == 0 {
        // Only finished saves to take, and this folder's save-tx holds something of its own: taking the account
        // alone would make the game start a new save under it.
        return Err(io::Error::other("the save waiting to be finished could not be taken: this folder has one of its own"));
    }
    let backups = source.join("save-backup");
    if backups.is_dir() && !profile.join("save-backup").exists() && copy(&backups, &staging.join("save-backup")).is_err() {
        let _ = fs::remove_dir_all(staging.join("save-backup"));
    }
    Ok(())
}

/// Moves a complete `staging` into place: the account, then the finished saves that wait (the newest progress: with
/// them in place a later step that fails still leaves a save the game loads), the committed save, the older saves.
fn publish(profile: &Path, staging: &Path, name: &str, uid: &[u8; 16], committed: bool) -> io::Result<()> {
    let uid_file = profile.join(UID_FILE);
    if uid_file.exists() && read_uid(profile) != Some(*uid) {
        // The account this folder made for itself: kept, under another name.
        fs::rename(&uid_file, profile.join(format!("{UID_FILE}.before-{}", paths::stamp())))?;
    }
    let pending = profile.join(format!("{UID_FILE}.new"));
    fs::write(&pending, uid).and_then(|_| fs::rename(&pending, &uid_file))?;

    if let Ok(entries) = fs::read_dir(staging.join("tx")) {
        fs::create_dir_all(profile.join("save-tx"))?;
        for e in entries.flatten() {
            fs::rename(e.path(), profile.join("save-tx").join(e.file_name()))?;
        }
    }
    if committed {
        let target = profile.join("save").join(name);
        fs::create_dir_all(profile.join("save"))?;
        if target.exists() {
            remove_empty(&target)?;
        }
        fs::rename(staging.join("save"), &target)?;
    }
    if staging.join("save-backup").is_dir() && !profile.join("save-backup").exists() {
        let _ = fs::rename(staging.join("save-backup"), profile.join("save-backup"));
    }
    Ok(())
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
    fn a_copy_that_fails_leaves_no_half_copied_save_behind() {
        let dir = root("fails");
        let (old, new) = (dir.join("old"), dir.join("new"));
        profile_with_save(&old, &UID, b"progress");
        fs::create_dir_all(old.join("save-tx").join("ready-7")).unwrap();
        fs::write(old.join("save-tx").join("ready-7").join("DATA.BIN"), b"newest").unwrap();
        // The game was started once in the new folder: its own account is there.
        let fresh = [7u8; 16];
        fs::create_dir_all(&new).unwrap();
        fs::write(new.join(UID_FILE), fresh).unwrap();

        // The disk fills up while the finished save that waits is copied: half of it is written, then an error.
        let failing = |from: &Path, to: &Path| -> io::Result<()> {
            if from.file_name().is_some_and(|n| n == "ready-7") {
                fs::create_dir_all(to)?;
                fs::write(to.join("DATA.BIN"), b"new")?;
                return Err(io::Error::other("disk full"));
            }
            copy_tree(from, to)
        };
        assert!(adopt_with(&new, std::slice::from_ref(&old), &failing).unwrap_err().contains("disk full"));
        assert!(!has_save(&new), "nothing the game would load");
        assert!(!new.join("save-tx").exists() && !new.join("save").exists());
        assert_eq!(read_uid(&new), Some(fresh), "the folder's own account is still the one in use");
        let left: Vec<String> = fs::read_dir(&new).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(left, [UID_FILE], "no staging folder is left: {left:?}");

        // The next start, with room on the disk, copies all of it: the waiting save too.
        assert_eq!(adopt(&new, std::slice::from_ref(&old)).unwrap(), Some(old));
        assert_eq!(fs::read(new.join("save-tx").join("ready-7").join("DATA.BIN")).unwrap(), b"newest");
        assert_eq!(fs::read(new.join("save").join(uid_name(&UID)).join("DATA.BIN")).unwrap(), b"progress");
        assert_eq!(read_uid(&new), Some(UID));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn what_a_save_commit_left_counts_as_a_save() {
        let dir = root("pending");
        let old = dir.join("old");
        profile_with_save(&old, &UID, b"progress");
        // The game takes each of these at its next start (fs_host.c save_recover): the folder is not empty.
        for (i, name) in ["ready-3", "new", "old"].iter().enumerate() {
            let new = dir.join(format!("new{i}"));
            fs::create_dir_all(new.join("save-tx").join(name)).unwrap();
            assert!(!has_save(&new), "{name} without a file is nothing");
            fs::write(new.join("save-tx").join(name).join("DATA.BIN"), b"mine").unwrap();
            assert!(has_save(&new), "{name}");
            assert_eq!(adopt(&new, std::slice::from_ref(&old)).unwrap(), None, "{name}");
            assert!(!new.join(UID_FILE).exists() && !new.join("save").exists());
        }
        // The working copy no commit took is not a save.
        let work = dir.join("work");
        fs::create_dir_all(work.join("save-tx").join("work")).unwrap();
        fs::write(work.join("save-tx").join("work").join("DATA.BIN"), b"uncommitted").unwrap();
        assert!(!has_save(&work));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_first_save_that_still_waits_is_taken_with_its_account() {
        let dir = root("first");
        let (old, new, nameless) = (dir.join("old"), dir.join("new"), dir.join("nameless"));
        // The game's first save was finished and not swapped in: no committed save yet.
        fs::create_dir_all(old.join("save-tx").join("ready-1")).unwrap();
        fs::write(old.join("save-tx").join("ready-1").join("DATA.BIN"), b"first").unwrap();
        fs::write(old.join(UID_FILE), UID).unwrap();
        assert_eq!(adopt(&new, std::slice::from_ref(&old)).unwrap(), Some(old.clone()));
        assert_eq!(read_uid(&new), Some(UID));
        assert_eq!(fs::read(new.join("save-tx").join("ready-1").join("DATA.BIN")).unwrap(), b"first");
        assert!(has_save(&new) && !new.join("save").exists());
        // Without the account file nobody knows whose save it is: nothing is copied.
        fs::remove_file(old.join(UID_FILE)).unwrap();
        assert_eq!(adopt(&nameless, &[old]).unwrap(), None);
        assert!(!nameless.exists() || !has_save(&nameless));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_staging_folder_another_start_is_using_is_left_alone() {
        let dir = root("staging");
        let (old, new) = (dir.join("old"), dir.join("new"));
        profile_with_save(&old, &UID, b"progress");
        let theirs = new.join("adopt-4242-1795000000.partial");
        fs::create_dir_all(theirs.join("save")).unwrap();
        fs::write(theirs.join("save").join("DATA.BIN"), b"being copied").unwrap();
        let kept = new.join("my-copy.partial");
        fs::create_dir_all(&kept).unwrap();
        fs::write(kept.join("DATA.BIN"), b"the player's own").unwrap();
        assert!(is_staging("adopt-4242-1795000000.partial") && is_staging(&staging_name()));
        assert!(!is_staging("my-copy.partial") && !is_staging("adopt-x-1.partial") && !is_staging("adopt-1-.partial"));

        assert_eq!(adopt(&new, std::slice::from_ref(&old)).unwrap(), Some(old));
        assert_eq!(fs::read(theirs.join("save").join("DATA.BIN")).unwrap(), b"being copied");
        assert_eq!(fs::read(kept.join("DATA.BIN")).unwrap(), b"the player's own");
        assert_eq!(fs::read(new.join("save").join(uid_name(&UID)).join("DATA.BIN")).unwrap(), b"progress");
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
