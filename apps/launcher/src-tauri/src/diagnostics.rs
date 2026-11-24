//! Optional diagnostics. Only allowlisted summaries leave the PC; logs stay local.

use crate::{VERSION, paths::Paths};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

const ENDPOINT: Option<&str> = option_env!("AOD_DIAGNOSTICS_URL");
const QUEUE: [&str; 4] = ["launcher_crash", "game_crash", "build_failure", "game_session"];
static SEND: Mutex<()> = Mutex::new(());
static SAVE_CHOICE: Mutex<()> = Mutex::new(());

fn version_ok(version: &str) -> bool {
    version.len() <= 20 && semver::Version::parse(version).is_ok_and(|v| {
        v.pre.is_empty() && v.build.is_empty() && [v.major, v.minor, v.patch].iter().all(|n| *n < 1_000_000)
    })
}

#[derive(Serialize)]
struct Report<'a> {
    version: &'a str,
    os: &'static str,
    event: &'static str,
    detail: &'static str,
    fps: Option<u32>,
}

#[derive(Deserialize, Serialize)]
struct Session {
    pid: u32,
    started: u64,
    version: String,
}

fn dir(paths: &Paths) -> PathBuf {
    paths.data.join("diagnostics")
}

pub fn available() -> bool {
    ENDPOINT.is_some_and(|url| {
        reqwest::Url::parse(url).is_ok_and(|u| {
            u.scheme() == "https" && u.path() == "/v1/reports" && u.host_str().is_some()
                && u.username().is_empty() && u.password().is_none() && u.query().is_none()
                && u.fragment().is_none()
        })
    })
}

pub fn choice(paths: &Paths) -> Option<bool> {
    let file = fs::File::open(paths.data.join("diagnostics-consent")).ok()?;
    let mut value = String::new();
    file.take(16).read_to_string(&mut value).ok()?;
    match value.as_str() {
        "enabled\n" => Some(true),
        "disabled\n" => Some(false),
        _ => None,
    }
}

pub fn enabled(paths: &Paths) -> bool {
    available() && choice(paths) == Some(true)
}

fn save_choice(paths: &Paths, enabled: bool) -> Result<(), String> {
    let _guard = SAVE_CHOICE.lock().map_err(|_| "Could not save diagnostics preference")?;
    fs::create_dir_all(&paths.data).map_err(|e| e.to_string())?;
    let part = paths.data.join(format!("diagnostics-consent.{}.part", std::process::id()));
    let value = if enabled { "enabled\n" } else { "disabled\n" };
    fs::write(&part, value).and_then(|_| fs::rename(part, paths.data.join("diagnostics-consent")))
        .map_err(|e| e.to_string())
}

pub fn set_choice(paths: &Paths, enabled: bool) -> Result<(), String> {
    if enabled && !available() { return Err("Diagnostics are unavailable in this build".into()); }
    save_choice(paths, enabled)?;
    if enabled {
        let paths = paths.clone();
        let _ = std::thread::spawn(move || start(&paths));
    } else {
        disable(paths);
    }
    Ok(())
}

fn process_start(pid: u32) -> Option<u64> {
    let mut system = sysinfo::System::new().ok()?;
    let pid = sysinfo::Pid::from_u32(pid);
    system.refresh_processes_specifics(sysinfo::ProcessesToUpdate::Some(&[pid]), true,
        sysinfo::ProcessRefreshKind::nothing());
    system.process(pid).map(sysinfo::Process::start_time)
}

fn marker(paths: &Paths) -> PathBuf {
    dir(paths).join(format!("active-{}.json", std::process::id()))
}

fn write_report(paths: &Paths, event: &'static str, detail: &'static str, fps: Option<u32>, version: &str) {
    if !enabled(paths) || !version_ok(version) { return; }
    let file = dir(paths).join(format!("{event}.json"));
    let report = Report { version, os: std::env::consts::OS, event, detail, fps };
    if let Ok(bytes) = serde_json::to_vec(&report) {
        let part = file.with_extension(format!("{}.part", std::process::id()));
        let _ = fs::create_dir_all(dir(paths));
        if fs::write(&part, bytes).is_ok() { let _ = fs::rename(part, file); }
    }
}

pub fn start(paths: &Paths) {
    if !enabled(paths) { return; }
    let Ok(_guard) = SEND.lock() else { return; };
    let directory = dir(paths);
    let _ = fs::create_dir_all(&directory);
    for file in fs::read_dir(&directory).into_iter().flatten().flatten() {
        let name = file.file_name();
        let Some(pid) = name.to_str().and_then(|n| n.strip_prefix("active-"))
            .and_then(|n| n.strip_suffix(".json")).and_then(|n| n.parse::<u32>().ok()) else { continue; };
        let Ok(bytes) = fs::read(file.path()) else { continue; };
        let Ok(session) = serde_json::from_slice::<Session>(&bytes) else { continue; };
        if session.pid != pid || process_start(pid) == Some(session.started) { continue; }
        write_report(paths, "launcher_crash", "unexpected_exit", None, &session.version);
        let _ = fs::remove_file(file.path());
    }
    let pid = std::process::id();
    if enabled(paths) && let Some(started) = process_start(pid) {
        let session = Session { pid, started, version: VERSION.into() };
        if let Ok(bytes) = serde_json::to_vec(&session) { let _ = fs::write(marker(paths), bytes); }
    }
    drop(_guard);
    flush(paths);
}

pub fn finish(paths: &Paths) {
    let _ = fs::remove_file(marker(paths));
}

fn disable(paths: &Paths) {
    for file in fs::read_dir(dir(paths)).into_iter().flatten().flatten() {
        let name = file.file_name();
        let Some(name) = name.to_str() else { continue; };
        if name.strip_prefix("active-").and_then(|n| n.strip_suffix(".json"))
            .is_some_and(|n| n.parse::<u32>().is_ok())
            || name.strip_suffix(".json").is_some_and(|n| QUEUE.contains(&n)) {
            let _ = fs::remove_file(file.path());
        }
    }
}

pub fn build_failed(paths: &Paths, stage: u32) {
    let detail = match stage {
        1 => "preflight", 2 => "exefs", 3 => "resources", 4 => "dlc",
        5 => "translate", 6 => "compile", 7 => "install", _ => "unknown",
    };
    write_report(paths, "build_failure", detail, None, VERSION);
    flush(paths);
}

fn game_failure(code: i32, log: &Path) -> &'static str {
    failure_kind(code, &crate::game::log_end(log, 256 * 1024).unwrap_or_default())
}

/// The kind of failure from the end of the game's log. The runtime writes a Vulkan error as its number
/// (`vk: upload submit failed (-4)`), and a lost device also as `vk: device lost` or `vk: device fault` lines
/// (engine `runtime/vk_device.c`); -1 and -2 are out of host and of device memory.
fn failure_kind(code: i32, text: &str) -> &'static str {
    let vulkan_failed = |n: &str| text.lines().any(|l| l.contains("vk: ") && l.trim_end().ends_with(n));
    if text.contains("vk: device lost") || text.contains("vk: device fault") || vulkan_failed("failed (-4)") {
        "device_lost"
    } else if vulkan_failed("failed (-1)") || vulkan_failed("failed (-2)") || text.contains("out of memory") {
        "out_of_memory"
    } else if code as u32 == 0xc0000005 {
        "access_violation"
    } else {
        "unexpected_exit"
    }
}

pub fn game_finished(paths: &Paths, code: i32, fps: Option<f64>, log: &Path) {
    if !enabled(paths) { return; }
    if code != 0 {
        write_report(paths, "game_crash", game_failure(code, log), None, VERSION);
    } else if let Some(value) = fps.filter(|f| f.is_finite() && (1.0..=300.0).contains(f)) {
        let band = ((value / 5.0).round() as u32 * 5).max(5);
        write_report(paths, "game_session", "completed", Some(band), VERSION);
    }
    flush(paths);
}

pub fn flush(paths: &Paths) {
    if !enabled(paths) { return; }
    let paths = paths.clone();
    let _ = std::thread::Builder::new().name("aod-diagnostics".into()).spawn(move || flush_now(&paths));
}

fn flush_now(paths: &Paths) {
    let Ok(_guard) = SEND.try_lock() else { return; };
    let Some(endpoint) = ENDPOINT else { return; };
    let Ok(client) = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5)).redirect(reqwest::redirect::Policy::none()).build() else { return; };
    for event in QUEUE {
        if !enabled(paths) { return; }
        let file = dir(paths).join(format!("{event}.json"));
        let Ok(bytes) = fs::read(&file) else { continue; };
        if bytes.len() > 1024 { continue; }
        // Reconstruct the wire fields before upload, even if another process edited the local queue.
        let Some(body) = wire_report(&bytes, event) else { continue; };
        if !enabled(paths) { return; }
        let accepted = client.post(endpoint).json(&body).send().is_ok_and(|r| r.status() == 204);
        if accepted && fs::read(&file).is_ok_and(|current| current == bytes) { let _ = fs::remove_file(file); }
    }
}

fn wire_report(bytes: &[u8], event: &str) -> Option<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let version = value["version"].as_str()?;
    if !version_ok(version) { return None; }
    let os = value["os"].as_str()?;
    if !matches!(os, "windows" | "linux") || value["event"].as_str()? != event { return None; }
    let detail = value["detail"].as_str()?;
    let fps = value["fps"].as_u64();
    match (event, detail, fps) {
        ("launcher_crash", "unexpected_exit", None)
        | ("game_crash", "device_lost" | "out_of_memory" | "access_violation" | "unexpected_exit", None)
        | ("build_failure", "preflight" | "exefs" | "resources" | "dlc" | "translate" | "compile" | "install" | "unknown", None) => {}
        ("game_session", "completed", Some(n)) if (5..=300).contains(&n) && n % 5 == 0 => {}
        _ => return None,
    }
    Some(serde_json::json!({ "version": version, "os": os, "event": event, "detail": detail, "fps": fps }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_filters_local_paths_and_rejects_free_text() {
        let queued = br#"{"version":"0.1.2","os":"windows","event":"game_crash","detail":"device_lost",
                          "fps":null,"log":"C:\\Users\\Tester\\game.nup"}"#;
        let wire = wire_report(queued, "game_crash").unwrap();
        assert_eq!(wire.as_object().unwrap().len(), 5);
        assert!(!wire.to_string().contains("Tester"));
        let invalid = br#"{"version":"0.1.2","os":"windows","event":"game_crash","detail":"C:\\Users\\Tester"}"#;
        assert!(wire_report(invalid, "game_crash").is_none());
    }

    #[test]
    fn a_failure_is_named_from_the_lines_the_game_writes() {
        // The end of a Radeon player's log: the runtime writes the Vulkan error as a number.
        let lost = "vk: GPU breadcrumbs: the GPU last reached the start of a part of submission 3\n\
                    vk: device lost: heap 0 usage 164 MiB of budget 15419 MiB (VMA blocks 128 MiB)\n\
                    vk: device fault: \"Device lost detected\" (0 address infos, 0 vendor infos, vendor binary 0 bytes)\n\
                    FATAL: vk: upload submit failed (-4)\n";
        assert_eq!(failure_kind(70, lost), "device_lost");
        assert_eq!(failure_kind(70, "FATAL: vk: vkQueueSubmit2 of value 12 failed (-4)\r\n"), "device_lost");
        assert_eq!(failure_kind(70, "FATAL: vk: staging buffer of 4096 bytes failed (-2)\n"), "out_of_memory");
        assert_eq!(failure_kind(70, "FATAL: vk: transient buffer of 64 bytes failed (-1)\n"), "out_of_memory");
        // A number that is not a Vulkan result, and a line that only looks like one.
        assert_eq!(failure_kind(70, "FATAL: vk: stencil clear shader failed (-3)\n"), "unexpected_exit");
        assert_eq!(failure_kind(70, "audio: open failed (-4)\n"), "unexpected_exit");
        assert_eq!(failure_kind(0xc0000005u32 as i32, ""), "access_violation");
        assert_eq!(failure_kind(71, "host: fault at 0x1234\n"), "unexpected_exit");
    }

    #[test]
    fn an_old_settings_snapshot_cannot_reenable_diagnostics() {
        let root = std::env::temp_dir().join(format!("aod-consent-test-{}", std::process::id()));
        let paths = Paths { exe_dir: root.clone(), data: root.clone(), engine: None };
        assert_eq!(choice(&paths), None);
        save_choice(&paths, true).unwrap();
        let old_config = crate::config::Config::load(&paths);
        save_choice(&paths, false).unwrap();
        old_config.save(&paths).unwrap();
        assert_eq!(choice(&paths), Some(false));
        fs::remove_dir_all(root).unwrap();
    }
}
