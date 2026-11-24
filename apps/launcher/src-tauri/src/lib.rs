//! Age of Decomp launcher backend: Tauri commands for the Svelte UI, plus the headless entry points used by
//! `AgeOfDecomp --play` and `AgeOfDecomp cli …` / `aod-cli` (RecompLauncher's `start` and `prepare`).

pub mod cli;
pub mod config;
mod diagnostics;
pub mod dlss;
pub mod game;
pub mod gamepad;
mod nintendo;
pub mod ini;
pub mod paths;
pub mod prepare;
pub mod saves;
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
    /// A screen of this PC shows HDR now (system::hdr_screen); `None` when the system does not say. Read with the
    /// rest each time the window comes back, so turning HDR on in the system's settings shows at once.
    hdr_screen: Option<bool>,
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
        hdr_screen: system::hdr_screen(),
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

/// NVIDIA DLSS and frame generation in the standalone compiler (dlss.rs): are the player's files there, is there an
/// RTX card.
#[tauri::command]
async fn dlss_status(app: tauri::State<'_, App>) -> Res<dlss::Status> {
    let paths = app.paths.clone();
    tauri::async_runtime::spawn_blocking(move || dlss::status(&paths, &system::system_info().gpus))
        .await.map_err(|e| e.to_string())
}

/// Which of NVIDIA's two files a command is about: `dlss` (also when the UI names none) or `fg`, frame generation.
fn dlss_kind(kind: Option<&str>) -> Res<dlss::Kind> {
    dlss::Kind::parse(kind.unwrap_or("dlss")).ok_or_else(|| "That NVIDIA file is not known.".to_string())
}

/// Downloads one of NVIDIA's files from NVIDIA's GitHub; the UI calls it only after the player accepted NVIDIA's
/// licence. Event `dlss-progress` (`fg-progress` for the frame generation file): 0..1, at most once per percent.
#[tauri::command]
async fn dlss_download(app: tauri::State<'_, App>, handle: tauri::AppHandle, kind: Option<String>) -> Res<dlss::Status> {
    let paths = app.paths.clone();
    let kind = dlss_kind(kind.as_deref())?;
    let event = if kind == dlss::Kind::FrameGen { "fg-progress" } else { "dlss-progress" };
    tauri::async_runtime::spawn_blocking(move || {
        let mut shown = -1i64;
        dlss::download(&paths, kind, |p| {
            let percent = (p * 100.0) as i64;
            if percent != shown {
                shown = percent;
                let _ = handle.emit(event, p);
            }
        })?;
        Ok(dlss::status(&paths, &system::system_info().gpus))
    }).await.map_err(|e| e.to_string())?
}

/// Opens NVIDIA's DLSS licence page (a fixed address) in the browser, the way open_folder opens folders.
#[tauri::command]
fn dlss_license() -> Res<()> {
    open_with_system(dlss::LICENSE_URL.as_ref())
}

/// Starts the system's opener (Explorer, xdg-open) on a folder or a web address. A thread waits for it, so on Linux
/// an opener that has finished is not left behind as a zombie until the launcher closes.
fn open_with_system(target: &std::ffi::OsStr) -> Res<()> {
    let opener = if cfg!(windows) { "explorer" } else { "xdg-open" };
    let mut child = std::process::Command::new(opener).arg(target).spawn().map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// A copy the player picked: kept only when it is NVIDIA's (pinned, or signed by NVIDIA Corporation).
#[tauri::command]
async fn dlss_pick(app: tauri::State<'_, App>, path: String, kind: Option<String>) -> Res<dlss::Status> {
    let paths = app.paths.clone();
    let kind = dlss_kind(kind.as_deref())?;
    tauri::async_runtime::spawn_blocking(move || {
        dlss::pick(&paths, kind, Path::new(&path))?;
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
        // Saved first: a choice that could not be written does not become the live one either.
        let mut cfg = app.cfg.lock().unwrap();
        let mut next = cfg.clone();
        next.nup = Some(nup);
        next.update_nup = update;
        next.dlc_nup = dlc;
        next.set_output(output);
        next.save(&app.paths)?;
        *cfg = next;
    }
    Ok(state_of(&app))
}

#[tauri::command]
fn start_prepare(handle: tauri::AppHandle, app: tauri::State<'_, App>) -> Res<()> {
    let updating = app.updating.lock().unwrap();
    if *updating { return Err("An update is being installed".into()); }
    if *app.game_running.lock().unwrap() { return Err("Close the game before rebuilding".into()); }
    let cfg = app.cfg.lock().unwrap().clone();
    // Unset is Automatic, the default: the game then takes the screen's resolution when it starts.
    let resolution = settings::read(&cfg, &app.paths).get("output_res").cloned().unwrap_or_else(|| "auto".into());
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
    // A Cancel pressed from here on stops this build: the flag is cleared here, never later in the worker.
    control.cancel.store(false, std::sync::atomic::Ordering::SeqCst);
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
        let mut code = launched.child.wait().ok().map(game::exit_code);
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
                    code = launched.child.wait().ok().map(game::exit_code);
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
            if close_on_play { handle.exit(0); }
            // The controllers come back to the launcher before Play is free again: a Play pressed right after must
            // not hand them to a new game only to have this one take them back.
            else if let Some(p) = app.pads.lock().unwrap().as_ref() {
                let _ = p.suspend(false);
            }
            *app.game_running.lock().unwrap() = false;
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
    open_with_system(dir.as_os_str())
}

#[tauri::command]
fn licenses(app: tauri::State<'_, App>) -> String {
    let mut out = String::from(include_str!("../../../../THIRD_PARTY_NOTICES.md"));
    if let Some(e) = &app.paths.engine
        && let Ok(t) = std::fs::read_to_string(e.join("source").join("THIRD_PARTY_NOTICES.md"))
    {
        out.push_str("\n\n──────── Engine: where it comes from and what it builds on ────────\n\n");
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
            let (h1, h2, h3) = (handle.clone(), handle.clone(), handle.clone());
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
                move |id| {
                    // The pad the player has just started using: the launcher's button hints follow it.
                    let focused = h3.get_webview_window("main").and_then(|w| w.is_focused().ok()).unwrap_or(false);
                    if focused {
                        let _ = h3.emit("pad-active", id);
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
            get_settings, set_setting, reset_settings, list_pads, pad_state, pad_rumble, play, open_folder,
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
/// build. `--dlss`: the player's nvngx_dlss.dll (RecompLauncher's `{input:dlss}`) for NVIDIA DLSS.
/// The program's arguments as text. One that is not UTF-8 (a Linux file name in another encoding) is refused with its
/// readable part, instead of the panic `std::env::args` ends in; the caller exits with 2, the bad-arguments code.
pub fn args() -> Result<Vec<String>, String> {
    std::env::args_os().skip(1).map(|a| a.into_string().map_err(|bad| {
        format!("An argument is not valid UTF-8 text: {} (rename the file, or move it to a folder whose name is \
                 plain text)", bad.to_string_lossy())
    })).collect()
}

/// The starts both programs run without the launcher's window: `cli …` (RecompLauncher's setup step and the other
/// commands) and `--play` (RecompLauncher's `start`). Some(exit code), or None when the window is asked for. aod-cli
/// takes them too, so the Linux AppImage's AppRun sends both to it: a PC without WebKitGTK still sets up and plays.
pub fn headless(args: &[String]) -> Option<i32> {
    if args.first().is_some_and(|a| a == "cli") {
        return Some(cli::run(args.to_vec()));
    }
    if !args.iter().any(|a| a == "--play") {
        return None;
    }
    // `--name=<value>` or `--name <value>`; an empty value (RecompLauncher's {input:…} without a file) is none.
    let value = |name: &str| {
        args.iter().find_map(|a| a.strip_prefix(&format!("{name}=")).map(str::to_string))
            .or_else(|| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned())
            .filter(|v| !v.is_empty()).map(PathBuf::from)
    };
    // The game folder, the player's copy (RecompLauncher's {input:nup}) and their NVIDIA DLSS file (its
    // {input:dlss}, only on RTX PCs).
    Some(play_headless(value("--game-dir"), value("--game"), value("--dlss")))
}

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
            let mut code = l.child.wait().ok().map(game::exit_code);
            // Restart from the in-game menu: one process for RecompLauncher, the game started again inside it.
            while l.wants_restart(code) {
                match game::launch(&cfg, &paths, bigpicture, None, true, copy.as_deref(), dlss.as_deref()) {
                    Ok(next) => {
                        l = next;
                        code = l.child.wait().ok().map(game::exit_code);
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
    let Some(text) = game::log_end(log, 16 * 1024) else { return };
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
