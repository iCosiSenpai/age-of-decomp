// Age of Decomp harness appended by tools/recomplauncher_check.py to a scratch worktree of RecompLauncher's
// `src-tauri/src/devtools.rs` (never to the reference clone). It runs the launcher's own Developer-tab checks
// (`check_profile`, `summary`) on a release folder, and its own NUP reader on a player's dump when AOD_NUP is set.
// Lines starting with "AOD " are what the Python tool reports.

#[cfg(test)]
mod aod_check {
    use super::*;

    #[test]
    fn aod_check_release_folder() {
        let dir = PathBuf::from(std::env::var("AOD_RELEASE").expect("AOD_RELEASE = release folder"));
        let text = std::fs::read_to_string(dir.join(FILE)).expect("recomplauncher.json in the release folder");
        let v: Value = serde_json::from_str(&text).expect("valid JSON");
        let unknown: Vec<String> =
            v.as_object().expect("one JSON object").keys().filter(|k| !TOP_KEYS.contains(&k.as_str())).cloned().collect();
        // A line of its own: cargo prints the test name without a newline before the test's output.
        println!("\nAOD unknown top-level keys: {unknown:?}");
        let p: profile::Profile = serde_json::from_value(v).expect("fits the format");
        println!("AOD format {}", p.format);
        let g: Installed = serde_json::from_value(serde_json::json!({
            "id": "age-of-decomp-local", "name": "Age of Decomp", "tag": "local", "path": dir
        }))
        .expect("installed entry");
        let mut items = vec![];
        let level = check_profile(&p, &dir, &g, &mut items);
        for it in &items {
            let detail = if it.detail.is_empty() { String::new() } else { format!("  — {}", it.detail) };
            println!("AOD [{}] {}{}", it.level, it.text, detail);
        }
        let (flow, caps) = summary(&p);
        println!("AOD level {level}, flow {flow:?}, caps {caps:?}");
        assert!(unknown.is_empty(), "unknown top-level keys");
        assert!(!items.iter().any(|i| i.level == "err"), "the Developer tab would show errors");
    }

    #[test]
    fn aod_check_players_nup() {
        let Ok(path) = std::env::var("AOD_NUP") else { return };
        let want = std::env::var("AOD_BUILD_ID").expect("AOD_BUILD_ID");
        let have = crate::nup::build_id_of_package(std::path::Path::new(&path)).expect("build id from the NUP");
        let m = crate::nup::matches(&have, &want);
        println!("\nAOD nup build id {have}");
        println!("AOD nup matches {m:?}");
        assert_eq!(m, Ok(true));
    }
}
