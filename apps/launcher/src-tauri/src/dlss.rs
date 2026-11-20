//! NVIDIA DLSS for DLAA (`temporal = dlaa`) in the standalone compiler. NVIDIA's `nvngx_dlss.dll` is the player's
//! own (NVIDIA RTX SDK licence) and is never shipped with Age of Decomp. RecompLauncher fetches it itself (the
//! `nvidia.dlss` input, handed to `--play --dlss=`); here the player either downloads NVIDIA's file from NVIDIA's own
//! GitHub repository after accepting NVIDIA's licence, or picks a copy signed by NVIDIA. Both end in `<data>/ngx`,
//! where `game::launch` looks for it.

use crate::paths::Paths;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The version RecompLauncher hands out too (recomplauncher.com/launcher/dlss.json), from NVIDIA's repository.
pub const VERSION: &str = "310.9.16";
const URL: &str = "https://raw.githubusercontent.com/NVIDIA/DLSS/v310.9.16/lib/Windows_x86_64/rel/nvngx_dlss.dll";
pub const LICENSE_URL: &str = "https://github.com/NVIDIA/DLSS/blob/v310.9.16/LICENSE.txt";
const SIZE: u64 = 58_956_912;
const SHA256: &str = "d00b5b0ab29c3ca101b70c3a5ecb3a01b7ef33766a3383a28a18623083e6123e";
const SIGNER: &str = "NVIDIA Corporation";
/// A picked file larger than this is not a DLSS library.
const PICK_MAX: u64 = 512 << 20;

#[derive(Clone, Debug, Serialize)]
pub struct Status {
    /// `<data>/ngx/nvngx_dlss.dll` is there.
    pub ready: bool,
    /// An NVIDIA RTX card is in this PC (DLAA runs only there).
    pub rtx: bool,
    pub version: &'static str,
    pub license_url: &'static str,
}

/// Where the player's copy lives for the standalone compiler.
pub fn file(paths: &Paths) -> PathBuf {
    paths.data.join("ngx").join("nvngx_dlss.dll")
}

/// An NVIDIA card with the tensor cores DLSS needs: its name says RTX (GeForce, Quadro and A-series alike; GTX 16xx
/// cards are Turing without them).
pub fn rtx(vendor: &str, name: &str) -> bool {
    vendor.eq_ignore_ascii_case("NVIDIA") && name.to_ascii_lowercase().contains("rtx")
}

pub fn status(paths: &Paths, gpus: &[crate::system::Gpu]) -> Status {
    Status {
        ready: cfg!(windows) && file(paths).is_file(),
        rtx: cfg!(windows) && gpus.iter().any(|g| rtx(&g.vendor, &g.name)),
        version: VERSION,
        license_url: LICENSE_URL,
    }
}

/// Downloads NVIDIA's file (the player accepted NVIDIA's licence in the UI) and keeps it only when its size and
/// SHA-256 are the pinned ones. `progress` gets 0..1.
pub fn download(paths: &Paths, progress: impl FnMut(f64)) -> Result<(), String> {
    if !cfg!(windows) {
        return Err("NVIDIA DLSS is only used on Windows".into());
    }
    // One download at a time (the page can be left and opened again while one runs): both would write the same file.
    static RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    struct Running;
    impl Drop for Running {
        fn drop(&mut self) {
            RUNNING.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
    if RUNNING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err("NVIDIA's DLSS file is already downloading.".into());
    }
    let _running = Running;
    fetch_verified(URL, SIZE, SHA256, &file(paths), progress)
}

fn fetch_verified(url: &str, size: u64, sha256: &str, dest: &Path, mut progress: impl FnMut(f64)) -> Result<(), String> {
    const FAILED: &str = "Could not download NVIDIA's DLSS file. The game runs without DLAA.";
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("AgeOfDecomp/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())?;
    let mut response = client.get(url).send().and_then(|r| r.error_for_status()).map_err(|_| FAILED)?;
    let dir = dest.parent().ok_or("no folder for the DLSS file")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let part = dest.with_extension("dll.part");
    let result = (|| -> Result<(), String> {
        let mut out = std::fs::File::create(&part).map_err(|e| e.to_string())?;
        let mut hasher = Sha256::new();
        let mut count = 0u64;
        let mut buffer = [0u8; 65536];
        loop {
            let n = response.read(&mut buffer).map_err(|_| FAILED)?;
            if n == 0 {
                break;
            }
            count += n as u64;
            if count > size {
                return Err(FAILED.into());
            }
            out.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
            hasher.update(&buffer[..n]);
            progress(count as f64 / size.max(1) as f64);
        }
        out.sync_all().map_err(|e| e.to_string())?;
        if count != size || hex(&hasher.finalize()) != sha256 {
            return Err("NVIDIA's DLSS file did not match the expected one, so it was not kept.".into());
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
pub fn pick(paths: &Paths, source: &Path) -> Result<(), String> {
    if !cfg!(windows) {
        return Err("NVIDIA DLSS is only used on Windows".into());
    }
    let meta = std::fs::metadata(source).map_err(|e| format!("{} cannot be read ({e})", source.display()))?;
    let named = source.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("nvngx_dlss.dll"));
    if !named || !meta.is_file() || meta.len() == 0 || meta.len() > PICK_MAX {
        return Err("That is not NVIDIA's DLSS file (nvngx_dlss.dll).".into());
    }
    let bytes = std::fs::read(source).map_err(|e| e.to_string())?;
    let pinned = meta.len() == SIZE && hex(&Sha256::digest(&bytes)) == SHA256;
    if !pinned && !signed_by_nvidia(source) {
        return Err("That file is not signed by NVIDIA, so it was not used.".into());
    }
    let dest = file(paths);
    std::fs::create_dir_all(dest.parent().ok_or("no folder for the DLSS file")?).map_err(|e| e.to_string())?;
    let part = dest.with_extension("dll.part");
    std::fs::write(&part, &bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&part, &dest).map_err(|e| e.to_string())
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
        fetch_verified(&serve(body.clone()), body.len() as u64, &digest, &dest, |p| last = p).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), body);
        assert_eq!(last, 1.0);
        std::fs::remove_file(&dest).unwrap();
        // Another hash, or another size: nothing is kept, not even the partial file.
        assert!(fetch_verified(&serve(body.clone()), body.len() as u64, &"0".repeat(64), &dest, |_| {}).is_err());
        assert!(fetch_verified(&serve(body.clone()), 4, &digest, &dest, |_| {}).is_err());
        assert!(!dest.exists() && !dest.with_extension("dll.part").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn only_rtx_cards_count() {
        assert!(rtx("NVIDIA", "NVIDIA GeForce RTX 5070 Ti"));
        assert!(rtx("NVIDIA", "NVIDIA RTX A2000"));
        assert!(!rtx("NVIDIA", "NVIDIA GeForce GTX 1660 SUPER"));
        assert!(!rtx("AMD", "AMD Radeon RX 9070 XT"));
    }

    /// Network: NVIDIA's real file from NVIDIA's GitHub, kept only with the pinned size and SHA-256 (into a temp folder).
    #[test]
    #[ignore]
    fn real_download_from_nvidia() {
        let dir = temp("real-download");
        let paths = Paths { exe_dir: dir.clone(), data: dir.join("data"), engine: None, portable: false };
        let mut last = 0.0;
        download(&paths, |p| last = p).unwrap();
        assert_eq!(std::fs::metadata(file(&paths)).unwrap().len(), SIZE);
        assert_eq!(last, 1.0);
        assert!(signed_by_nvidia(&file(&paths)));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// With AOD_TEST_NVNGX=<a real nvngx_dlss.dll> (not in the repository): NVIDIA's signed file is taken.
    #[test]
    #[ignore]
    fn a_picked_nvidia_file_is_kept() {
        let source = PathBuf::from(std::env::var("AOD_TEST_NVNGX").expect("AOD_TEST_NVNGX"));
        let dir = temp("pick-real");
        let paths = Paths { exe_dir: dir.clone(), data: dir.join("data"), engine: None, portable: false };
        pick(&paths, &source).unwrap();
        assert_eq!(std::fs::metadata(file(&paths)).unwrap().len(), std::fs::metadata(&source).unwrap().len());
        // A signed NVIDIA DLL under another name is refused.
        let other = dir.join("sl.dlss.dll");
        std::fs::copy(&source, &other).unwrap();
        assert!(pick(&paths, &other).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_picked_unsigned_file_is_refused() {
        let dir = temp("pick");
        std::fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("nvngx_dlss.dll");
        std::fs::write(&fake, b"MZ not signed").unwrap();
        let paths = Paths { exe_dir: dir.clone(), data: dir.join("data"), engine: None, portable: false };
        assert!(pick(&paths, &fake).is_err());
        assert!(!file(&paths).exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
