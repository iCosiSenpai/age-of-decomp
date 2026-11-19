//! Controllers through SDL 3 — the same library and HIDAPI drivers the game uses, so the launcher sees exactly the
//! pads the game will: Xbox, DualShock 4, DualSense, Switch Pro, Joy-Con (paired), Switch 2 Pro Controller, Joy-Con 2,
//! GameCube. SDL3 is loaded at run time from the engine bundle (no build-time dependency). All SDL calls happen on one
//! thread; the rest of the launcher talks to it through a channel. While the game runs, SDL is shut down so the game
//! gets the controllers to itself.

use libloading::Library;
use serde::Serialize;
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

#[repr(C)]
#[derive(Clone, Copy)]
struct SdlGuid {
    data: [u8; 16],
}

const SDL_INIT_JOYSTICK: u32 = 0x200;
const SDL_INIT_HAPTIC: u32 = 0x1000;
const SDL_INIT_GAMEPAD: u32 = 0x2000;
const SDL_INIT_EVENTS: u32 = 0x4000;
const SDL_INIT_SENSOR: u32 = 0x8000;
const SENSOR_ACCEL: c_int = 1;
const SENSOR_GYRO: c_int = 2;
const NINTENDO_VID: u16 = 0x057e;
/// Switch 2 controllers (Joy-Con 2 R/L, Pro Controller 2, NSO GameCube).
const SWITCH2_PIDS: [u16; 4] = [0x2066, 0x2067, 0x2069, 0x2073];

macro_rules! sdl_api {
    ($($name:ident : fn($($arg:ty),*) $(-> $ret:ty)?;)*) => {
        #[allow(non_snake_case)]
        struct Api { _lib: Library, $($name: unsafe extern "C" fn($($arg),*) $(-> $ret)?,)* }
        impl Api {
            #[allow(non_snake_case)]
            unsafe fn load(lib: Library) -> Result<Api, libloading::Error> {
                unsafe {
                    $(let $name = *lib.get::<unsafe extern "C" fn($($arg),*) $(-> $ret)?>(concat!(stringify!($name), "\0").as_bytes())?;)*
                    Ok(Api { _lib: lib, $($name,)* })
                }
            }
        }
    };
}

sdl_api! {
    SDL_SetHint: fn(*const c_char, *const c_char) -> bool;
    SDL_Init: fn(u32) -> bool;
    SDL_Quit: fn();
    SDL_PumpEvents: fn();
    SDL_UpdateGamepads: fn();
    SDL_GetGamepads: fn(*mut c_int) -> *mut u32;
    SDL_free: fn(*mut c_void);
    SDL_OpenGamepad: fn(u32) -> *mut c_void;
    SDL_CloseGamepad: fn(*mut c_void);
    SDL_GetGamepadName: fn(*mut c_void) -> *const c_char;
    SDL_GetGamepadType: fn(*mut c_void) -> c_int;
    SDL_GetGamepadVendor: fn(*mut c_void) -> u16;
    SDL_GetGamepadProduct: fn(*mut c_void) -> u16;
    SDL_GetGamepadGUIDForID: fn(u32) -> SdlGuid;
    SDL_GUIDToString: fn(SdlGuid, *mut c_char, c_int);
    SDL_GetGamepadButton: fn(*mut c_void, c_int) -> bool;
    SDL_GetGamepadAxis: fn(*mut c_void, c_int) -> i16;
    SDL_GamepadHasSensor: fn(*mut c_void, c_int) -> bool;
    SDL_SetGamepadSensorEnabled: fn(*mut c_void, c_int, bool) -> bool;
    SDL_GetGamepadSensorData: fn(*mut c_void, c_int, *mut f32, c_int) -> bool;
    SDL_RumbleGamepad: fn(*mut c_void, u16, u16, u32) -> bool;
    SDL_GetGamepadProperties: fn(*mut c_void) -> u32;
    SDL_GetBooleanProperty: fn(u32, *const c_char, bool) -> bool;
    SDL_GetGamepadPowerInfo: fn(*mut c_void, *mut c_int) -> c_int;
    SDL_GetGamepadConnectionState: fn(*mut c_void) -> c_int;
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct PadInfo {
    pub id: u32,
    pub guid: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub gen2: bool,
    pub has_gyro: bool,
    pub has_accel: bool,
    pub has_rumble: bool,
    pub has_trigger_rumble: bool,
    pub has_led: bool,
    pub battery: Option<i32>,
    pub wired: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PadState {
    pub buttons: HashMap<&'static str, bool>,
    pub axes: HashMap<&'static str, f32>,
    pub gyro: Option<[f32; 3]>,
    pub accel: Option<[f32; 3]>,
}

const BUTTONS: [(&str, c_int); 16] = [
    ("south", 0), ("east", 1), ("west", 2), ("north", 3), ("back", 4), ("guide", 5), ("start", 6),
    ("left_stick", 7), ("right_stick", 8), ("left_shoulder", 9), ("right_shoulder", 10),
    ("dpad_up", 11), ("dpad_down", 12), ("dpad_left", 13), ("dpad_right", 14), ("misc1", 15),
];
const AXES: [(&str, c_int); 6] =
    [("leftx", 0), ("lefty", 1), ("rightx", 2), ("righty", 3), ("left_trigger", 4), ("right_trigger", 5)];

fn kind_of(t: c_int) -> &'static str {
    match t {
        1 => "standard",
        2 => "xbox360",
        3 => "xboxone",
        4 => "ps3",
        5 => "ps4",
        6 => "ps5",
        7 => "switch_pro",
        8 => "joycon_left",
        9 => "joycon_right",
        10 => "joycon_pair",
        11 => "gamecube",
        _ => "unknown",
    }
}

enum Req {
    List(Sender<Vec<PadInfo>>),
    State(u32, Sender<Option<PadState>>),
    Rumble(u32, f32, f32, u32, Sender<bool>),
    Suspend(bool, Sender<Option<crate::nintendo::Handoff>>),
}

#[derive(Clone)]
pub struct Gamepads {
    tx: Sender<Req>,
}

impl Gamepads {
    /// Starts the SDL thread. `on_change` gets the pad list whenever it changes; `on_nav` gets logical launcher
    /// buttons (up/down/left/right/a/b/lb/rb/start, Nintendo pads confirm with A on the right) as they go down/up.
    pub fn start(
        candidates: Vec<PathBuf>,
        on_change: impl Fn(Vec<PadInfo>) + Send + 'static,
        on_nav: impl Fn(&'static str, bool) + Send + 'static,
    ) -> Option<Gamepads> {
        let (tx, rx) = channel::<Req>();
        let (ready_tx, ready_rx) = channel::<bool>();
        std::thread::Builder::new()
            .name("aod-sdl".into())
            .spawn(move || {
                // SDL opens libusb by name (wired Switch 2 pads on Windows), and Windows looks for it beside the
                // program, not beside SDL3.dll: load the copy next to SDL3.dll first so that name resolves to it.
                let _libusb: Vec<Library> = candidates
                    .iter()
                    .filter_map(|p| p.parent().map(|d| d.join("libusb-1.0.dll")))
                    .filter(|p| cfg!(windows) && p.is_file())
                    .take(1)
                    // SAFETY: loading the unmodified libusb DLL shipped with the engine; no symbols are called here.
                    .filter_map(|p| unsafe { Library::new(p).ok() })
                    .collect();
                let api = candidates.iter().find_map(|p| {
                    // SAFETY: loading SDL3; the symbols are the documented SDL 3 API with matching signatures.
                    unsafe { Library::new(p).ok().and_then(|l| Api::load(l).ok()) }
                });
                let Some(api) = api else {
                    let _ = ready_tx.send(false);
                    return;
                };
                let _ = ready_tx.send(true);
                let nintendo = crate::nintendo::Nintendo::load(&candidates);
                run(api, rx, on_change, on_nav, nintendo);
            })
            .ok()?;
        ready_rx.recv_timeout(Duration::from_secs(5)).ok().filter(|ok| *ok).map(|_| Gamepads { tx })
    }

    pub fn list(&self) -> Vec<PadInfo> {
        let (t, r) = channel();
        let _ = self.tx.send(Req::List(t));
        r.recv_timeout(Duration::from_secs(2)).unwrap_or_default()
    }
    pub fn state(&self, id: u32) -> Option<PadState> {
        let (t, r) = channel();
        let _ = self.tx.send(Req::State(id, t));
        r.recv_timeout(Duration::from_secs(1)).ok().flatten()
    }
    pub fn rumble(&self, id: u32, low: f32, high: f32, ms: u32) -> bool {
        let (t, r) = channel();
        let _ = self.tx.send(Req::Rumble(id, low, high, ms, t));
        r.recv_timeout(Duration::from_secs(1)).unwrap_or(false)
    }
    /// Transfers controller input to the game (true), or takes it back (false).
    pub fn suspend(&self, on: bool) -> Option<crate::nintendo::Handoff> {
        let (tx, rx) = channel();
        self.tx.send(Req::Suspend(on, tx)).ok()?;
        rx.recv_timeout(Duration::from_secs(4)).ok().flatten()
    }
}

struct Open {
    handle: *mut c_void,
    info: PadInfo,
    nintendo: bool,
}

fn hint(api: &Api, k: &str, v: &str) {
    let (k, v) = (CString::new(k).unwrap(), CString::new(v).unwrap());
    // SAFETY: valid C strings for the duration of the call.
    unsafe {
        (api.SDL_SetHint)(k.as_ptr(), v.as_ptr());
    }
}

fn init(api: &Api) -> bool {
    for (k, v) in [
        ("SDL_JOYSTICK_HIDAPI", "1"),
        ("SDL_JOYSTICK_HIDAPI_PS4", "1"),
        ("SDL_JOYSTICK_HIDAPI_PS5", "1"),
        ("SDL_JOYSTICK_HIDAPI_SWITCH", "1"),
        ("SDL_JOYSTICK_HIDAPI_SWITCH2", "1"),
        ("SDL_JOYSTICK_HIDAPI_JOY_CONS", "1"),
        ("SDL_JOYSTICK_HIDAPI_COMBINE_JOY_CONS", "1"),
        ("SDL_JOYSTICK_HIDAPI_GAMECUBE", "1"),
        ("SDL_HIDAPI_LIBUSB", "1"),
        ("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "0"),
        ("SDL_JOYSTICK_HIDAPI_SWITCH_HOME_LED", "0"),
    ] {
        hint(api, k, v);
    }
    // SAFETY: plain SDL initialisation on this thread.
    unsafe { (api.SDL_Init)(SDL_INIT_JOYSTICK | SDL_INIT_GAMEPAD | SDL_INIT_EVENTS | SDL_INIT_SENSOR | SDL_INIT_HAPTIC) }
}

fn describe(api: &Api, id: u32, h: *mut c_void) -> (PadInfo, bool) {
    let prop = |props: u32, name: &str| {
        let n = CString::new(name).unwrap();
        // SAFETY: valid properties id from SDL and a C string.
        unsafe { (api.SDL_GetBooleanProperty)(props, n.as_ptr(), false) }
    };
    // SAFETY: `h` is an open gamepad handle owned by this thread.
    unsafe {
        let name = {
            let p = (api.SDL_GetGamepadName)(h);
            if p.is_null() { "Controller".to_string() } else { CStr::from_ptr(p).to_string_lossy().into_owned() }
        };
        let mut buf = [0 as c_char; 33];
        (api.SDL_GUIDToString)((api.SDL_GetGamepadGUIDForID)(id), buf.as_mut_ptr(), 33);
        let guid = CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned();
        let kind = kind_of((api.SDL_GetGamepadType)(h));
        let vid = (api.SDL_GetGamepadVendor)(h);
        let pid = (api.SDL_GetGamepadProduct)(h);
        let gen2 = vid == NINTENDO_VID && SWITCH2_PIDS.contains(&pid);
        let props = (api.SDL_GetGamepadProperties)(h);
        let has_gyro = (api.SDL_GamepadHasSensor)(h, SENSOR_GYRO);
        let has_accel = (api.SDL_GamepadHasSensor)(h, SENSOR_ACCEL);
        if has_gyro {
            (api.SDL_SetGamepadSensorEnabled)(h, SENSOR_GYRO, true);
        }
        if has_accel {
            (api.SDL_SetGamepadSensorEnabled)(h, SENSOR_ACCEL, true);
        }
        let mut pct: c_int = -1;
        let power = (api.SDL_GetGamepadPowerInfo)(h, &mut pct);
        let wired = match (api.SDL_GetGamepadConnectionState)(h) {
            1 => Some(true),
            2 => Some(false),
            _ => None,
        };
        let nintendo = matches!(kind, "switch_pro" | "joycon_left" | "joycon_right" | "joycon_pair" | "gamecube")
            || vid == NINTENDO_VID;
        let info = PadInfo {
            id,
            guid,
            name,
            kind,
            gen2,
            has_gyro,
            has_accel,
            has_rumble: prop(props, "SDL.joystick.cap.rumble"),
            has_trigger_rumble: prop(props, "SDL.joystick.cap.trigger_rumble"),
            has_led: prop(props, "SDL.joystick.cap.rgb_led") || prop(props, "SDL.joystick.cap.player_led"),
            battery: (power >= 1 && pct >= 0).then_some(pct),
            wired,
        };
        (info, nintendo)
    }
}

fn run(api: Api, rx: Receiver<Req>, on_change: impl Fn(Vec<PadInfo>), on_nav: impl Fn(&'static str, bool),
       nintendo: Option<crate::nintendo::Nintendo>) {
    let mut active = init(&api);
    if active && let Some(bridge) = &nintendo { bridge.start(); }
    let mut open: Vec<Open> = Vec::new();
    let mut last_scan = Instant::now() - Duration::from_secs(10);
    let mut held: HashMap<&'static str, bool> = HashMap::new();
    let mut published: Vec<PadInfo> = Vec::new();

    loop {
        match rx.recv_timeout(Duration::from_millis(8)) {
            Ok(Req::List(t)) => {
                let _ = t.send(open.iter().map(|o| o.info.clone()).collect());
            }
            Ok(Req::State(id, t)) => {
                let st = open.iter().find(|o| o.info.id == id).map(|o| read_state(&api, o));
                let _ = t.send(st);
            }
            Ok(Req::Rumble(id, low, high, ms, t)) => {
                let ok = open.iter().find(|o| o.info.id == id).is_some_and(|o| {
                    let s = |v: f32| (v.clamp(0.0, 1.0) * 65535.0) as u16;
                    // SAFETY: open handle owned by this thread.
                    unsafe { (api.SDL_RumbleGamepad)(o.handle, s(low), s(high), ms) }
                });
                let _ = t.send(ok);
            }
            Ok(Req::Suspend(on, done)) => {
                let mut handoff = None;
                if on && active {
                    for o in open.drain(..) {
                        // SAFETY: closing handles we opened.
                        unsafe { (api.SDL_CloseGamepad)(o.handle) };
                    }
                    if let Some(bridge) = &nintendo {
                        handoff = bridge.lend();
                        if handoff.is_none() { bridge.stop(); }
                    }
                    // SAFETY: shuts SDL down on the thread that initialised it.
                    unsafe { (api.SDL_Quit)() };
                    active = false;
                    published.clear();
                    on_change(Vec::new());
                } else if !on && !active {
                    active = init(&api);
                    if active && let Some(bridge) = &nintendo { bridge.start(); }
                    last_scan = Instant::now() - Duration::from_secs(10);
                }
                let _ = done.send(handoff);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if !active {
            continue;
        }
        if let Some(bridge) = &nintendo { bridge.tick(); }
        // SAFETY: SDL event pump and gamepad update on the SDL thread.
        unsafe {
            (api.SDL_PumpEvents)();
            (api.SDL_UpdateGamepads)();
        }
        if last_scan.elapsed() > Duration::from_millis(500) {
            last_scan = Instant::now();
            rescan(&api, &mut open);
            let now: Vec<PadInfo> = open.iter().map(|o| o.info.clone()).collect();
            if now != published {
                published = now.clone();
                on_change(now);
            }
        }
        // Logical launcher buttons: any controller can drive the interface.
        let mut pressed: HashMap<&'static str, bool> = HashMap::new();
        for o in &open {
            let b = |i: c_int| unsafe { (api.SDL_GetGamepadButton)(o.handle, i) };
            let a = |i: c_int| unsafe { (api.SDL_GetGamepadAxis)(o.handle, i) } as f32 / 32767.0;
            let (confirm, back) = if o.nintendo { (1, 0) } else { (0, 1) };
            for (name, on) in [
                ("up", b(11) || a(1) < -0.6),
                ("down", b(12) || a(1) > 0.6),
                ("left", b(13) || a(0) < -0.6),
                ("right", b(14) || a(0) > 0.6),
                ("a", b(confirm)),
                ("b", b(back)),
                ("lb", b(9)),
                ("rb", b(10)),
                ("start", b(6)),
            ] {
                *pressed.entry(name).or_default() |= on;
            }
        }
        for name in ["up", "down", "left", "right", "a", "b", "lb", "rb", "start"] {
            let now = pressed.get(name).copied().unwrap_or(false);
            if held.get(name).copied().unwrap_or(false) != now {
                held.insert(name, now);
                on_nav(name, now);
            }
        }
    }
    if let Some(bridge) = &nintendo { bridge.stop(); }
    for o in open { unsafe { (api.SDL_CloseGamepad)(o.handle); } }
    if active { unsafe { (api.SDL_Quit)(); } }
}

fn rescan(api: &Api, open: &mut Vec<Open>) {
    let mut count: c_int = 0;
    // SAFETY: SDL returns an array of `count` ids that we free with SDL_free.
    let ids: Vec<u32> = unsafe {
        let p = (api.SDL_GetGamepads)(&mut count);
        if p.is_null() {
            Vec::new()
        } else {
            let v = std::slice::from_raw_parts(p, count.max(0) as usize).to_vec();
            (api.SDL_free)(p as *mut c_void);
            v
        }
    };
    open.retain(|o| {
        let keep = ids.contains(&o.info.id);
        if !keep {
            // SAFETY: closing a handle we opened.
            unsafe { (api.SDL_CloseGamepad)(o.handle) };
        }
        keep
    });
    for id in ids {
        if open.iter().any(|o| o.info.id == id) {
            continue;
        }
        // SAFETY: opening a gamepad SDL just listed.
        let h = unsafe { (api.SDL_OpenGamepad)(id) };
        if !h.is_null() {
            let (info, nintendo) = describe(api, id, h);
            open.push(Open { handle: h, info, nintendo });
        }
    }
    // Battery levels change slowly; refresh them with each scan.
    for o in open.iter_mut() {
        let mut pct: c_int = -1;
        // SAFETY: open handle.
        let power = unsafe { (api.SDL_GetGamepadPowerInfo)(o.handle, &mut pct) };
        o.info.battery = (power >= 1 && pct >= 0).then_some(pct);
    }
}

fn read_state(api: &Api, o: &Open) -> PadState {
    let h = o.handle;
    // SAFETY: open handle; sensor reads fill exactly 3 floats.
    unsafe {
        let buttons = BUTTONS.iter().map(|(n, i)| (*n, (api.SDL_GetGamepadButton)(h, *i))).collect();
        let axes = AXES.iter().map(|(n, i)| (*n, (api.SDL_GetGamepadAxis)(h, *i) as f32 / 32767.0)).collect();
        let sensor = |t: c_int, has: bool| {
            if !has {
                return None;
            }
            let mut v = [0f32; 3];
            (api.SDL_GetGamepadSensorData)(h, t, v.as_mut_ptr(), 3).then_some(v)
        };
        PadState { buttons, axes, gyro: sensor(SENSOR_GYRO, o.info.has_gyro), accel: sensor(SENSOR_ACCEL, o.info.has_accel) }
    }
}

/// Where SDL3 can be found: the engine bundle's deps, next to the program, then the system.
pub fn sdl_candidates(engine: Option<&std::path::Path>, exe_dir: &std::path::Path) -> Vec<PathBuf> {
    let name = if cfg!(windows) { "SDL3.dll" } else { "libSDL3.so.0" };
    let mut v = Vec::new();
    if let Some(e) = engine {
        v.push(e.join("deps").join("bin").join(name));
        v.push(e.join("deps").join("lib").join(name));
    }
    v.push(exe_dir.join(name));
    if cfg!(unix) {
        v.push(PathBuf::from(name));
    }
    v
}
