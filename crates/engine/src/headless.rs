//! Terminal front end: the engine with a console on stdin/stdout, no window.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use crate::command::{self, Command};
use crate::project::Store;
use crate::state::{EngineEvent, Level, LogLine};
use crate::{info, Engine, EngineCmd};

#[derive(Debug, Default)]
pub struct Options {
    /// Data directory; the per-user default when `None`.
    pub data_dir: Option<PathBuf>,
    /// Project to open: a `.vrcmt` path, or the name (or 2.0 id) of a project. The last one when `None`.
    pub project: Option<String>,
    /// A v1 `config.json` or project file to open without remembering it.
    pub import: Option<PathBuf>,
}

fn print_colored(out: &mut StandardStream, color: Option<Color>, dim: bool, text: &str) {
    let mut spec = ColorSpec::new();
    spec.set_fg(color).set_intense(color.is_some()).set_dimmed(dim);
    let _ = out.set_color(&spec);
    let _ = write!(out, "{text}");
    let _ = out.reset();
    let _ = writeln!(out);
}

fn print_line(out: &mut StandardStream, line: &LogLine) {
    let (color, dim) = match line.level {
        Level::Info => (None, false),
        Level::Debug => (None, true),
        Level::Warn => (Some(Color::Yellow), false),
        Level::Error => (Some(Color::Red), false),
        Level::Success => (Some(Color::Green), false),
        Level::Accent => (Some(Color::Blue), false),
    };
    print_colored(out, color, dim, &line.text);
}

/// Directories where a v1 `config.json` may lie: the working directory and next to the executable.
pub fn legacy_config_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd);
    }
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
    {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    dirs
}

/// Run until `exit` is typed or the process is interrupted. Returns the exit code.
pub fn run(options: Options) -> i32 {
    let mut out = StandardStream::stdout(ColorChoice::Auto);
    let _ = writeln!(out);
    print_colored(
        &mut out,
        Some(Color::Blue),
        false,
        info::BANNER.trim_end_matches(['\r', '\n']),
    );
    let _ = writeln!(out);

    match run_engine(options, &mut out) {
        Ok(()) => 0,
        Err(message) => {
            print_colored(&mut out, Some(Color::Red), false, &format!("Error: {message}"));
            1
        }
    }
}

/// Where new and converted projects go. With a custom data directory (tests,
/// portable use) they stay inside it instead of the user's Documents folder.
pub fn documents_dir(custom_data_dir: bool, store: &Store) -> PathBuf {
    if custom_data_dir {
        store.dir().join("Projects")
    } else {
        Store::default_documents_dir()
    }
}

fn run_engine(options: Options, out: &mut StandardStream) -> Result<(), String> {
    let custom_dir = options.data_dir.is_some();
    let store = Store::open(options.data_dir.unwrap_or_else(Store::default_dir))?;
    let _instance = store.lock_instance()?;
    let documents = documents_dir(custom_dir, &store);

    let mut notes = Vec::new();
    let project = if let Some(path) = &options.import {
        let project = store.read_file(path)?;
        notes.push(format!(
            "Imported {} as project '{}'",
            path.display(),
            project.name
        ));
        project
    } else if let Some(name) = &options.project {
        let found = store.find(name, &documents)?;
        notes.extend(found.notes);
        found.project
    } else {
        let start = store.startup(None, &documents, &legacy_config_dirs());
        notes.extend(start.notes);
        start.project
    };
    let project_name = project.name.clone();

    let (engine, events) = Engine::start(project);
    let engine = Arc::new(engine);
    engine.send(EngineCmd::Log(Level::Info, format!("Project: {project_name}")));
    for note in notes {
        engine.send(EngineCmd::Log(Level::Info, note));
    }

    // Console input. An empty line quits; end of input does not, so the app
    // keeps running under a service manager or with stdin closed.
    let stdin_engine = engine.clone();
    thread::Builder::new()
        .name("stdin".into())
        .spawn(move || {
            let stdin = io::stdin();
            let mut line = String::new();
            loop {
                line.clear();
                match stdin.lock().read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                let quit = matches!(command::parse(&line), Command::Empty | Command::Exit);
                stdin_engine.send(EngineCmd::Console(if quit {
                    "exit".into()
                } else {
                    line.clone()
                }));
                if quit {
                    break;
                }
            }
        })
        .map_err(|e| e.to_string())?;

    let signal_engine = engine.clone();
    let _ = ctrlc::set_handler(move || signal_engine.send(EngineCmd::Console("exit".into())));

    for event in &events {
        match event {
            EngineEvent::Log(line) => print_line(out, &line),
            EngineEvent::ExitRequested => break,
            _ => {}
        }
    }

    print_colored(out, None, false, "Closing connections and exiting...");
    engine.shutdown();
    for event in events.try_iter() {
        if let EngineEvent::Log(line) = event {
            print_line(out, &line);
        }
    }
    Ok(())
}
