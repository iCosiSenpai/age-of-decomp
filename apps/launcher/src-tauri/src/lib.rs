//! Age of Decomp launcher backend: Tauri commands for the Svelte UI, plus the headless entry points used by
//! `AgeOfDecomp --play` and `aod-cli` (RecompLauncher's `start` and `prepare`).

pub mod config;
pub mod game;
pub mod gamepad;
pub mod ini;
pub mod paths;
pub mod prepare;
pub mod settings;
pub mod system;

use config::{Config, GameState, UiPrefs};
use paths::Paths;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
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
        cfg.ui = serde_json::from_value(ui).map_err(|e| e.to_string())?;
        cfg.save(&app.paths)?;
    }
    Ok(state_of(&app))
}

#[tauri::command]
async fn system_info() -> system::SystemInfo {
    tauri::async_runtime::spawn_blocking(system::system_info).await.unwrap_or_else(|_| system::system_info())
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
    {
        let mut busy = app.preparing.lock().unwrap();
        if *busy {
            return Err("A build is already running".into());
        }
        *busy = true;
    }
    let cfg = app.cfg.lock().unwrap().clone();
    let resolution = settings::read(&cfg, &app.paths).get("output_res").cloned().unwrap_or_else(|| "1920x1080".into());
    let job = prepare::Job::from_config(&cfg, &app.paths, resolution)?;
    let paths = app.paths.clone();
    let control = app.prepare.clone();
    std::thread::spawn(move || {
        let h = handle.clone();
        let result = prepare::run(&paths, &job, control, move |e| {
            let _ = h.emit("prepare", e);
        });
        if result.is_ok() {
            let _ = settings::merge_pending(&cfg, &paths);
        }
        if let Some(app) = handle.try_state::<App>() {
            *app.preparing.lock().unwrap() = false;
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
    let mut running = app.game_running.lock().unwrap();
    if *running {
        return Err("The game is already running".into());
    }
    let cfg = app.cfg.lock().unwrap().clone();
    let bigpicture = std::env::var("RECOMPLAUNCHER_MODE").is_ok_and(|m| m == "bigpicture");
    let pads = app.pads.lock().unwrap().clone();
    if let Some(p) = &pads {
        p.suspend(true); // the game gets the controllers to itself
    }
    let launched = match game::launch(&cfg, &app.paths, bigpicture) {
        Ok(l) => l,
        Err(e) => {
            if let Some(p) = &pads {
                p.suspend(false);
            }
            return Err(e);
        }
    };
    *running = true;
    let _ = handle.emit("game", serde_json::json!({ "kind": "started" }));
    if cfg.ui.close_on_play {
        handle.exit(0);
        return Ok(());
    }
    std::thread::spawn(move || {
        let mut child = launched.child;
        let code = child.wait().ok().and_then(|s| s.code());
        if let Some(fps) = game::session_fps(&launched.log) {
            let _ = handle.emit("game", serde_json::json!({ "kind": "perf", "fps": fps }));
        }
        let _ = handle.emit("game", serde_json::json!({ "kind": "exited", "code": code }));
        if let Some(app) = handle.try_state::<App>() {
            *app.game_running.lock().unwrap() = false;
            if let Some(p) = app.pads.lock().unwrap().as_ref() {
                p.suspend(false);
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
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(App {
            paths: paths.clone(),
            cfg: Mutex::new(cfg),
            prepare: Arc::new(prepare::Control::default()),
            preparing: Mutex::new(false),
            pads: Mutex::new(None),
            game_running: Mutex::new(false),
        })
        .setup(move |tauri_app| {
            let handle = tauri_app.handle().clone();
            let (h1, h2) = (handle.clone(), handle.clone());
            let pads = gamepad::Gamepads::start(
                gamepad::sdl_candidates(paths.engine.as_deref(), &paths.exe_dir),
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
        .invoke_handler(tauri::generate_handler![
            get_state, set_ui, system_info, disk_info, find_nups, inspect_nup, set_game, start_prepare, cancel_prepare,
            get_settings, set_setting, reset_settings, list_pads, pad_state, pad_rumble, play, open_folder, log_tail,
            licenses
        ])
        .run(tauri::generate_context!())
        .expect("error while running Age of Decomp");
}

// ── headless entry points (RecompLauncher) ──────────────────────────────────────────────────────────────────────────

/// `AgeOfDecomp --play [--game-dir DIR]`: starts the built game without a window and returns its exit code.
pub fn play_headless(game_dir: Option<PathBuf>) -> i32 {
    let paths = Paths::detect();
    let mut cfg = Config::load(&paths);
    if let Some(d) = game_dir {
        cfg.output = Some(d.to_string_lossy().into_owned());
    }
    let bigpicture = std::env::var("RECOMPLAUNCHER_MODE").is_ok_and(|m| m == "bigpicture");
    match game::launch(&cfg, &paths, bigpicture) {
        Ok(mut l) => l.child.wait().ok().and_then(|s| s.code()).unwrap_or(1),
        Err(e) => {
            eprintln!("{e}");
            write_last_error(&paths, &e);
            // Distinct code for "the game is not set up" (shown by RecompLauncher through exit_codes).
            if e.contains("not built") || e.contains("incomplete") { 64 } else if e.contains("game file") { 65 } else { 1 }
        }
    }
}

fn write_last_error(paths: &Paths, e: &str) {
    let _ = std::fs::create_dir_all(paths.logs());
    let _ = std::fs::write(paths.logs().join("last-error.log"), e);
}
