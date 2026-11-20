//! The window front end: Tauri commands and the event pump to the webview.
//!
//! The typed counterpart of this file is `src/lib/ipc.ts`; keep them in sync.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, RunEvent, State, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_window_state::StateFlags;

use transposer_engine::headless::{documents_dir, legacy_config_dirs};
use transposer_engine::project::{file_name_for, Project, RecentFile, Store, EXTENSION};
use transposer_engine::{info, Engine, EngineCmd, EngineEvent, Level, LogLine, Snapshot, Source};

/// Events are collected for this long before one frame is sent to the webview.
const COALESCE: Duration = Duration::from_millis(12);
/// Upper bound of log lines per frame; older ones in a burst are dropped from the frame.
const MAX_LOGS_PER_FRAME: usize = 300;

pub struct Options {
    pub data_dir: Option<PathBuf>,
    pub project: Option<String>,
    pub import: Option<PathBuf>,
    /// A project file given on the command line (double-click on a `.vrcmt`).
    pub file: Option<PathBuf>,
}

/// A file the system asked us to open (second launch, macOS "open with")
/// that the UI has not taken yet. Managed before setup: on macOS the request
/// can arrive before the engine exists.
#[derive(Default)]
pub struct PendingOpen(Mutex<Option<PathBuf>>);

fn is_false(value: &bool) -> bool {
    !*value
}

/// Coalesced update for the webview. Full state, never a delta (except `logs`).
#[derive(Clone, Default, Serialize)]
pub struct Frame {
    #[serde(skip_serializing_if = "Option::is_none")]
    snap: Option<Snapshot>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    logs: Vec<LogLine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    keys: Option<[u32; 8]>,
    /// Velocities of the keys set in `keys`, in bit order. Sent together with `keys`.
    #[serde(skip_serializing_if = "Option::is_none")]
    vel: Option<Vec<u8>>,
    /// `[pitch bend 0..=16383, mod wheel 0..=127]`. Sent together with `keys`.
    #[serde(skip_serializing_if = "Option::is_none")]
    wheels: Option<[u16; 2]>,
    #[serde(skip_serializing_if = "is_false")]
    cleared: bool,
    #[serde(skip_serializing_if = "is_false")]
    close_requested: bool,
    /// A file is waiting to be opened: ask about unsaved changes, then call `take_pending_open`.
    #[serde(skip_serializing_if = "is_false")]
    open_pending: bool,
}

#[derive(Serialize)]
pub struct ProjectState {
    project: Value,
    dirty: bool,
    /// The project file, or `None` for a project that was never saved.
    path: Option<String>,
    recent: Vec<RecentFile>,
}

#[derive(Serialize)]
struct AppInfo {
    name: &'static str,
    version: &'static str,
    owner: &'static str,
    email: &'static str,
    license: &'static str,
    repository: &'static str,
    data_dir: String,
    /// Where Save As starts for a project that has no file yet.
    documents_dir: String,
}

#[derive(Serialize)]
pub struct Init {
    #[serde(flatten)]
    state: ProjectState,
    app: AppInfo,
    banner: &'static str,
    snapshot: Snapshot,
    ui: Map<String, Value>,
    logs: Vec<LogLine>,
    keys: [u32; 8],
    vel: Vec<u8>,
    wheels: [u16; 2],
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiCmd {
    SetTranspose {
        value: i32,
    },
    Nudge {
        delta: i32,
    },
    SetOscSending {
        on: bool,
    },
    SetSendOriginal {
        on: bool,
    },
    SetDebug {
        on: bool,
    },
    CustomOsc {
        index: usize,
        value: f32,
    },
    RefreshPorts,
    Panic,
    /// MIDI Learn for a custom control; `index: null` cancels.
    MidiLearn {
        index: Option<usize>,
    },
}

/// The project being edited, its last saved version and its file.
struct Work {
    working: Project,
    saved: Project,
    path: Option<PathBuf>,
}

impl Work {
    /// Unsaved changes. A project without a file counts as changed unless it
    /// is still the empty default (an imported config.json must be saved).
    fn dirty(&self) -> bool {
        self.working != self.saved
            || (self.path.is_none() && self.working != Project::named("Untitled").normalized())
    }
}

pub struct AppState {
    engine: Engine,
    store: Store,
    documents: PathBuf,
    /// Released on exit before a restart, so the new process can take it.
    instance: Mutex<Option<File>>,
    work: Mutex<Work>,
    channel: Mutex<Option<Channel<Frame>>>,
    visible: AtomicBool,
    /// Something was not sent while hidden; send everything when visible again.
    stale: AtomicBool,
    quitting: AtomicBool,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The project as the webview sees it: the MQTT password is never sent.
fn to_ui(project: &Project) -> Value {
    let mut value = serde_json::to_value(project).unwrap_or(Value::Null);
    if let Some(mqtt) = value.get_mut("mqtt").and_then(Value::as_object_mut) {
        mqtt.insert(
            "password_set".into(),
            Value::Bool(!project.mqtt.password.is_empty()),
        );
        mqtt.insert("password".into(), Value::String(String::new()));
    }
    value
}

/// A project edited in the webview. An empty password with `password_set`
/// keeps the stored one. The id cannot be changed from the UI.
fn from_ui(value: Value, current: &Project) -> Result<Project, String> {
    let keep_password = value
        .pointer("/mqtt/password_set")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut project: Project = serde_json::from_value(value).map_err(|e| format!("Invalid project: {e}"))?;
    if project.mqtt.password.is_empty() && keep_password {
        project.mqtt.password = current.mqtt.password.clone();
    }
    project.id = current.id.clone();
    Ok(project.normalized())
}

impl AppState {
    fn init(options: &Options) -> Result<(AppState, Receiver<EngineEvent>), String> {
        let custom_dir = options.data_dir.is_some();
        let store = Store::open(options.data_dir.clone().unwrap_or_else(Store::default_dir))?;
        let instance = store.lock_instance()?;
        let documents = documents_dir(custom_dir, &store);

        let mut notes = Vec::new();
        let (project, path) = if let Some(path) = &options.import {
            let project = store.read_file(path)?;
            notes.push(format!(
                "Imported {} as project '{}'",
                path.display(),
                project.name
            ));
            (project, None)
        } else if let Some(name) = &options.project {
            let (project, path) = store.find(name)?;
            store.remember(&path);
            (project, Some(path))
        } else {
            let start = store.startup(options.file.as_deref(), &documents, &legacy_config_dirs());
            notes.extend(start.notes);
            (start.project, start.path)
        };
        if path.is_none() {
            store.forget_last();
        }

        let (engine, events) = Engine::start(project.clone());
        engine.send(EngineCmd::Log(Level::Info, format!("Project: {}", project.name)));
        for note in notes {
            engine.send(EngineCmd::Log(Level::Info, note));
        }

        // A file that was not imported starts clean; an import starts as unsaved.
        let saved = if path.is_some() {
            project.clone()
        } else {
            Project::named("Untitled").normalized()
        };
        let state = AppState {
            engine,
            store,
            documents,
            instance: Mutex::new(Some(instance)),
            work: Mutex::new(Work {
                working: project,
                saved,
                path,
            }),
            channel: Mutex::new(None),
            visible: AtomicBool::new(true),
            stale: AtomicBool::new(false),
            quitting: AtomicBool::new(false),
        };
        Ok((state, events))
    }

    fn project_state(&self, work: &Work) -> ProjectState {
        ProjectState {
            project: to_ui(&work.working),
            dirty: work.dirty(),
            path: work.path.as_ref().map(|p| p.display().to_string()),
            recent: self.store.recent(),
        }
    }

    fn is_dirty(&self) -> bool {
        lock(&self.work).dirty()
    }

    fn send(&self, frame: Frame) {
        let mut channel = lock(&self.channel);
        if let Some(ch) = channel.as_ref() {
            if ch.send(frame).is_err() {
                // The webview went away (reload); wait for the next subscribe.
                *channel = None;
            }
        }
    }

    /// Pressed keys, their velocities and the wheels, read together.
    fn visual(&self) -> ([u32; 8], Vec<u8>, [u16; 2]) {
        let keys = self.engine.take_keys();
        (keys, self.engine.velocities(&keys), self.engine.wheels())
    }

    /// Everything, for a webview that just became visible again.
    fn full_frame(&self) -> Frame {
        let (keys, vel, wheels) = self.visual();
        Frame {
            snap: Some(self.engine.snapshot()),
            logs: self.engine.logs(),
            keys: Some(keys),
            vel: Some(vel),
            wheels: Some(wheels),
            cleared: true,
            close_requested: false,
            open_pending: false,
        }
    }

    /// Make a project the open one and run it. `path: None` for an import or a new project.
    fn open(&self, work: &mut Work, project: Project, path: Option<PathBuf>) {
        match &path {
            Some(p) => self.store.remember(p),
            None => self.store.forget_last(),
        }
        self.engine
            .send(EngineCmd::Log(Level::Info, format!("Project: {}", project.name)));
        self.engine
            .send(EngineCmd::LoadProject(Box::new(project.clone())));
        work.saved = if path.is_some() {
            project.clone()
        } else {
            Project::named("Untitled").normalized()
        };
        work.working = project;
        work.path = path;
    }

    /// Open a file: a project keeps its path, a v1 config.json opens as an unsaved import.
    fn open_path(&self, work: &mut Work, path: &Path) -> Result<(), String> {
        let project = self.store.read_file(path)?;
        let is_project = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION));
        self.open(work, project, is_project.then(|| path.to_path_buf()));
        Ok(())
    }

    fn write(&self, work: &mut Work, path: PathBuf) -> Result<(), String> {
        self.store.write_file(&path, &work.working)?;
        self.store.remember(&path);
        work.saved = work.working.clone();
        work.path = Some(path);
        Ok(())
    }

    /// Let go of the single-instance lock (on exit, before a restart).
    fn release_instance(&self) {
        lock(&self.instance).take();
    }
}

/// The first argument of a second launch that names an existing file.
fn file_from_args(args: &[String], cwd: &str) -> Option<PathBuf> {
    args.iter()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .map(|a| {
            let p = PathBuf::from(a);
            if p.is_absolute() {
                p
            } else {
                Path::new(cwd).join(p)
            }
        })
        .find(|p| p.is_file())
}

/// Remember a file the system wants opened and tell the UI.
fn request_open(app: &AppHandle, path: PathBuf) {
    if let Some(pending) = app.try_state::<PendingOpen>() {
        *lock(&pending.0) = Some(path);
    }
    show_main_window(app);
    if let Some(state) = app.try_state::<AppState>() {
        state.send(Frame {
            open_pending: true,
            ..Frame::default()
        });
    }
}

// ---------------------------------------------------------------------------
// Event pump
// ---------------------------------------------------------------------------

fn request_quit(app: &AppHandle, state: &AppState) {
    if state.is_dirty() && lock(&state.channel).is_some() {
        state.send(Frame {
            close_requested: true,
            ..Frame::default()
        });
    } else {
        state.quitting.store(true, Ordering::Relaxed);
        app.exit(0);
    }
}

/// Forward engine events to the webview, at most one frame per `COALESCE`.
fn pump(app: AppHandle, events: Receiver<EngineEvent>) {
    let state = app.state::<AppState>();
    while let Ok(first) = events.recv() {
        let mut frame = Frame::default();
        let (mut want_snapshot, mut want_keys, mut exit) = (false, false, false);
        let mut absorb = |event: EngineEvent, frame: &mut Frame| match event {
            EngineEvent::Log(line) => frame.logs.push(line),
            EngineEvent::Changed => want_snapshot = true,
            EngineEvent::Keys => want_keys = true,
            EngineEvent::LogCleared => {
                frame.logs.clear();
                frame.cleared = true;
            }
            EngineEvent::ExitRequested => exit = true,
        };
        absorb(first, &mut frame);
        thread::sleep(COALESCE);
        for event in events.try_iter() {
            absorb(event, &mut frame);
        }

        if exit {
            request_quit(&app, &state);
        }
        if !state.visible.load(Ordering::Relaxed) {
            state.stale.store(true, Ordering::Relaxed);
            continue;
        }
        if state.stale.swap(false, Ordering::Relaxed) {
            state.send(state.full_frame());
            continue;
        }
        if frame.logs.len() > MAX_LOGS_PER_FRAME {
            let excess = frame.logs.len() - MAX_LOGS_PER_FRAME;
            frame.logs.drain(..excess);
        }
        if want_snapshot {
            frame.snap = Some(state.engine.snapshot());
        }
        if want_keys {
            let (keys, vel, wheels) = state.visual();
            frame.keys = Some(keys);
            frame.vel = Some(vel);
            frame.wheels = Some(wheels);
        }
        state.send(frame);
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn subscribe(state: State<AppState>, channel: Channel<Frame>) -> Init {
    *lock(&state.channel) = Some(channel);
    state.visible.store(true, Ordering::Relaxed);
    state.stale.store(false, Ordering::Relaxed);
    let work = lock(&state.work);
    let (keys, vel, wheels) = state.visual();
    Init {
        state: state.project_state(&work),
        app: AppInfo {
            name: info::APP_NAME,
            version: info::VERSION,
            owner: info::OWNER,
            email: info::EMAIL,
            license: info::LICENSE,
            repository: info::REPOSITORY,
            data_dir: state.store.dir().display().to_string(),
            documents_dir: state.documents.display().to_string(),
        },
        banner: info::BANNER,
        snapshot: state.engine.snapshot(),
        ui: state.store.settings().ui,
        logs: state.engine.logs(),
        keys,
        vel,
        wheels,
    }
}

#[tauri::command]
fn set_visible(state: State<AppState>, visible: bool) {
    state.visible.store(visible, Ordering::Relaxed);
    if visible && state.stale.swap(false, Ordering::Relaxed) {
        state.send(state.full_frame());
    }
}

#[tauri::command]
fn engine_cmd(state: State<AppState>, cmd: UiCmd) {
    state.engine.send(match cmd {
        UiCmd::SetTranspose { value } => EngineCmd::SetTranspose(value, Source::Ui),
        UiCmd::Nudge { delta } => EngineCmd::Nudge(delta, Source::Ui),
        UiCmd::SetOscSending { on } => EngineCmd::SetOscSending(on, Source::Ui),
        UiCmd::SetSendOriginal { on } => EngineCmd::SetSendOriginal(on, Source::Ui),
        UiCmd::SetDebug { on } => EngineCmd::SetDebug(on, Source::Ui),
        UiCmd::CustomOsc { index, value } => EngineCmd::CustomOsc {
            index,
            value,
            source: Source::Ui,
        },
        UiCmd::RefreshPorts => EngineCmd::RefreshPorts,
        UiCmd::Panic => EngineCmd::Panic,
        UiCmd::MidiLearn { index } => EngineCmd::MidiLearn(index),
    });
}

#[tauri::command]
fn console_line(state: State<AppState>, line: String) {
    let line = line.trim();
    if line.is_empty() {
        return;
    }
    state
        .engine
        .send(EngineCmd::Log(Level::Accent, format!("> {line}")));
    state.engine.send(EngineCmd::Console(line.to_string()));
}

#[tauri::command]
fn clear_log(state: State<AppState>) {
    state.engine.send(EngineCmd::ClearLog);
}

#[tauri::command]
fn update_project(state: State<AppState>, project: Value) -> Result<ProjectState, String> {
    let mut work = lock(&state.work);
    let project = from_ui(project, &work.working)?;
    if project != work.working {
        work.working = project.clone();
        state.engine.send(EngineCmd::UpdateProject(Box::new(project)));
    }
    Ok(state.project_state(&work))
}

#[tauri::command]
fn save_project(state: State<AppState>) -> Result<ProjectState, String> {
    let mut work = lock(&state.work);
    let path = work.path.clone().ok_or("NO_PATH")?;
    state.write(&mut work, path)?;
    Ok(state.project_state(&work))
}

/// Ask where to save, then save there. `None` if cancelled.
#[tauri::command]
async fn save_project_as(
    app: AppHandle,
    state: State<'_, AppState>,
    title: String,
) -> Result<Option<ProjectState>, String> {
    let (dir, name) = {
        let work = lock(&state.work);
        let dir = work
            .path
            .as_ref()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| state.documents.clone());
        (dir, file_name_for(&work.working.name))
    };
    let _ = std::fs::create_dir_all(&dir);
    let picked = app
        .dialog()
        .file()
        .set_title(title)
        .set_directory(&dir)
        .set_file_name(name)
        .add_filter(info::APP_NAME, &[EXTENSION])
        .blocking_save_file();
    let Some(picked) = picked else { return Ok(None) };
    let mut path = picked.into_path().map_err(|e| e.to_string())?;
    if !path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION))
    {
        let mut with_ext = path.into_os_string();
        with_ext.push(format!(".{EXTENSION}"));
        path = PathBuf::from(with_ext);
    }
    let mut work = lock(&state.work);
    state.write(&mut work, path)?;
    Ok(Some(state.project_state(&work)))
}

#[tauri::command]
fn revert_project(state: State<AppState>) -> Result<ProjectState, String> {
    let mut work = lock(&state.work);
    if work.working != work.saved {
        work.working = work.saved.clone();
        state
            .engine
            .send(EngineCmd::UpdateProject(Box::new(work.working.clone())));
    }
    Ok(state.project_state(&work))
}

#[tauri::command]
fn new_project(state: State<AppState>) -> ProjectState {
    let mut work = lock(&state.work);
    state.open(&mut work, Project::named("Untitled").normalized(), None);
    state.project_state(&work)
}

/// Pick a `.vrcmt` (or a v1 `config.json`) and open it. `None` if cancelled.
#[tauri::command]
async fn open_project_dialog(
    app: AppHandle,
    state: State<'_, AppState>,
    title: String,
) -> Result<Option<ProjectState>, String> {
    let dir = {
        let work = lock(&state.work);
        work.path
            .as_ref()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| state.documents.clone())
    };
    let picked = app
        .dialog()
        .file()
        .set_title(title)
        .set_directory(&dir)
        .add_filter(info::APP_NAME, &[EXTENSION, "json"])
        .blocking_pick_file();
    let Some(picked) = picked else { return Ok(None) };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    let mut work = lock(&state.work);
    state.open_path(&mut work, &path)?;
    Ok(Some(state.project_state(&work)))
}

/// Open a file by path (recent files, double-click).
#[tauri::command]
fn open_file(state: State<AppState>, path: String) -> Result<ProjectState, String> {
    let mut work = lock(&state.work);
    state.open_path(&mut work, Path::new(&path))?;
    Ok(state.project_state(&work))
}

#[tauri::command]
fn remove_recent(state: State<AppState>, path: String) -> ProjectState {
    state.store.remove_recent(Path::new(&path));
    state.project_state(&lock(&state.work))
}

/// The file the system asked to open, if any; clears the request.
#[tauri::command]
fn take_pending_open(pending: State<PendingOpen>) -> Option<String> {
    lock(&pending.0).take().map(|p| p.display().to_string())
}

#[tauri::command]
fn save_ui(state: State<AppState>, ui: Map<String, Value>) -> Result<(), String> {
    let mut settings = state.store.settings();
    settings.ui = ui;
    state.store.save_settings(&settings)
}

/// Open the project page in the default browser. The URL is a constant.
#[tauri::command]
fn open_repository() -> Result<(), String> {
    let url = info::REPOSITORY;
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = std::process::Command::new("explorer");
        c.arg(url);
        c
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = std::process::Command::new("open");
        c.arg(url);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };
    command
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Cannot open {url}: {e}"))
}

#[tauri::command]
fn quit(app: AppHandle, state: State<AppState>, force: bool) {
    if force {
        state.quitting.store(true, Ordering::Relaxed);
        app.exit(0);
    } else {
        request_quit(&app, &state);
    }
}

// ---------------------------------------------------------------------------
// Application
// ---------------------------------------------------------------------------

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn run(options: Options) {
    let app = tauri::Builder::default()
        // Must be the first plugin: a second start only focuses the running window.
        .plugin(tauri_plugin_single_instance::init(
            |app, args, cwd| match file_from_args(&args, &cwd) {
                Some(path) => request_open(app, path),
                None => show_main_window(app),
            },
        ))
        .manage(PendingOpen::default())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                // The window is shown by us once the engine is running.
                .with_state_flags(StateFlags::all() & !StateFlags::VISIBLE)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            match AppState::init(&options) {
                Ok((state, events)) => {
                    app.manage(state);
                    let handle = app.handle().clone();
                    thread::Builder::new()
                        .name("ui-pump".into())
                        .spawn(move || pump(handle, events))?;
                    show_main_window(app.handle());
                }
                Err(message) => {
                    // No engine, no window: tell the user why and leave.
                    app.dialog()
                        .message(message)
                        .title(info::APP_NAME)
                        .kind(MessageDialogKind::Error)
                        .show(|_| std::process::exit(1));
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let Some(state) = window.try_state::<AppState>() else {
                    return;
                };
                let ask = state.is_dirty()
                    && !state.quitting.load(Ordering::Relaxed)
                    && lock(&state.channel).is_some();
                if ask {
                    api.prevent_close();
                    state.send(Frame {
                        close_requested: true,
                        ..Frame::default()
                    });
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            subscribe,
            set_visible,
            engine_cmd,
            console_line,
            clear_log,
            update_project,
            save_project,
            save_project_as,
            revert_project,
            new_project,
            open_project_dialog,
            open_file,
            remove_recent,
            take_pending_open,
            save_ui,
            open_repository,
            quit,
        ])
        .build(tauri::generate_context!())
        .expect("failed to start the application");

    app.run(|app, event| match event {
        RunEvent::Exit => {
            // Release held notes, say "offline" on MQTT, close the ports, then
            // let go of the instance lock so a restart can start right away.
            if let Some(state) = app.try_state::<AppState>() {
                state.engine.shutdown();
                state.release_instance();
            }
        }
        // macOS delivers files to open (double-click, "Open With") as an event.
        #[cfg(target_os = "macos")]
        RunEvent::Opened { urls } => {
            if let Some(path) = urls.iter().find_map(|u| u.to_file_path().ok()) {
                request_open(app, path);
            }
        }
        _ => {}
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_never_reaches_the_ui_and_survives_a_round_trip() {
        let mut project = Project::named("p");
        project.id = "abc".into();
        project.mqtt.password = "secret".into();

        let ui = to_ui(&project);
        assert_eq!(ui["mqtt"]["password"], "");
        assert_eq!(ui["mqtt"]["password_set"], true);
        assert!(!ui.to_string().contains("secret"));

        // Unchanged in the UI: the stored password is kept.
        assert_eq!(from_ui(ui.clone(), &project).unwrap(), project);

        // Typed a new one.
        let mut typed = ui.clone();
        typed["mqtt"]["password"] = "new".into();
        assert_eq!(from_ui(typed, &project).unwrap().mqtt.password, "new");

        // Cleared.
        let mut cleared = ui.clone();
        cleared["mqtt"]["password_set"] = false.into();
        assert_eq!(from_ui(cleared, &project).unwrap().mqtt.password, "");

        // The id cannot be changed from the UI.
        let mut renamed = ui;
        renamed["id"] = "other".into();
        assert_eq!(from_ui(renamed, &project).unwrap().id, "abc");
    }

    #[test]
    fn empty_frames_serialize_to_nothing() {
        assert_eq!(serde_json::to_string(&Frame::default()).unwrap(), "{}");
        let frame = Frame {
            keys: Some([1, 0, 0, 0, 0, 0, 0, 0]),
            ..Frame::default()
        };
        assert_eq!(
            serde_json::to_string(&frame).unwrap(),
            r#"{"keys":[1,0,0,0,0,0,0,0]}"#
        );
    }

    #[test]
    fn ui_commands_deserialize() {
        let cmd: UiCmd = serde_json::from_str(r#"{"type":"set_transpose","value":-3}"#).unwrap();
        assert!(matches!(cmd, UiCmd::SetTranspose { value: -3 }));
        let cmd: UiCmd = serde_json::from_str(r#"{"type":"custom_osc","index":2,"value":0.5}"#).unwrap();
        assert!(matches!(cmd, UiCmd::CustomOsc { index: 2, .. }));
        assert!(matches!(
            serde_json::from_str::<UiCmd>(r#"{"type":"panic"}"#).unwrap(),
            UiCmd::Panic
        ));
    }
}
