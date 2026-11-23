//! `aod-cli` — Age of Decomp's command line in a console program (no window): the same commands as `AgeOfDecomp cli …`
//! (src/cli.rs has them and their exit codes), plus `--apply-update <staging>`, which the launcher's own updates run
//! from a copy of this program while the launcher's files are replaced.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--apply-update") {
        let result = args.get(1).ok_or("Missing update staging directory".to_string())
            .and_then(|path| aod_launcher_lib::update::apply(std::path::Path::new(path)));
        if let Err(e) = result { eprintln!("{e}"); std::process::exit(1); }
        return;
    }
    std::process::exit(aod_launcher_lib::cli::run(args));
}
