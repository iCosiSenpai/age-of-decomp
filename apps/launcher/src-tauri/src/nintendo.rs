//! Windows BLE transport bundled with Age of Decomp; virtual pads live only in our SDL instance.
use std::path::PathBuf;
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

pub struct Handoff {
    #[cfg(windows)]
    data: String,
    #[cfg(windows)]
    accepted: OwnedHandle,
}

impl Handoff {
    pub fn pass_to(&self, command: &mut std::process::Command) {
        #[cfg(windows)]
        command.env("AOD_NINTENDO_HANDOFF", &self.data);
        #[cfg(not(windows))]
        let _ = command;
    }

    pub fn wait_for_game(&self) {
        #[cfg(windows)]
        // SAFETY: the duplicated event is owned by this handoff for the duration of the wait.
        unsafe { windows_sys::Win32::System::Threading::WaitForSingleObject(self.accepted.as_raw_handle(), 8000); }
    }
}

#[cfg(windows)]
pub struct Nintendo {
    _lib: libloading::Library,
    helper: std::ffi::CString,
    start: unsafe extern "C" fn(*const std::ffi::c_char, bool) -> bool,
    tick: unsafe extern "C" fn(),
    stop: unsafe extern "C" fn(),
    lend: unsafe extern "C" fn(*mut usize) -> bool,
}
#[cfg(not(windows))]
pub struct Nintendo;

impl Nintendo {
    #[cfg(windows)]
    pub fn load(candidates: &[PathBuf]) -> Option<Self> {
        for candidate in candidates {
            for parent in candidate.ancestors().skip(1).take(4) {
                let folder = parent.join("nintendo");
                let helper = folder.join("AgeOfDecomp.Nintendo.exe");
                let library = folder.join("aod_nintendo.dll");
                if !helper.is_file() || !library.is_file() { continue; }
                // SAFETY: our bundled adapter and its fixed C ABI; the SDL DLL is already loaded.
                let loaded = unsafe {
                    (|| -> Option<Self> {
                        let lib = libloading::Library::new(library).ok()?;
                        Some(Self {
                            helper: std::ffi::CString::new(helper.to_str()?).ok()?,
                            start: *lib.get(b"aod_nintendo_start\0").ok()?,
                            tick: *lib.get(b"aod_nintendo_tick\0").ok()?,
                            stop: *lib.get(b"aod_nintendo_stop\0").ok()?,
                            lend: *lib.get(b"aod_nintendo_lend\0").ok()?,
                            _lib: lib,
                        })
                    })()
                };
                if loaded.is_some() { return loaded; }
            }
        }
        None
    }
    #[cfg(not(windows))]
    pub fn load(_candidates: &[PathBuf]) -> Option<Self> { None }

    #[cfg(windows)]
    pub fn start(&self) { unsafe { (self.start)(self.helper.as_ptr(), false); } }
    #[cfg(not(windows))]
    pub fn start(&self) {}
    #[cfg(windows)]
    pub fn tick(&self) { unsafe { (self.tick)(); } }
    #[cfg(not(windows))]
    pub fn tick(&self) {}
    #[cfg(windows)]
    pub fn lend(&self) -> Option<Handoff> {
        use windows_sys::Win32::Foundation::{DUPLICATE_SAME_ACCESS, DuplicateHandle};
        use windows_sys::Win32::System::Threading::GetCurrentProcess;

        let mut handles = [0; 3];
        // SAFETY: the adapter writes three machine-word handles. The SDL worker
        // keeps these handles open until the game exits or accepts ownership.
        unsafe {
            if !(self.lend)(handles.as_mut_ptr()) { return None; }
            let owner = GetCurrentProcess();
            let mut event = std::ptr::null_mut();
            if DuplicateHandle(owner, handles[2] as _, owner, &mut event, 0, 0, DUPLICATE_SAME_ACCESS) == 0 {
                return None;
            }
            Some(Handoff {
                data: format!("{}:{:x}:{:x}:{:x}", std::process::id(), handles[0], handles[1], handles[2]),
                accepted: OwnedHandle::from_raw_handle(event),
            })
        }
    }
    #[cfg(not(windows))]
    pub fn lend(&self) -> Option<Handoff> { None }
    #[cfg(windows)]
    pub fn stop(&self) { unsafe { (self.stop)(); } }
    #[cfg(not(windows))]
    pub fn stop(&self) {}
}
impl Drop for Nintendo { fn drop(&mut self) { self.stop(); } }
