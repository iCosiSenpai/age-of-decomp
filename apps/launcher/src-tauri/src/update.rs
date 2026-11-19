//! GitHub release updates. Downloads are staged beside the installed compiler; a separate copy of aod-cli
//! waits for the UI to exit, swaps only release-owned paths, rolls back failures, and restarts the compiler.
use reqwest::blocking::Client;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const API: &str = "https://api.github.com/repos/iCosiSenpai/age-of-decomp";
const OWNED: &[&str] = &["AgeOfDecomp.exe", "aod-cli.exe", "engine", "licenses", "LICENSE.txt", "README.txt",
    "recomplauncher.json", "build-info.json"];

#[derive(Clone, Deserialize)]
struct Asset { name: String, url: String, size: u64, digest: Option<String> }

#[derive(Deserialize)]
struct Release { tag_name: String, draft: bool, assets: Vec<Asset> }

#[derive(Serialize)]
pub struct UpdateInfo {
    current: String,
    latest: Option<String>,
    available: bool,
    previous_error: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Plan { destination: PathBuf, parent: u32 }

fn hidden(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    { use std::os::windows::process::CommandExt; cmd.creation_flags(0x0800_0000); }
    cmd
}

fn existing_github_token() -> Option<String> {
    std::env::var("GH_TOKEN").or_else(|_| std::env::var("GITHUB_TOKEN")).ok().or_else(|| {
        hidden(Command::new("gh").args(["auth", "token", "--hostname", "github.com"]))
            .stdin(Stdio::null()).stderr(Stdio::null()).output().ok()
            .filter(|o| o.status.success()).and_then(|o| String::from_utf8(o.stdout).ok())
    })
}

fn client(token: Option<&str>) -> Result<Client, String> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::ACCEPT, "application/vnd.github+json".parse().unwrap());
    if let Some(token) = token {
        let mut value = reqwest::header::HeaderValue::from_str(&format!("Bearer {}", token.trim()))
            .map_err(|_| "Invalid GitHub credentials")?;
        value.set_sensitive(true);
        headers.insert(reqwest::header::AUTHORIZATION, value);
    }
    Client::builder().user_agent(concat!("AgeOfDecomp/", env!("CARGO_PKG_VERSION")))
        .default_headers(headers).connect_timeout(Duration::from_secs(10)).timeout(Duration::from_secs(600))
        .build().map_err(|e| e.to_string())
}

fn release_response(client: &Client) -> Result<reqwest::blocking::Response, String> {
    client.get(format!("{API}/releases?per_page=100")).timeout(Duration::from_secs(20))
        .send().map_err(|_| "Could not reach GitHub. Check your connection and try again.".into())
}

fn parse_releases(response: reqwest::blocking::Response) -> Result<Vec<Release>, String> {
    if response.status() == reqwest::StatusCode::NOT_FOUND || response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("GitHub could not access the releases. Check your GitHub credentials and try again.".into());
    }
    let response = response.error_for_status().map_err(|e| format!("GitHub update check failed ({})", e.status().map(|s| s.as_u16()).unwrap_or(0)))?;
    let mut bytes = Vec::new();
    response.take(2 * 1024 * 1024).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|_| "GitHub returned an invalid release list".into())
}

fn release_feed() -> Result<(Client, Vec<Release>), String> {
    // Public releases need no login. Only look for an existing credential if GitHub
    // denies the anonymous request (for example for a private repo or rate limit).
    let public = client(None)?;
    let response = release_response(&public)?;
    if matches!(response.status().as_u16(), 401 | 403 | 404 | 429)
        && let Some(token) = existing_github_token()
    {
        let authenticated = client(Some(&token))?;
        let list = parse_releases(release_response(&authenticated)?)?;
        return Ok((authenticated, list));
    }
    Ok((public, parse_releases(response)?))
}

fn newest(releases: &[Release]) -> Option<(&Release, Version)> {
    // Preview releases participate: the project currently distributes previews. Drafts never do.
    releases.iter().filter(|r| !r.draft && r.assets.iter().any(|a| a.name == asset_name(&r.tag_name)))
        .filter_map(|r| Version::parse(r.tag_name.trim_start_matches('v')).ok().map(|v| (r, v)))
        .max_by(|a, b| a.1.cmp(&b.1))
}

fn asset_name(tag: &str) -> String {
    format!("AgeOfDecomp-{tag}-Windows-x64.zip")
}

pub fn check() -> Result<UpdateInfo, String> {
    let (_, list) = release_feed()?;
    let latest = newest(&list);
    let current = Version::parse(crate::VERSION).map_err(|e| e.to_string())?;
    Ok(UpdateInfo { current: crate::VERSION.into(), latest: latest.as_ref().map(|(r, _)| r.tag_name.clone()),
        available: cfg!(windows) && latest.is_some_and(|(_, v)| v > current),
        previous_error: std::env::var("AOD_UPDATE_ERROR").ok() })
}

fn download(client: &Client, asset: &Asset, path: &Path, mut progress: impl FnMut(f64)) -> Result<(), String> {
    if !asset.url.starts_with(&format!("{API}/releases/assets/")) || asset.size > 2 * 1024 * 1024 * 1024 {
        return Err("Invalid release asset".into());
    }
    let mut response = client.get(&asset.url).header(reqwest::header::ACCEPT, "application/octet-stream")
        .send().and_then(|r| r.error_for_status()).map_err(|_| "Could not download the update from GitHub")?;
    let mut file = fs::File::create(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut count = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let n = response.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 { break; }
        count += n as u64;
        if count > asset.size { return Err("Update download exceeds its declared size".into()); }
        file.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
        hasher.update(&buffer[..n]);
        progress(count as f64 / asset.size.max(1) as f64);
    }
    file.sync_all().map_err(|e| e.to_string())?;
    let digest = format!("sha256:{}", hasher.finalize().iter().map(|b| format!("{b:02x}")).collect::<String>());
    if count != asset.size || asset.digest.as_deref() != Some(digest.as_str()) {
        return Err("Update integrity check failed. The installed compiler was kept.".into());
    }
    Ok(())
}

fn extract(zip_path: &Path, dest: &Path, tag: &str) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(fs::File::open(zip_path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let root = format!("AgeOfDecomp-{tag}-Windows-x64");
    let mut total = 0u64;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let path = file.enclosed_name().ok_or("Unsafe path in update archive")?;
        let mut parts = path.components();
        if parts.next().and_then(|c| c.as_os_str().to_str()) != Some(root.as_str()) {
            return Err("Unexpected update archive layout".into());
        }
        let rel = parts.as_path();
        if rel.as_os_str().is_empty() { continue; }
        let top = rel.components().next().and_then(|c| c.as_os_str().to_str()).ok_or("Invalid update path")?;
        if !OWNED.contains(&top) || file.name().contains([':', '\\'])
            || file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            return Err("Unexpected file in update archive".into());
        }
        total = total.checked_add(file.size()).ok_or("Update archive too large")?;
        if total > 4 * 1024 * 1024 * 1024 { return Err("Update archive too large".into()); }
        let target = dest.join(rel);
        if file.is_dir() { fs::create_dir_all(&target).map_err(|e| e.to_string())?; }
        else {
            fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
            let mut out = fs::File::create(&target).map_err(|e| e.to_string())?;
            std::io::copy(&mut file, &mut out).map_err(|e| e.to_string())?;
        }
    }
    for required in ["AgeOfDecomp.exe", "aod-cli.exe", "engine/bundle.json", "recomplauncher.json", "build-info.json"] {
        if !dest.join(required).is_file() { return Err(format!("Update is missing {required}")); }
    }
    Ok(())
}

pub fn stage(mut progress: impl FnMut(f64)) -> Result<(), String> {
    if !cfg!(windows) { return Err("Automatic installation is currently available for the Windows portable compiler.".into()); }
    let destination = std::env::current_exe().map_err(|e| e.to_string())?.parent().unwrap().to_path_buf();
    if !destination.join("build-info.json").is_file() || !destination.join("engine/bundle.json").is_file() {
        return Err("Run the compiler from the downloaded release folder to install updates.".into());
    }
    let (client, list) = release_feed()?;
    let (release, version) = newest(&list).ok_or("No compatible release is available")?;
    if version <= Version::parse(crate::VERSION).map_err(|e| e.to_string())? { return Err("The compiler is already up to date.".into()); }
    let asset = release.assets.iter().find(|a| a.name == asset_name(&release.tag_name)).unwrap();
    let stage = destination.join(format!(".aod-update-{}-{}", crate::paths::stamp(), std::process::id()));
    fs::create_dir(&stage).map_err(|e| format!("Cannot write the update beside the compiler ({e})"))?;
    let package = stage.join("package.zip");
    download(&client, asset, &package, &mut progress)?;
    extract(&package, &stage.join("new"), &release.tag_name)?;
    if installed_version(&stage.join("new"))? != version {
        return Err("The update package version does not match its release tag".into());
    }
    fs::remove_file(package).map_err(|e| e.to_string())?;
    fs::copy(destination.join("aod-cli.exe"), stage.join("update-helper.exe")).map_err(|e| e.to_string())?;
    fs::write(stage.join("plan.json"), serde_json::to_vec(&Plan { destination, parent: std::process::id() }).unwrap())
        .map_err(|e| e.to_string())?;
    hidden(Command::new(stage.join("update-helper.exe")).arg("--apply-update").arg(&stage))
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(windows)]
fn wait_for_parent(pid: u32) -> Result<(), String> {
    use windows_sys::Win32::{Foundation::CloseHandle, System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE}};
    unsafe {
        let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if handle.is_null() { return Ok(()); }
        let result = WaitForSingleObject(handle, 120_000);
        CloseHandle(handle);
        if result == 0 { Ok(()) } else { Err("Compiler did not exit; update was left staged".into()) }
    }
}

fn swap(stage: &Path, destination: &Path) -> Result<(), String> {
    let backup = stage.join("backup");
    fs::create_dir(&backup).map_err(|e| e.to_string())?;
    let mut moved = Vec::new();
    let mut installed = Vec::new();
    let result = (|| -> std::io::Result<()> {
        for name in OWNED {
            if !stage.join("new").join(name).exists() { continue; }
            if destination.join(name).exists() {
                fs::rename(destination.join(name), backup.join(name))?;
                moved.push(*name);
            }
            fs::rename(stage.join("new").join(name), destination.join(name))?;
            installed.push(*name);
        }
        Ok(())
    })();
    if let Err(error) = result {
        let mut failures = Vec::new();
        for name in installed.iter().rev() {
            if let Err(e) = fs::rename(destination.join(name), stage.join("new").join(name)) { failures.push(e.to_string()); }
        }
        for name in moved.iter().rev() {
            if let Err(e) = fs::rename(backup.join(name), destination.join(name)) { failures.push(e.to_string()); }
        }
        return Err(format!("Update failed ({error}). Rollback errors: {failures:?}. Backup: {}", backup.display()));
    }
    Ok(())
}

fn installed_version(dir: &Path) -> Result<Version, String> {
    let info: serde_json::Value = serde_json::from_slice(&fs::read(dir.join("build-info.json")).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    Version::parse(info["compiler_version"].as_str().ok_or("Package has no compiler version")?).map_err(|e| e.to_string())
}

#[cfg(windows)]
fn install_lock(destination: &Path) -> Result<fs::File, String> {
    use std::os::windows::fs::OpenOptionsExt;
    for _ in 0..1200 {
        match fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).share_mode(0)
            .open(destination.join(".aod-update.lock")) {
            Ok(file) => return Ok(file),
            Err(e) if e.raw_os_error() == Some(32) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("Another compiler update is still being installed".into())
}

/// Internal helper entry point. The helper and plan live on the installation's filesystem.
pub fn apply(stage: &Path) -> Result<(), String> {
    let stage = stage.canonicalize().map_err(|e| e.to_string())?;
    let plan: Plan = serde_json::from_slice(&fs::read(stage.join("plan.json")).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let destination = plan.destination.canonicalize().map_err(|e| e.to_string())?;
    if stage.parent() != Some(destination.as_path()) || !stage.file_name().unwrap().to_string_lossy().starts_with(".aod-update-") {
        return Err("Invalid update staging directory".into());
    }
    #[cfg(windows)]
    wait_for_parent(plan.parent)?;
    #[cfg(windows)]
    let _lock = install_lock(&destination)?;
    let candidate = installed_version(&stage.join("new"))?;
    if installed_version(&destination).is_ok_and(|current| current >= candidate) {
        // Another instance already installed this release or a newer one while this package downloaded.
        Command::new(destination.join("AgeOfDecomp.exe")).current_dir(&destination).spawn().map_err(|e| e.to_string())?;
        return Ok(());
    }
    let result = swap(&stage, &destination);
    fs::write(stage.join("result.log"), match &result { Ok(()) => "Update installed".into(), Err(e) => e.clone() })
        .map_err(|e| e.to_string())?;
    let mut restart = Command::new(destination.join("AgeOfDecomp.exe"));
    restart.current_dir(&destination).env_remove("AOD_UPDATE_ERROR");
    if let Err(error) = &result { restart.env("AOD_UPDATE_ERROR", error); }
    restart.spawn().map_err(|e| e.to_string())?;
    // Keep the previous compiler in backup for recovery. No player folders are traversed by this updater.
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_published_compatible_semver() {
        let releases: Vec<Release> = serde_json::from_str(r#"[
            {"tag_name":"v0.9.0","draft":false,"assets":[{"name":"AgeOfDecomp-v0.9.0-Windows-x64.zip","url":"","size":0}]},
            {"tag_name":"v0.10.0","draft":false,"assets":[{"name":"AgeOfDecomp-v0.10.0-Windows-x64.zip","url":"","size":0}]},
            {"tag_name":"v2.0.0","draft":true,"assets":[]},
            {"tag_name":"v3.0.0","draft":false,"assets":[]}]"#).unwrap();
        assert_eq!(newest(&releases).unwrap().0.tag_name, "v0.10.0");
    }

    #[test]
    fn swaps_only_release_files_and_keeps_a_backup() {
        let stage = std::env::temp_dir().join(format!("aod-update-test-{}", std::process::id()));
        let dest = stage.join("dest");
        fs::create_dir_all(&dest).unwrap();
        fs::create_dir_all(stage.join("new")).unwrap();
        fs::write(dest.join("AgeOfDecomp.exe"), "old").unwrap();
        fs::write(stage.join("new/AgeOfDecomp.exe"), "new").unwrap();
        fs::write(dest.join("game.nup"), "player data").unwrap();
        fs::write(dest.join("aod-cli.exe"), "old cli").unwrap();
        fs::write(stage.join("new/aod-cli.exe"), "new cli").unwrap();
        assert!(swap(&stage, &dest).is_ok());
        assert_eq!(fs::read_to_string(dest.join("AgeOfDecomp.exe")).unwrap(), "new");
        assert_eq!(fs::read_to_string(stage.join("backup/AgeOfDecomp.exe")).unwrap(), "old");
        assert_eq!(fs::read_to_string(dest.join("game.nup")).unwrap(), "player data");
        fs::remove_dir_all(stage).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn rolls_back_after_a_locked_file() {
        use std::os::windows::fs::OpenOptionsExt;
        let stage = std::env::temp_dir().join(format!("aod-rollback-test-{}", std::process::id()));
        let dest = stage.join("dest");
        fs::create_dir_all(&dest).unwrap();
        fs::create_dir_all(stage.join("new")).unwrap();
        for name in ["AgeOfDecomp.exe", "aod-cli.exe"] {
            fs::write(dest.join(name), "old").unwrap();
            fs::write(stage.join("new").join(name), "new").unwrap();
        }
        let lock = fs::OpenOptions::new().read(true).share_mode(0).open(dest.join("aod-cli.exe")).unwrap();
        assert!(swap(&stage, &dest).is_err());
        assert_eq!(fs::read_to_string(dest.join("AgeOfDecomp.exe")).unwrap(), "old");
        drop(lock);
        assert_eq!(fs::read_to_string(dest.join("aod-cli.exe")).unwrap(), "old");
        fs::remove_dir_all(stage).unwrap();
    }

    #[test]
    fn rejects_player_files_in_an_update_archive() {
        use zip::write::SimpleFileOptions;
        let root = std::env::temp_dir().join(format!("aod-archive-test-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("update.zip");
        let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        zip.start_file("AgeOfDecomp-v1.0.0-Windows-x64/game/hwaoi.ini", SimpleFileOptions::default()).unwrap();
        zip.write_all(b"player settings").unwrap();
        zip.finish().unwrap();
        assert!(extract(&path, &root.join("new"), "v1.0.0").is_err());
        assert!(!root.join("new/game/hwaoi.ini").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "requires a published Windows release and GitHub access"]
    fn verifies_and_extracts_published_package() {
        let (client, list) = release_feed().unwrap();
        let (release, _) = newest(&list).expect("published compatible release");
        let asset = release.assets.iter().find(|a| a.name == asset_name(&release.tag_name)).unwrap();
        let root = std::env::temp_dir().join(format!("aod-live-update-test-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        download(&client, asset, &root.join("package.zip"), |_| {}).unwrap();
        extract(&root.join("package.zip"), &root.join("new"), &release.tag_name).unwrap();
        assert!(root.join("new/engine/source/runtime/ui/menu.cpp").is_file());
        let info: serde_json::Value = serde_json::from_slice(&fs::read(root.join("new/build-info.json")).unwrap()).unwrap();
        assert_eq!(info["dirty"], false);
        fs::remove_dir_all(root).unwrap();
    }
}
