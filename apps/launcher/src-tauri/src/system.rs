//! What the setup's "This PC" step shows: CPU (AVX2), memory, graphics cards through Vulkan, free disk space, the
//! system language, and NUP files already on the PC.

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize)]
pub struct Gpu {
    pub index: u32,
    pub name: String,
    pub vendor: String,
    pub vram_gb: f64,
    pub discrete: bool,
    /// The card's own Vulkan version is 1.3 or later: what the game needs (the loader's version says nothing of it).
    pub vulkan13: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SystemInfo {
    pub os: String,
    pub cpu: String,
    pub threads: usize,
    pub avx2: bool,
    pub ram_gb: f64,
    pub gpus: Vec<Gpu>,
    pub vulkan: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiskInfo {
    pub path: String,
    pub free_gb: f64,
    pub total_gb: f64,
}

fn avx2() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        std::arch::is_x86_feature_detected!("avx2")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// Graphics cards in the order the runtime numbers them (Vulkan physical devices), with the loader's version.
fn vulkan() -> (Option<String>, Vec<Gpu>) {
    // SAFETY: loading the system Vulkan loader; every call follows the Vulkan spec and the instance is destroyed.
    unsafe {
        let Ok(entry) = ash::Entry::load() else { return (None, Vec::new()) };
        let version = entry.try_enumerate_instance_version().ok().flatten().unwrap_or(ash::vk::API_VERSION_1_0);
        let ver = format!(
            "{}.{}.{}",
            ash::vk::api_version_major(version),
            ash::vk::api_version_minor(version),
            ash::vk::api_version_patch(version)
        );
        let app = ash::vk::ApplicationInfo::default().api_version(ash::vk::API_VERSION_1_1);
        let info = ash::vk::InstanceCreateInfo::default().application_info(&app);
        let Ok(instance) = entry.create_instance(&info, None) else { return (Some(ver), Vec::new()) };
        let mut gpus = Vec::new();
        if let Ok(devs) = instance.enumerate_physical_devices() {
            for (i, d) in devs.iter().enumerate() {
                let props = instance.get_physical_device_properties(*d);
                let mem = instance.get_physical_device_memory_properties(*d);
                let vram: u64 = mem.memory_heaps[..mem.memory_heap_count as usize]
                    .iter()
                    .filter(|h| h.flags.contains(ash::vk::MemoryHeapFlags::DEVICE_LOCAL))
                    .map(|h| h.size)
                    .max()
                    .unwrap_or(0);
                let name = props.device_name_as_c_str().map(|c| c.to_string_lossy().into_owned()).unwrap_or_default();
                let vendor = match props.vendor_id {
                    0x10de => "NVIDIA",
                    0x1002 => "AMD",
                    0x8086 => "Intel",
                    _ => "Other",
                };
                gpus.push(Gpu {
                    index: i as u32,
                    name,
                    vendor: vendor.into(),
                    vram_gb: vram as f64 / (1u64 << 30) as f64,
                    discrete: props.device_type == ash::vk::PhysicalDeviceType::DISCRETE_GPU,
                    vulkan13: props.api_version >= ash::vk::API_VERSION_1_3,
                });
            }
        }
        instance.destroy_instance(None);
        (Some(ver), gpus)
    }
}

pub fn system_info() -> SystemInfo {
    // A system the library cannot read (it says so since 0.40) shows no processor name and no memory size: the page
    // then has nothing to say about them, and nothing else depends on the two.
    let (cpu, ram) = match sysinfo::System::new() {
        Ok(mut sys) => {
            sys.refresh_memory();
            sys.refresh_cpu_all();
            (sys.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default(), sys.total_memory())
        }
        Err(_) => (String::new(), 0),
    };
    let (vulkan, gpus) = vulkan();
    SystemInfo {
        os: format!(
            "{} {}",
            sysinfo::System::name().unwrap_or_default(),
            sysinfo::System::os_version().unwrap_or_default()
        )
        .trim()
        .to_string(),
        cpu,
        threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
        avx2: avx2(),
        ram_gb: ram as f64 / (1u64 << 30) as f64,
        gpus,
        vulkan,
    }
}

/// Whether a screen of this PC shows HDR now: what the game's HDR output needs (Windows turns it on per screen, and
/// the game then finds an HDR10 picture format for its window). `None` when the system does not say: every other
/// system, or a Windows that does not answer. Windows names it "advanced colour", which is on with HDR and also
/// with its automatic colour management alone, so `Some(false)` is the certain answer: no screen can take HDR.
pub fn hdr_screen() -> Option<bool> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Devices::Display::{
            DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig,
            DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO, DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO,
            DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO, QDC_ONLY_ACTIVE_PATHS,
        };
        // SAFETY: the two buffers have the sizes Windows asked for, the query writes at most that many entries and
        // says how many, and the info block carries its own size.
        unsafe {
            let (mut paths_n, mut modes_n) = (0u32, 0u32);
            if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut paths_n, &mut modes_n) != 0 {
                return None;
            }
            let mut paths = vec![std::mem::zeroed::<DISPLAYCONFIG_PATH_INFO>(); paths_n as usize];
            let mut modes = vec![std::mem::zeroed::<DISPLAYCONFIG_MODE_INFO>(); modes_n as usize];
            if QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut paths_n,
                paths.as_mut_ptr(),
                &mut modes_n,
                modes.as_mut_ptr(),
                std::ptr::null_mut(),
            ) != 0
            {
                return None;
            }
            let mut answer = None;
            for path in paths.iter().take(paths_n as usize) {
                let mut info: DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO = std::mem::zeroed();
                info.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO;
                info.header.size = std::mem::size_of::<DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO>() as u32;
                info.header.adapterId = path.targetInfo.adapterId;
                info.header.id = path.targetInfo.id;
                if DisplayConfigGetDeviceInfo(&mut info.header) == 0 {
                    // bit 1: advancedColorEnabled (bit 0 is "supported", the screen could if it were turned on)
                    let on = info.Anonymous.value & 0x2 != 0;
                    answer = Some(answer.unwrap_or(false) || on);
                }
            }
            answer
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}

pub fn disk_info(path: &Path) -> DiskInfo {
    // No list (the library says when it cannot read the disks, since 0.40) is like a folder on no known disk: 0 GB.
    let disks = sysinfo::Disks::new_with_refreshed_list().ok();
    let mut probe = path.to_path_buf();
    while !probe.exists() {
        match probe.parent() {
            Some(p) => probe = p.to_path_buf(),
            None => break,
        }
    }
    let probe = probe.canonicalize().unwrap_or(probe);
    let best = disks
        .iter()
        .flat_map(|list| list.iter())
        .filter(|d| on_disk(&probe, d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len());
    let gb = |b: u64| b as f64 / (1u64 << 30) as f64;
    DiskInfo {
        path: path.to_string_lossy().into_owned(),
        free_gb: best.map(|d| gb(d.available_space())).unwrap_or(0.0),
        total_gb: best.map(|d| gb(d.total_space())).unwrap_or(0.0),
    }
}

/// Whether `path` lies on the disk mounted at `mount`: by whole folder names (`/mnt/games` is not on `/mnt/game`),
/// without regard to case on Windows only.
fn on_disk(path: &Path, mount: &Path) -> bool {
    let plain = |p: &Path| {
        let text = p.to_string_lossy();
        let text = text.trim_start_matches(r"\\?\");
        PathBuf::from(if cfg!(windows) { text.to_ascii_lowercase() } else { text.to_string() })
    };
    plain(path).starts_with(plain(mount))
}

/// `{language, country}` from the desktop locale ("it-IT" → it / IT).
pub fn locale() -> (String, String) {
    let tag = sys_locale::get_locale().unwrap_or_else(|| "en-US".into());
    let mut parts = tag.split(['-', '_']);
    let lang = parts.next().unwrap_or("en").to_ascii_lowercase();
    let country = parts.find(|p| p.len() == 2).unwrap_or("").to_ascii_uppercase();
    (lang, country)
}

/// .nup files in the places players keep games: Downloads, Desktop, Documents, Games folders and the drive roots
/// (a few levels deep, bounded).
pub fn find_nups(extra: &[PathBuf]) -> Vec<String> {
    let mut roots: Vec<PathBuf> = extra.to_vec();
    if let Some(home) = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from) {
        for d in ["Downloads", "Desktop", "Documents", "Games", "Giochi"] {
            roots.push(home.join(d));
        }
    }
    if cfg!(windows) {
        for letter in b'C'..=b'Z' {
            let root = PathBuf::from(format!("{}:\\", letter as char));
            if root.exists() {
                roots.push(root);
            }
        }
    } else if let Some(user) = std::env::var_os("USER") {
        // SD cards and other drives (a Steam Deck mounts its card in /run/media/<user>/<label>)
        for base in ["/run/media", "/media"] {
            roots.push(Path::new(base).join(&user));
        }
    }
    let mut found = Vec::new();
    let mut visited = 0usize;
    for root in roots {
        walk(&root, 0, &mut found, &mut visited);
        if found.len() >= 20 || visited > 20_000 {
            break;
        }
    }
    found.sort();
    found.dedup();
    found
}

fn walk(dir: &Path, depth: u32, found: &mut Vec<String>, visited: &mut usize) {
    if depth > 3 || *visited > 20_000 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        if *visited > 20_000 || found.len() >= 20 {
            return;
        }
        *visited += 1;
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name.starts_with('$') || name.eq_ignore_ascii_case("Windows") || name.eq_ignore_ascii_case("node_modules") {
            continue;
        }
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_file() && name.to_ascii_lowercase().ends_with(".nup") {
            found.push(e.path().to_string_lossy().into_owned());
        } else if ft.is_dir() && !ft.is_symlink() {
            walk(&e.path(), depth + 1, found, visited);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `cargo test this_pc -- --ignored --nocapture` prints what Setup's "This PC" checks read here.
    #[test]
    #[ignore]
    fn this_pc() {
        println!("{:#?}", system_info());
    }

    #[test]
    fn a_folder_is_on_a_disk_by_whole_names() {
        if cfg!(windows) {
            assert!(on_disk(Path::new(r"\\?\C:\Games\AoD"), Path::new(r"C:\")));
            assert!(on_disk(Path::new(r"d:\games"), Path::new(r"D:\")));
            assert!(!on_disk(Path::new(r"D:\games"), Path::new(r"C:\")));
        } else {
            assert!(on_disk(Path::new("/mnt/game/build"), Path::new("/mnt/game")));
            assert!(!on_disk(Path::new("/mnt/games/build"), Path::new("/mnt/game")));
            assert!(!on_disk(Path::new("/mnt/Game/build"), Path::new("/mnt/game")));
            assert!(on_disk(Path::new("/home/player"), Path::new("/")));
        }
    }
}

#[cfg(test)]
mod hdr_tests {
    #[test]
    fn the_screens_hdr_state_is_read_without_a_fault() {
        // A PC with a screen answers yes or no; a session without one (a build server) may answer nothing.
        let first = super::hdr_screen();
        eprintln!("hdr_screen: {first:?}");
        assert_eq!(first, super::hdr_screen());
    }
}
