//! Age of Decomp launcher backend: Tauri commands for the Svelte UI, plus the headless entry points used by
//! `AgeOfDecomp --play` and `aod-cli` (RecompLauncher's `start` and `prepare`).

pub mod config;
mod diagnostics;
mod dlss;
pub mod game;
pub mod gamepad;
mod nintendo;
pub mod ini;
pub mod paths;
pub mod prepare;
pub mod settings;
pub mod system;
pub mod update;

use config::{Config, GameState, UiPrefs};
use paths::Paths;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

struct App {
    paths: Paths,
    cfg: Mutex<Config>,
    prepare: Arc<prepare::Control>,
    preparing: Mutex<bool>,
    pads: Mutex<Option<gamepad::Gamepads>>,
    game_running: Mutex<bool>,
    updating: Mutex<bool>,
}

#[derive(Serialize)]
struct LauncherState {
    version: &'static str,
    engine_dir: Option<String>,
    data_dir: String,
    game: GameState,
    ui: UiPrefs,
    system_locale: Locale,
    recomplauncher: bool,
    diagnostics_available: bool,
    diagnostics: Option<bool>,
}

#[derive(Serialize)]
struct Locale {
    language: String,
    country: String,
}

fn state_of(app: &App) -> LauncherState {
    let cfg = app.cfg.lock().unwrap().clone();
    let (language, country) = system::locale();
    LauncherState {
        version: VERSION,
        engine_dir: app.paths.engine.as_ref().map(|p| p.to_string_lossy().into_owned()),
        data_dir: app.paths.data.to_string_lossy().into_owned(),
        game: config::game_state(&cfg, &app.paths),
        ui: cfg.ui.clone(),
        system_locale: Locale { language, country },
        recomplauncher: std::env::var_os("RECOMPLAUNCHER").is_some(),
        diagnostics_available: diagnostics::available(),
        diagnostics: diagnostics::choice(&app.paths),
    }
}

type Res<T> = Result<T, String>;

#[tauri::command]
fn get_state(app: tauri::State<'_, App>) -> LauncherState {
    state_of(&app)
}

#[tauri::command]
fn set_ui(app: tauri::State<'_, App>, patch: serde_json::Value) -> Res<LauncherState> {
    {
        let mut cfg = app.cfg.lock().unwrap();
        let mut ui = serde_json::to_value(&cfg.ui).map_err(|e| e.to_string())?;
        if let (Some(dst), Some(src)) = (ui.as_object_mut(), patch.as_object()) {
            for (k, v) in src {
                dst.insert(k.clone(), v.clone());
            }
        }
        let mut next = cfg.clone();
        next.ui = serde_json::from_value(ui).map_err(|e| e.to_string())?;
        next.save(&app.paths)?;
        *cfg = next;
    }
    Ok(state_of(&app))
}

#[tauri::command]
fn set_diagnostics(app: tauri::State<'_, App>, enabled: bool) -> Res<LauncherState> {
    diagnostics::set_choice(&app.paths, enabled)?;
    Ok(state_of(&app))
}

#[tauri::command]
async fn check_updates() -> Res<update::UpdateInfo> {
    tauri::async_runtime::spawn_blocking(update::check).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn install_update(handle: tauri::AppHandle) -> Res<()> {
    {
        let app = handle.state::<App>();
        let mut updating = app.updating.lock().unwrap();
        if *updating || *app.preparing.lock().unwrap() || *app.game_running.lock().unwrap() {
            return Err("Close the game and wait for setup to finish before updating.".into());
        }
        *updating = true;
    }
    let events = handle.clone();
    let result = tauri::async_runtime::spawn_blocking(move || update::stage(|fraction| {
        let _ = events.emit("update-progress", fraction);
    })).await.map_err(|e| e.to_string()).and_then(|r| r);
    if result.is_ok() { handle.exit(0); }
    *handle.state::<App>().updating.lock().unwrap() = false;
    result
}

#[tauri::command]
async fn system_info() -> system::SystemInfo {
    tauri::async_runtime::spawn_blocking(system::system_info).await.unwrap_or_else(|_| system::system_info())
}

/// NVIDIA DLSS for DLAA in the standalone compiler (dlss.rs): is the player's file there, is there an RTX card.
#[tauri::command]
async fn dlss_status(app: tauri::State<'_, App>) -> Res<dlss::Status> {
    let paths = app.paths.clone();
    tauri::async_runtime::spawn_blocking(move || dlss::status(&paths, &system::system_info().gpus))
        .await.map_err(|e| e.to_string())
}

/// Downloads NVIDIA's file from NVIDIA's GitHub; the UI calls it only after the player accepted NVIDIA's licence.
/// Event `dlss-progress`: 0..1, at most once per percent.
#[tauri::command]
async fn dlss_download(app: tauri::State<'_, App>, handle: tauri::AppHandle) -> Res<dlss::Status> {
    let paths = app.paths.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut shown = -1i64;
        dlss::download(&paths, |p| {
            let percent = (p * 100.0) as i64;
            if percent != shown {
                shown = percent;
                let _ = handle.emit("dlss-progress", p);
            }
        })?;
        Ok(dlss::status(&paths, &system::system_info().gpus))
    }).await.map_err(|e| e.to_string())?
}

/// Opens NVIDIA's DLSS licence page (a fixed address) in the browser, the way open_folder opens folders.
#[tauri::command]
fn dlss_license() -> Res<()> {
    let opener = if cfg!(windows) { "explorer" } else { "xdg-open" };
    std::process::Command::new(opener).arg(dlss::LICENSE_URL).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

/// A copy the player picked: kept only when it is NVIDIA's (pinned, or signed by NVIDIA Corporation).
#[tauri::command]
async fn dlss_pick(app: tauri::State<'_, App>, path: String) -> Res<dlss::Status> {
    let paths = app.paths.clone();
    tauri::async_runtime::spawn_blocking(move || {
        dlss::pick(&paths, Path::new(&path))?;
        Ok(dlss::status(&paths, &system::system_info().gpus))
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn disk_info(path: String) -> system::DiskInfo {
    system::disk_info(&PathBuf::from(path))
}

#[tauri::command]
async fn find_nups(app: tauri::State<'_, App>) -> Res<Vec<String>> {
    let mut extra = vec![app.paths.exe_dir.clone()];
    if let Some(n) = app.cfg.lock().unwrap().nup.as_ref().and_then(|n| PathBuf::from(n).parent().map(PathBuf::from)) {
        extra.insert(0, n);
    }
    tauri::async_runtime::spawn_blocking(move || system::find_nups(&extra)).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn inspect_nup(path: String, update: Option<String>) -> Res<aod_nup::profile::Report> {
    tauri::async_runtime::spawn_blocking(move || {
        let nup = aod_nup::Nup::open(&path).map_err(|e| e.to_string())?;
        let upd = update.map(aod_nup::Nup::open).transpose().map_err(|e| e.to_string())?;
        aod_nup::profile::check(&nup, upd.as_ref(), &aod_nup::profile::GameProfile::hwaoi()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn set_game(app: tauri::State<'_, App>, nup: String, update: Option<String>, dlc: Option<String>, output: String) -> Res<LauncherState> {
    {
        let mut cfg = app.cfg.lock().unwrap();
        cfg.nup = Some(nup);
        cfg.update_nup = update;
        cfg.dlc_nup = dlc;
        cfg.output = Some(output);
        cfg.save(&app.paths)?;
    }
    Ok(state_of(&app))
}

#[tauri::command]
fn start_prepare(handle: tauri::AppHandle, app: tauri::State<'_, App>) -> Res<()> {
    let updating = app.updating.lock().unwrap();
    if *updating { return Err("An update is being installed".into()); }
    if *app.game_running.lock().unwrap() { return Err("Close the game before rebuilding".into()); }
    let cfg = app.cfg.lock().unwrap().clone();
    let resolution = settings::read(&cfg, &app.paths).get("output_res").cloned().unwrap_or_else(|| "1920x1080".into());
    let job = prepare::Job::from_config(&cfg, &app.paths, resolution)?;
    {
        let mut busy = app.preparing.lock().unwrap();
        if *busy {
            return Err("A build is already running".into());
        }
        *busy = true;
    }
    let paths = app.paths.clone();
    let control = app.prepare.clone();
    std::thread::spawn(move || {
        let h = handle.clone();
        let mut terminal = None;
        let result = prepare::run(&paths, &job, control, |e| {
            if matches!(e.kind, "done" | "error" | "cancelled") { terminal = Some(e); }
            else { let _ = h.emit("prepare", e); }
        });
        if let Some(app) = handle.try_state::<App>() {
            *app.preparing.lock().unwrap() = false;
        }
        if let Some(event) = terminal {
            let _ = h.emit("prepare", event);
        } else if let Err(error) = result {
            let _ = h.emit("prepare", serde_json::json!({ "kind": "error", "text": error }));
        }
    });
    Ok(())
}

#[tauri::command]
fn cancel_prepare(app: tauri::State<'_, App>) {
    prepare::cancel(&app.prepare);
}

#[tauri::command]
fn get_settings(app: tauri::State<'_, App>) -> BTreeMap<String, String> {
    let cfg = app.cfg.lock().unwrap().clone();
    settings::read(&cfg, &app.paths)
}

#[tauri::command]
fn set_setting(app: tauri::State<'_, App>, key: String, value: String) -> Res<BTreeMap<String, String>> {
    let cfg = app.cfg.lock().unwrap().clone();
    settings::write(&cfg, &app.paths, &key, &value)?;
    Ok(settings::read(&cfg, &app.paths))
}

#[tauri::command]
fn reset_settings(app: tauri::State<'_, App>, keys: Vec<String>) -> Res<BTreeMap<String, String>> {
    let cfg = app.cfg.lock().unwrap().clone();
    settings::reset(&cfg, &app.paths, &keys)?;
    Ok(settings::read(&cfg, &app.paths))
}

#[tauri::command]
fn list_pads(app: tauri::State<'_, App>) -> Vec<gamepad::PadInfo> {
    app.pads.lock().unwrap().as_ref().map(|p| p.list()).unwrap_or_default()
}

#[tauri::command]
fn pad_state(app: tauri::State<'_, App>, id: u32) -> Option<gamepad::PadState> {
    app.pads.lock().unwrap().as_ref().and_then(|p| p.state(id))
}

#[tauri::command]
fn pad_rumble(app: tauri::State<'_, App>, id: u32, low: f32, high: f32, ms: u32) -> bool {
    app.pads.lock().unwrap().as_ref().is_some_and(|p| p.rumble(id, low, high, ms.min(2000)))
}

#[tauri::command]
fn play(handle: tauri::AppHandle, app: tauri::State<'_, App>) -> Res<()> {
    let updating = app.updating.lock().unwrap();
    if *updating { return Err("An update is being installed".into()); }
    if *app.preparing.lock().unwrap() { return Err("Wait for the build to finish before playing".into()); }
    let mut running = app.game_running.lock().unwrap();
    if *running {
        return Err("The game is already running".into());
    }
    let cfg = app.cfg.lock().unwrap().clone();
    let bigpicture = std::env::var("RECOMPLAUNCHER_MODE").is_ok_and(|m| m == "bigpicture");
    let pads = app.pads.lock().unwrap().clone();
    let handoff = pads.as_ref().and_then(|p| p.suspend(true));
    let observe_exit = diagnostics::enabled(&app.paths);
    // The in-game Restart needs this process to wait for the game: not when it closes as the game starts.
    let restart = !(cfg.ui.close_on_play && !observe_exit);
    let launched = match game::launch(&cfg, &app.paths, bigpicture, handoff.as_ref(), restart, None, None) {
        Ok(l) => l,
        Err(e) => {
            if let Some(p) = &pads {
                let _ = p.suspend(false);
            }
            return Err(e.to_string());
        }
    };
    *running = true;
    let _ = handle.emit("game", serde_json::json!({ "kind": "started" }));
    if cfg.ui.close_on_play && !observe_exit {
        if let Some(handoff) = handoff {
            std::thread::spawn(move || {
                handoff.wait_for_game();
                handle.exit(0);
            });
        } else {
            handle.exit(0);
        }
        return Ok(());
    }
    if cfg.ui.close_on_play {
        // Keep observing child exits for players who opted into crash reports.
        if let Some(window) = handle.get_webview_window("main") { let _ = window.hide(); }
    }
    let close_on_play = cfg.ui.close_on_play;
    std::thread::spawn(move || {
        let mut launched = launched;
        let mut code = launched.child.wait().ok().and_then(|s| s.code());
        // Restart from the in-game menu: the game again with the settings it just wrote, the controllers handed over
        // as on Play.
        while launched.wants_restart(code) {
            let Some(app) = handle.try_state::<App>() else { break };
            let cfg = app.cfg.lock().unwrap().clone();
            let pads = app.pads.lock().unwrap().clone();
            if let Some(p) = &pads { let _ = p.suspend(false); }
            let handoff = pads.as_ref().and_then(|p| p.suspend(true));
            match game::launch(&cfg, &app.paths, bigpicture, handoff.as_ref(), true, None, None) {
                Ok(l) => {
                    launched = l;
                    code = launched.child.wait().ok().and_then(|s| s.code());
                }
                Err(e) => {
                    write_last_error(&app.paths, &e.to_string());
                    code = Some(e.exit_code());
                    break;
                }
            }
        }
        let fps = game::session_fps(&launched.log);
        if let Some(fps) = fps {
            let _ = handle.emit("game", serde_json::json!({ "kind": "perf", "fps": fps }));
        }
        let _ = handle.emit("game", serde_json::json!({ "kind": "exited", "code": code }));
        if let Some(app) = handle.try_state::<App>() {
            if let Some(code) = code { diagnostics::game_finished(&app.paths, code, fps, &launched.log); }
            *app.game_running.lock().unwrap() = false;
            if close_on_play { handle.exit(0); }
            else if let Some(p) = app.pads.lock().unwrap().as_ref() {
                let _ = p.suspend(false);
            }
        }
    });
    Ok(())
}

#[tauri::command]
fn open_folder(app: tauri::State<'_, App>, kind: String) -> Res<()> {
    let cfg = app.cfg.lock().unwrap().clone();
    let out = cfg.output(&app.paths);
    // Only Age of Decomp's own folders; the webview never opens arbitrary paths.
    let dir = match kind.as_str() {
        "game" => out,
        "saves" => paths::saves_dir(&out),
        "logs" => paths::game_logs_dir(&out),
        "data" => app.paths.data.clone(),
        "engine" => app.paths.engine.clone().ok_or("no engine")?,
        _ => return Err("unknown folder".into()),
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let opener = if cfg!(windows) { "explorer" } else { "xdg-open" };
    std::process::Command::new(opener).arg(&dir).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn log_tail(app: tauri::State<'_, App>, kind: String) -> String {
    let cfg = app.cfg.lock().unwrap().clone();
    let dir = if kind == "game" { paths::game_logs_dir(&cfg.output(&app.paths)) } else { app.paths.logs() };
    let newest = std::fs::read_dir(&dir)
        .ok()
        .and_then(|rd| rd.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "log")).max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok()));
    newest
        .and_then(|e| std::fs::read_to_string(e.path()).ok())
        .map(|t| t.lines().rev().take(200).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n"))
        .unwrap_or_default()
}

#[tauri::command]
fn licenses(app: tauri::State<'_, App>) -> String {
    let mut out = String::from(include_str!("../../../../THIRD_PARTY_NOTICES.md"));
    if let Some(e) = &app.paths.engine
        && let Ok(t) = std::fs::read_to_string(e.join("source").join("THIRD_PARTY_NOTICES.md"))
    {
        out.push_str("\n\n──────── Engine (HWAoI Recomp) ────────\n\n");
        out.push_str(&t);
    }
    out
}

pub fn run() {
    let paths = Paths::detect();
    let cfg = Config::load(&paths);
    // Start before the WebView is created so an early compiler crash is visible on restart.
    diagnostics::start(&paths);
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(App {
            paths: paths.clone(),
            cfg: Mutex::new(cfg),
            prepare: Arc::new(prepare::Control::default()),
            preparing: Mutex::new(false),
            pads: Mutex::new(None),
            game_running: Mutex::new(false),
            updating: Mutex::new(false),
        })
        .setup(move |tauri_app| {
            let handle = tauri_app.handle().clone();
            let (h1, h2) = (handle.clone(), handle.clone());
            let pads = gamepad::Gamepads::start(
                gamepad::sdl_candidates(paths.engine.as_deref(), &paths.exe_dir),
                paths.engine.clone(),
                move |list| {
                    let _ = h1.emit("pads", list);
                },
                move |button, down| {
                    // Only the focused launcher reacts to controller buttons.
                    let focused = h2.get_webview_window("main").and_then(|w| w.is_focused().ok()).unwrap_or(false);
                    if focused || !down {
                        let _ = h2.emit("pad-nav", serde_json::json!({ "button": button, "down": down }));
                    }
                },
            );
            *tauri_app.state::<App>().pads.lock().unwrap() = pads;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" && matches!(event, tauri::WindowEvent::Focused(false)) {
                for button in ["up", "down", "left", "right", "a", "b", "lb", "rb", "start"] {
                    let _ = window.emit("pad-nav", serde_json::json!({"button": button, "down": false}));
                }
            }
            if window.label() == "main" && matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                let app = window.state::<App>();
                if *app.preparing.lock().unwrap() {
                    prepare::cancel(&app.prepare);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_state, set_ui, set_diagnostics, system_info, disk_info, find_nups, inspect_nup, set_game, start_prepare, cancel_prepare,
            get_settings, set_setting, reset_settings, list_pads, pad_state, pad_rumble, play, open_folder, log_tail,
            licenses, check_updates, install_update, dlss_status, dlss_download, dlss_pick, dlss_license
        ])
        .build(tauri::generate_context!())
        .expect("error while building Age of Decomp")
        .run(|handle, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                diagnostics::finish(&handle.state::<App>().paths);
            }
        });
}

// ── headless entry points (RecompLauncher) ──────────────────────────────────────────────────────────────────────────

/// `AgeOfDecomp --play [--game-dir DIR] [--game=NUP] [--dlss=FILE]`: starts the built game without a window and returns
/// its exit code. `--game`: where the player's copy is now (RecompLauncher's `{input:nup}`), when it has moved since the
/// build. `--dlss`: the player's nvngx_dlss.dll (RecompLauncher's `{input:dlss}`) for DLAA.
pub fn play_headless(game_dir: Option<PathBuf>, copy: Option<PathBuf>, dlss: Option<PathBuf>) -> i32 {
    let paths = Paths::detect();
    let mut cfg = Config::load(&paths);
    if let Some(d) = game_dir {
        cfg.output = Some(d.to_string_lossy().into_owned());
    }
    let bigpicture = std::env::var("RECOMPLAUNCHER_MODE").is_ok_and(|m| m == "bigpicture");
    match game::launch(&cfg, &paths, bigpicture, None, true, copy.as_deref(), dlss.as_deref()) {
        Ok(mut l) => {
            diagnostics::flush(&paths);
            let mut code = l.child.wait().ok().and_then(|s| s.code());
            // Restart from the in-game menu: one process for RecompLauncher, the game started again inside it.
            while l.wants_restart(code) {
                match game::launch(&cfg, &paths, bigpicture, None, true, copy.as_deref(), dlss.as_deref()) {
                    Ok(next) => {
                        l = next;
                        code = l.child.wait().ok().and_then(|s| s.code());
                    }
                    Err(e) => {
                        play_failed(&paths, &cfg, &e.to_string());
                        return e.exit_code();
                    }
                }
            }
            let code = code.unwrap_or(1);
            if code != 0 { print_log_end(&l.log, code); }
            diagnostics::game_finished(&paths, code, game::session_fps(&l.log), &l.log);
            code
        }
        Err(e) => {
            play_failed(&paths, &cfg, &e.to_string());
            // Distinct code for "the game is not set up" (shown by RecompLauncher through exit_codes).
            e.exit_code()
        }
    }
}

/// Play could not start the game: the reason on stderr (RecompLauncher keeps what a game prints), in the launcher's
/// last-error.log, and as a log in the game folder's `logs`, where RecompLauncher finds the log it sends with a report.
fn play_failed(paths: &Paths, cfg: &Config, e: &str) {
    eprintln!("{e}");
    write_last_error(paths, e);
    let logs = paths::game_logs_dir(&cfg.output(paths));
    if std::fs::create_dir_all(&logs).is_ok() {
        let _ = std::fs::write(logs.join(format!("launch-{}.log", paths::stamp())),
                               format!("Age of Decomp {VERSION}: Play stopped before the game started.\n{e}\n"));
    }
}

/// The game ended with an error: the end of its log on stderr, so the reason (a "fatal:" line, a crash report) is in
/// what RecompLauncher keeps of the game's output too. The whole log stays in the game folder's `logs`.
fn print_log_end(log: &Path, code: i32) {
    let Ok(bytes) = std::fs::read(log) else { return };
    let tail = &bytes[bytes.len().saturating_sub(16 * 1024)..];
    let text = String::from_utf8_lossy(tail);
    let lines: Vec<&str> = text.lines().collect();
    eprintln!("Age of Decomp {VERSION}: the game ended with code {code}. The end of {}:", log.display());
    for line in &lines[lines.len().saturating_sub(60)..] {
        eprintln!("{line}");
    }
}

fn write_last_error(paths: &Paths, e: &str) {
    let _ = std::fs::create_dir_all(paths.logs());
    let _ = std::fs::write(paths.logs().join("last-error.log"), e);
}
