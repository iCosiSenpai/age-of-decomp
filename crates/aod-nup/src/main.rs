//! `aod-nup` — inspect a NUP and check it against the Age of Decomp game profile.
//!
//!   aod-nup info <file.nup>                 packages, versions, build IDs, RomFS size (JSON)
//!   aod-nup check <file.nup> [update.nup]   full profile check (JSON); exit 0 when the build is supported
//!   aod-nup build-id <file.nup>             build ID of the newest program package

use aod_nup::{Nup, profile};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = match std::env::args_os().skip(1).map(|a| a.into_string()).collect() {
        Ok(args) => args,
        Err(bad) => {
            eprintln!("aod-nup: an argument is not valid UTF-8 text: {}", bad.to_string_lossy());
            return ExitCode::from(2);
        }
    };
    let run = || -> Result<bool, aod_nup::Error> {
        match args.first().map(String::as_str) {
            Some("info") if args.len() == 2 => {
                let nup = Nup::open(&args[1])?;
                let mut pkgs = Vec::new();
                for p in nup.packages() {
                    let (files, bytes) = nup.romfs_stats(p)?;
                    pkgs.push(serde_json::json!({
                        "index": p.index, "kind": p.kind, "title_id": format!("{:016X}", p.title_id),
                        "version": p.version, "standalone_update": p.standalone_update, "meta": p.meta,
                        "build_id": nup.build_id(p)?, "romfs_files": files, "romfs_bytes": bytes,
                    }));
                }
                let out = serde_json::json!({ "file_size": nup.file_size(), "identity": nup.identity(), "packages": pkgs });
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
                Ok(true)
            }
            Some("check") if (2..=3).contains(&args.len()) => {
                let nup = Nup::open(&args[1])?;
                let update = args.get(2).map(Nup::open).transpose()?;
                let report = profile::check(&nup, update.as_ref(), &profile::GameProfile::hwaoi())?;
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
                Ok(report.ok)
            }
            Some("build-id") if args.len() == 2 => {
                let nup = Nup::open(&args[1])?;
                let p = nup.newest_program().ok_or_else(|| aod_nup::Error::Corrupt("no program package".into()))?;
                println!("{}", nup.build_id(p)?.unwrap_or_default());
                Ok(true)
            }
            _ => {
                eprintln!("usage: aod-nup info <file.nup> | check <file.nup> [update.nup] | build-id <file.nup>");
                Ok(false)
            }
        }
    };
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}
