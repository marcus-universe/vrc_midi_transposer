// Release builds on Windows are GUI programs: no console window flashes up
// behind the app. `--headless` gets a console of its own (see `console`).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod console;
mod gui;

use std::path::PathBuf;

use transposer_engine::{headless, info};

const USAGE: &str = "\
Usage: vrc-midi-transposer [OPTIONS]

Without options the graphical app starts.

Options:
  --headless           Run in the terminal without a window
  --project <NAME>     Open this project (name or id) instead of the last one
  --import <FILE>      Import a v1 config.json or an exported project and open it
  --data-dir <DIR>     Use this data directory instead of the per-user default
  --version, -V        Print version, owner, license and repository
  --help, -h           Print this help";

#[derive(Debug, Default)]
struct Args {
    headless: bool,
    version: bool,
    help: bool,
    project: Option<String>,
    import: Option<PathBuf>,
    data_dir: Option<PathBuf>,
}

fn parse_args(mut argv: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut args = Args::default();
    while let Some(arg) = argv.next() {
        let mut value = |name: &str| argv.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--headless" => args.headless = true,
            "--version" | "-V" => args.version = true,
            "--help" | "-h" => args.help = true,
            "--project" => args.project = Some(value("--project")?),
            "--import" => args.import = Some(PathBuf::from(value("--import")?)),
            "--data-dir" => args.data_dir = Some(PathBuf::from(value("--data-dir")?)),
            // Arguments added by the OS or the bundle launcher (e.g. macOS -psn_…) are ignored.
            other if !other.starts_with("--") => {}
            other => return Err(format!("Unknown option {other}")),
        }
    }
    Ok(args)
}

fn main() {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            console::attach_for_output();
            eprintln!("{message}\n\n{USAGE}");
            std::process::exit(2);
        }
    };

    if args.version || args.help {
        console::attach_for_output();
        println!(
            "{}",
            if args.version {
                info::version_text()
            } else {
                USAGE.to_string()
            }
        );
        return;
    }

    if args.headless {
        console::ensure_interactive();
        let code = headless::run(headless::Options {
            data_dir: args.data_dir,
            project: args.project,
            import: args.import,
        });
        std::process::exit(code);
    }

    gui::run(gui::Options {
        data_dir: args.data_dir,
        project: args.project,
        import: args.import,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(list: &[&str]) -> Result<Args, String> {
        parse_args(list.iter().map(|s| s.to_string()))
    }

    #[test]
    fn parses_all_options() {
        let args = parse(&[
            "--headless",
            "--project",
            "My Setup",
            "--data-dir",
            "d",
            "--import",
            "config.json",
        ])
        .unwrap();
        assert!(args.headless);
        assert_eq!(args.project.as_deref(), Some("My Setup"));
        assert_eq!(args.data_dir, Some(PathBuf::from("d")));
        assert_eq!(args.import, Some(PathBuf::from("config.json")));
        assert!(parse(&["-V"]).unwrap().version);
        assert!(parse(&["--help"]).unwrap().help);
    }

    #[test]
    fn rejects_unknown_options_and_missing_values() {
        assert!(parse(&["--nope"]).is_err());
        assert!(parse(&["--project"]).is_err());
        assert!(parse(&["stray"]).is_ok());
    }
}
