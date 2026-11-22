//! NVIDIA DLSS (`temporal = dlss`: DLAA and DLSS Super Resolution) and frame generation (`frame_gen`) in the
//! standalone compiler. NVIDIA's `nvngx_dlss.dll` and `nvngx_dlssg.dll` are the player's own (NVIDIA RTX SDK licence)
//! and are never shipped with Age of Decomp. RecompLauncher fetches the first itself (the `nvidia.dlss` input, handed
//! to `--play --dlss=`); here the player either downloads NVIDIA's file from NVIDIA's own GitHub repository after
//! accepting NVIDIA's licence, or picks a copy signed by NVIDIA. Both end in `<data>/ngx`, where `game::launch` looks
//! for them.

use crate::paths::Paths;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The version RecompLauncher hands out too (recomplauncher.com/launcher/dlss.json), from NVIDIA's repository.
pub const VERSION: &str = "310.9.16";
pub const LICENSE_URL: &str = "https://github.com/NVIDIA/DLSS/blob/v310.9.16/LICENSE.txt";

/// NVIDIA's two files: DLSS itself, and frame generation (the same file the Streamline SDK 2.14.16 carries).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Dlss,
    FrameGen,
}

/// What each file is, where NVIDIA keeps it, the size and SHA-256 it must have, and the messages the UI shows (and
/// translates) when its download fails, the bytes are another file's, a download already runs or a picked file is
/// not this one.
struct Pinned {
    name: &'static str,
    url: &'static str,
    size: u64,
    sha256: &'static str,
    failed: &'static str,
    mismatch: &'static str,
    busy: &'static str,
    not_it: &'static str,
}

impl Kind {
    /// The name the UI and the commands use: `dlss` or `fg`.
    pub fn parse(s: &str) -> Option<Kind> {
        match s {
            "dlss" => Some(Kind::Dlss),
            "fg" => Some(Kind::FrameGen),
            _ => None,
        }
    }

    fn pinned(self) -> &'static Pinned {
        match self {
            Kind::Dlss => &Pinned {
                name: "nvngx_dlss.dll",
                url: "https://raw.githubusercontent.com/NVIDIA/DLSS/v310.9.16/lib/Windows_x86_64/rel/nvngx_dlss.dll",
                size: 58_956_912,
                sha256: "d00b5b0ab29c3ca101b70c3a5ecb3a01b7ef33766a3383a28a18623083e6123e",
                failed: "Could not download NVIDIA's DLSS file. The game runs without DLSS.",
                mismatch: "NVIDIA's DLSS file did not match the expected one, so it was not kept.",
                busy: "NVIDIA's DLSS file is already downloading.",
                not_it: "That is not NVIDIA's DLSS file (nvngx_dlss.dll).",
            },
            Kind::FrameGen => &Pinned {
                name: "nvngx_dlssg.dll",
                url: "https://raw.githubusercontent.com/NVIDIA/DLSS/v310.9.16/lib/Windows_x86_64/rel/nvngx_dlssg.dll",
                size: 7_506_544,
                sha256: "8d59d49b36410dd6b62fef2503a5871d347415bd85d943fa0848563a95dd50c7",
                failed: "Could not download NVIDIA's frame generation file. The game runs without frame generation.",
                mismatch: "NVIDIA's frame generation file did not match the expected one, so it was not kept.",
                busy: "NVIDIA's frame generation file is already downloading.",
                not_it: "That is not NVIDIA's frame generation file (nvngx_dlssg.dll).",
            },
        }
    }
}
#[cfg(windows)] // the Authenticode check exists only on Windows
const SIGNER: &str = "NVIDIA Corporation";
/// A picked file larger than this is not a DLSS library.
const PICK_MAX: u64 = 512 << 20;

#[derive(Clone, Debug, Serialize)]
pub struct Status {
    /// `<data>/ngx/nvngx_dlss.dll` is there.
    pub ready: bool,
    /// `<data>/ngx/nvngx_dlssg.dll` is there (frame generation).
    pub fg_ready: bool,
    /// An NVIDIA RTX card is in this PC (DLSS runs only there).
    pub rtx: bool,
    /// One of them is of a series frame generation runs on (RTX 40 and later).
    pub fg_card: bool,
    pub version: &'static str,
    pub license_url: &'static str,
}

/// Where the player's copy of one of the two files lives for the standalone compiler.
pub fn file(paths: &Paths, kind: Kind) -> PathBuf {
    paths.data.join("ngx").join(kind.pinned().name)
}

/// An NVIDIA card with the tensor cores DLSS needs: its name says RTX (GeForce, Quadro and A-series alike; GTX 16xx
/// cards are Turing without them).
pub fn rtx(vendor: &str, name: &str) -> bool {
    vendor.eq_ignore_ascii_case("NVIDIA") && name.to_ascii_lowercase().contains("rtx")
}

/// A card DLSS frame generation runs on: a GeForce RTX 40 or 50 (or a later series), or their workstation siblings
/// (RTX ... Ada Generation, RTX PRO ... Blackwell). Older RTX cards (20, 30, Quadro RTX, RTX A-series) have DLSS only.
/// The game itself asks the driver; this only decides what the launcher offers.
pub fn fg_card(vendor: &str, name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    if !rtx(vendor, &name) || name.contains("quadro") {
        return false;
    }
    if name.contains("ada generation") || name.contains("blackwell") {
        return true;
    }
    // the model number after "rtx": 4070 is the 40 series
    let model: String = name.split("rtx").nth(1).unwrap_or("").trim_start().chars().take_while(char::is_ascii_digit).collect();
    model.len() == 4 && model.parse::<u32>().is_ok_and(|n| n >= 4000)
}

pub fn status(paths: &Paths, gpus: &[crate::system::Gpu]) -> Status {
    Status {
        ready: cfg!(windows) && file(paths, Kind::Dlss).is_file(),
        fg_ready: cfg!(windows) && file(paths, Kind::FrameGen).is_file(),
        rtx: cfg!(windows) && gpus.iter().any(|g| rtx(&g.vendor, &g.name)),
        fg_card: cfg!(windows) && gpus.iter().any(|g| fg_card(&g.vendor, &g.name)),
        version: VERSION,
        license_url: LICENSE_URL,
    }
}

/// Downloads one of NVIDIA's files (the player accepted NVIDIA's licence in the UI) and keeps it only when its size
/// and SHA-256 are the pinned ones. `progress` gets 0..1.
pub fn download(paths: &Paths, kind: Kind, progress: impl FnMut(f64)) -> Result<(), String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    if !cfg!(windows) {
        return Err("NVIDIA DLSS is only used on Windows".into());
    }
    // One download of a file at a time (the page can be left and opened again while one runs): both would write the
    // same file.
    static RUNNING: [AtomicBool; 2] = [AtomicBool::new(false), AtomicBool::new(false)];
    struct Running(&'static AtomicBool);
    impl Drop for Running {
        fn drop(&mut self) {
            self.0.store(false, Ordering::SeqCst);
        }
    }
    let pin = kind.pinned();
    let running = &RUNNING[kind as usize];
    if running.swap(true, Ordering::SeqCst) {
        return Err(pin.busy.into());
    }
    let _running = Running(running);
    fetch_verified(pin.url, pin.size, pin.sha256, &file(paths, kind), pin.failed, pin.mismatch, progress)
}

/// `failed`: what the player reads when the download does not come through; `mismatch`: when it is another file.
fn fetch_verified(url: &str, size: u64, sha256: &str, dest: &Path, failed: &str, mismatch: &str,
                  mut progress: impl FnMut(f64)) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("AgeOfDecomp/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())?;
    let mut response = client.get(url).send().and_then(|r| r.error_for_status()).map_err(|_| failed.to_string())?;
    let dir = dest.parent().ok_or("no folder for the DLSS file")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    // A name of this process's own: the window and the setup step (aod-cli) can fetch the same file at once.
    let part = dest.with_extension(format!("dll.{}.part", std::process::id()));
    let result = (|| -> Result<(), String> {
        let mut out = std::fs::File::create(&part).map_err(|e| e.to_string())?;
        let mut hasher = Sha256::new();
        let mut count = 0u64;
        let mut buffer = [0u8; 65536];
        loop {
            let n = response.read(&mut buffer).map_err(|_| failed.to_string())?;
            if n == 0 {
                break;
            }
            count += n as u64;
            if count > size {
                return Err(failed.into());
            }
            out.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
            hasher.update(&buffer[..n]);
            progress(count as f64 / size.max(1) as f64);
        }
        out.sync_all().map_err(|e| e.to_string())?;
        if count != size || hex(&hasher.finalize()) != sha256 {
            return Err(mismatch.into());
        }
        Ok(())
    })();
    match result {
        Ok(()) => std::fs::rename(&part, dest).map_err(|e| e.to_string()),
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            Err(e)
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A copy the player already has (from a game or the DLSS SDK): kept when it is NVIDIA's pinned file, or carries a
/// valid Authenticode signature of NVIDIA Corporation.
pub fn pick(paths: &Paths, kind: Kind, source: &Path) -> Result<(), String> {
    if !cfg!(windows) {
        return Err("NVIDIA DLSS is only used on Windows".into());
    }
    let pin = kind.pinned();
    let meta = std::fs::metadata(source).map_err(|e| format!("{} cannot be read ({e})", source.display()))?;
    let named = source.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(pin.name));
    if !named || !meta.is_file() || meta.len() == 0 || meta.len() > PICK_MAX {
        return Err(pin.not_it.into());
    }
    let bytes = std::fs::read(source).map_err(|e| e.to_string())?;
    let pinned = bytes.len() as u64 == pin.size && hex(&Sha256::digest(&bytes)) == pin.sha256;
    let dest = file(paths, kind);
    std::fs::create_dir_all(dest.parent().ok_or("no folder for the DLSS file")?).map_err(|e| e.to_string())?;
    // Our own copy is what gets checked, never the picked file a second time: the bytes Windows calls signed are
    // then the bytes that go into place, whatever happens to the picked file meanwhile.
    let part = dest.with_extension(format!("{}.part.dll", std::process::id()));
    std::fs::write(&part, &bytes).map_err(|e| e.to_string())?;
    let result = if pinned || signed_by_nvidia(&part) {
        std::fs::rename(&part, &dest).map_err(|e| e.to_string())
    } else {
        Err("That file is not signed by NVIDIA, so it was not used.".into())
    };
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
}

/// Windows' own verdict on the file's Authenticode signature (Get-AuthenticodeSignature, WinVerifyTrust underneath):
/// valid, and the signing certificate names NVIDIA Corporation.
#[cfg(windows)]
fn signed_by_nvidia(path: &Path) -> bool {
    use std::os::windows::process::CommandExt;
    let literal = path.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$s = Get-AuthenticodeSignature -LiteralPath '{literal}'; \"$($s.Status)|$($s.SignerCertificate.Subject)\"");
    let Ok(out) = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .output()
    else {
        return false;
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let (status, subject) = text.trim().split_once('|').unwrap_or(("", ""));
    status == "Valid" && (subject.contains(&format!("O={SIGNER}")) || subject.contains(&format!("O=\"{SIGNER}\"")))
}

#[cfg(not(windows))]
fn signed_by_nvidia(_path: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /// A one-shot HTTP server on localhost that answers with `body`.
    fn serve(body: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                let mut request = [0u8; 2048];
                let _ = s.read(&mut request);
                let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = s.write_all(head.as_bytes());
                let _ = s.write_all(&body);
            }
        });
        format!("http://127.0.0.1:{port}/nvngx_dlss.dll")
    }

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("aod-dlss-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn download_keeps_only_the_pinned_file() {
        let body = b"not really a dll".to_vec();
        let digest = hex(&Sha256::digest(&body));
        let dir = temp("download");
        let dest = dir.join("ngx").join("nvngx_dlss.dll");
        let mut last = 0.0;
        fetch_verified(&serve(body.clone()), body.len() as u64, &digest, &dest, "failed", "mismatch", |p| last = p).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), body);
        assert_eq!(last, 1.0);
        std::fs::remove_file(&dest).unwrap();
        // Another hash, or another size: nothing is kept, not even the partial file.
        let other_hash = fetch_verified(&serve(body.clone()), body.len() as u64, &"0".repeat(64), &dest, "failed", "mismatch", |_| {});
        assert_eq!(other_hash.unwrap_err(), "mismatch");
        let other_size = fetch_verified(&serve(body.clone()), 4, &digest, &dest, "failed", "mismatch", |_| {});
        assert_eq!(other_size.unwrap_err(), "failed");
        assert_eq!(std::fs::read_dir(dest.parent().unwrap()).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn frame_generation_needs_the_40_series_or_later() {
        for name in ["NVIDIA GeForce RTX 5070 Ti", "NVIDIA GeForce RTX 4060 Laptop GPU", "NVIDIA GeForce RTX 4090",
                     "NVIDIA RTX 4000 Ada Generation", "NVIDIA RTX PRO 5000 Blackwell"] {
            assert!(fg_card("NVIDIA", name), "{name}");
        }
        for name in ["NVIDIA GeForce RTX 3080 Ti", "NVIDIA GeForce RTX 2060", "Quadro RTX 4000", "NVIDIA RTX A4000",
                     "NVIDIA GeForce GTX 1660 SUPER"] {
            assert!(!fg_card("NVIDIA", name), "{name}");
        }
        assert!(!fg_card("AMD", "AMD Radeon RX 9070 XT"));
    }

    #[test]
    fn only_rtx_cards_count() {
        assert!(rtx("NVIDIA", "NVIDIA GeForce RTX 5070 Ti"));
        assert!(rtx("NVIDIA", "NVIDIA RTX A2000"));
        assert!(!rtx("NVIDIA", "NVIDIA GeForce GTX 1660 SUPER"));
        assert!(!rtx("AMD", "AMD Radeon RX 9070 XT"));
    }

    /// Network: NVIDIA's real files from NVIDIA's GitHub, kept only with the pinned size and SHA-256 (into a temp
    /// folder).
    #[test]
    #[ignore]
    fn real_download_from_nvidia() {
        let dir = temp("real-download");
        let paths = Paths { exe_dir: dir.clone(), data: dir.join("data"), engine: None, portable: false };
        for kind in [Kind::FrameGen, Kind::Dlss] {
            let mut last = 0.0;
            download(&paths, kind, |p| last = p).unwrap();
            assert_eq!(std::fs::metadata(file(&paths, kind)).unwrap().len(), kind.pinned().size);
            assert_eq!(last, 1.0);
            assert!(signed_by_nvidia(&file(&paths, kind)));
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_two_files_have_their_own_names_and_pins() {
        assert_eq!(Kind::parse("dlss"), Some(Kind::Dlss));
        assert_eq!(Kind::parse("fg"), Some(Kind::FrameGen));
        assert_eq!(Kind::parse("other"), None);
        let (a, b) = (Kind::Dlss.pinned(), Kind::FrameGen.pinned());
        assert_eq!((a.name, b.name), ("nvngx_dlss.dll", "nvngx_dlssg.dll"));
        for pin in [a, b] {
            assert!(pin.url.starts_with("https://raw.githubusercontent.com/NVIDIA/") && pin.url.ends_with(pin.name));
            assert_eq!(pin.sha256.len(), 64);
        }
        assert_ne!(a.sha256, b.sha256);
        let paths = Paths { exe_dir: PathBuf::from("x"), data: PathBuf::from("data"), engine: None, portable: false };
        assert_eq!(file(&paths, Kind::FrameGen), PathBuf::from("data").join("ngx").join("nvngx_dlssg.dll"));
    }

    /// With AOD_TEST_NVNGX=<a real nvngx_dlss.dll> (not in the repository): NVIDIA's signed file is taken.
    #[test]
    #[ignore]
    fn a_picked_nvidia_file_is_kept() {
        let source = PathBuf::from(std::env::var("AOD_TEST_NVNGX").expect("AOD_TEST_NVNGX"));
        let dir = temp("pick-real");
        let paths = Paths { exe_dir: dir.clone(), data: dir.join("data"), engine: None, portable: false };
        pick(&paths, Kind::Dlss, &source).unwrap();
        assert_eq!(std::fs::metadata(file(&paths, Kind::Dlss)).unwrap().len(), std::fs::metadata(&source).unwrap().len());
        // A signed NVIDIA DLL under another name is refused, and so is this one as the frame generation file.
        let other = dir.join("sl.dlss.dll");
        std::fs::copy(&source, &other).unwrap();
        assert!(pick(&paths, Kind::Dlss, &other).is_err());
        assert!(pick(&paths, Kind::FrameGen, &source).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_picked_unsigned_file_is_refused() {
        let dir = temp("pick");
        std::fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("nvngx_dlss.dll");
        std::fs::write(&fake, b"MZ not signed").unwrap();
        let paths = Paths { exe_dir: dir.clone(), data: dir.join("data"), engine: None, portable: false };
        assert!(pick(&paths, Kind::Dlss, &fake).is_err());
        assert!(!file(&paths, Kind::Dlss).exists());
        // The same for the frame generation file, under its own name and under the other one.
        let fake_fg = dir.join("nvngx_dlssg.dll");
        std::fs::write(&fake_fg, b"MZ not signed").unwrap();
        assert!(pick(&paths, Kind::FrameGen, &fake_fg).is_err());
        assert_eq!(pick(&paths, Kind::FrameGen, &fake).unwrap_err(), "That is not NVIDIA's frame generation file (nvngx_dlssg.dll).");
        assert!(!file(&paths, Kind::FrameGen).exists());
        // The copy that was checked is gone too.
        assert_eq!(std::fs::read_dir(file(&paths, Kind::Dlss).parent().unwrap()).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(dir);
    }
}
