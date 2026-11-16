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
                });
            }
        }
        instance.destroy_instance(None);
        (Some(ver), gpus)
    }
}

pub fn system_info() -> SystemInfo {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();
    let cpu = sys.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default();
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
        ram_gb: sys.total_memory() as f64 / (1u64 << 30) as f64,
        gpus,
        vulkan,
    }
}

pub fn disk_info(path: &Path) -> DiskInfo {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut probe = path.to_path_buf();
    while !probe.exists() {
        match probe.parent() {
            Some(p) => probe = p.to_path_buf(),
            None => break,
        }
    }
    let probe = probe.canonicalize().unwrap_or(probe);
    let plain = |p: &Path| p.to_string_lossy().trim_start_matches(r"\\?\").to_ascii_lowercase();
    let target = plain(&probe);
    let best = disks
        .list()
        .iter()
        .filter(|d| target.starts_with(&plain(d.mount_point())))
        .max_by_key(|d| d.mount_point().as_os_str().len());
    let gb = |b: u64| b as f64 / (1u64 << 30) as f64;
    DiskInfo {
        path: path.to_string_lossy().into_owned(),
        free_gb: best.map(|d| gb(d.available_space())).unwrap_or(0.0),
        total_gb: best.map(|d| gb(d.total_space())).unwrap_or(0.0),
    }
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
