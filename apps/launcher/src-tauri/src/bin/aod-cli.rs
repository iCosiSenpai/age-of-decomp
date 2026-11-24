//! `aod-cli` — Age of Decomp's command line in a console program (no window): the same commands as `AgeOfDecomp cli …`
//! (src/cli.rs has them and their exit codes) and the same `--play …`, plus `--apply-update <staging>`, which the
//! launcher's own updates run from a copy of this program while the launcher's files are replaced.

fn main() {
    let args = aod_launcher_lib::args().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    if args.first().is_some_and(|a| a == "--apply-update") {
        let result = args.get(1).ok_or("Missing update staging directory".to_string())
            .and_then(|path| aod_launcher_lib::update::apply(std::path::Path::new(path)));
        if let Err(e) = result { eprintln!("{e}"); std::process::exit(1); }
        return;
    }
    // `cli …` and `--play …` as the launcher takes them (the Linux AppImage's AppRun sends both here), else a command
    // without the leading `cli`.
    std::process::exit(aod_launcher_lib::headless(&args).unwrap_or_else(|| aod_launcher_lib::cli::run(args)));
}
