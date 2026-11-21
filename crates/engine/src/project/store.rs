//! Project files (`.vrcmt`) and the application data directory.
//!
//! A project is a file the user saves wherever they like. The data directory
//! holds what belongs to this computer: settings (recent files, UI state),
//! the MQTT passwords (never written into project files, which get shared)
//! and the single-instance lock.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

use super::legacy;
use super::model::Project;
use crate::info;

/// File extension of project files.
pub const EXTENSION: &str = "vrcmt";
/// Value of the `format` field that marks a project file.
const FORMAT: &str = "vrcmt";
/// How many recent files are remembered.
const RECENT_MAX: usize = 10;

/// Application-level settings, not part of any project.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Most recently used project files, newest first.
    pub recent: Vec<PathBuf>,
    /// The file that was open when the app last closed.
    pub last_file: Option<PathBuf>,
    /// Projects of version 2.0 (stored in `projects/`) were converted to files.
    pub migrated: bool,
    /// Version 2.0 project ids and the files they were converted to, so that
    /// `--project <id>` keeps working.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub migrated_ids: BTreeMap<String, PathBuf>,
    /// Version 2.0: id of the last project in `projects/`. Read for migration only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_project: Option<String>,
    /// Free-form UI state (language, console height, last view, ...), owned by the frontend.
    pub ui: serde_json::Map<String, serde_json::Value>,
}

/// An entry of the recent files list, as the UI shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RecentFile {
    pub path: String,
    /// Project name stored in the file, or the file name if it cannot be read.
    pub name: String,
    pub exists: bool,
}

/// What `Store::startup` decided to open.
#[derive(Debug)]
pub struct Startup {
    pub project: Project,
    /// `None`: an untitled project that has never been saved.
    pub path: Option<PathBuf>,
    /// Things worth telling the user (imports, a file that could not be opened).
    pub notes: Vec<String>,
}

pub struct Store {
    dir: PathBuf,
    /// `recent()` as last built, with the list it was built from. Reading the
    /// files again on every call would block on slow or offline drives.
    recent_cache: Mutex<Option<(Vec<PathBuf>, Vec<RecentFile>)>>,
}

/// Who may read a file written by `write_atomic`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Access {
    /// The default permissions of new files.
    Normal,
    /// Readable and writable by the user only (unix). The file is created
    /// that way, so it is never readable by others, not even briefly.
    Private,
}

fn write_atomic(path: &Path, bytes: &[u8], access: Access) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    {
        let mut options = File::options();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        if access == Access::Private {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
            // A tmp file left over from a crash keeps its old mode: start fresh.
            let _ = fs::remove_file(&tmp);
        }
        #[cfg(not(unix))]
        let _ = access;
        let mut file = options.open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)
}

/// Create the data directory and its parents. On unix the data directory
/// itself is accessible by the user only, since it holds the MQTT passwords.
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    if let Some(parent) = dir.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dir)
}

/// A `.vrcmt` file (any case), as opposed to a v1 `config.json` or an exported `.json`.
pub fn is_project_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION))
}

/// The name stored in a project file, or `None` if it cannot be read.
fn project_name_in(path: &Path) -> Option<String> {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    fs::read_to_string(path)
        .ok()
        .and_then(|t| parse_file(&t, stem).ok())
        .map(|(p, _)| p.name)
}

/// Key under which an MQTT password is kept: the broker and the user name.
fn secret_key(project: &Project) -> Option<String> {
    let m = &project.mqtt;
    let host = m.broker_host.trim().to_lowercase();
    (!host.is_empty()).then(|| format!("{host}:{}:{}", m.broker_port, m.username))
}

/// A file name made from a project name (no characters that file systems reject).
pub fn file_name_for(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_.()".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches(['.', ' ']).to_string();
    let stem = if cleaned.is_empty() {
        "Project".to_string()
    } else {
        cleaned
    };
    format!("{stem}.{EXTENSION}")
}

/// `path`, or `path` with " (2)", " (3)" … before the extension if it exists.
fn unique_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Project")
        .to_string();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or(EXTENSION)
        .to_string();
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}).{ext}")))
        .find(|p| !p.exists())
        .expect("an unused file name")
}

/// The text of a project file: the project with a format marker, without id and password.
pub fn file_text(project: &Project) -> Result<String, String> {
    let mut clean = project.clone();
    clean.id.clear();
    clean.mqtt.password.clear();
    let mut value = serde_json::to_value(&clean).map_err(|e| e.to_string())?;
    let obj = value.as_object_mut().ok_or("project is not an object")?;
    obj.remove("id");
    let mut out = serde_json::Map::new();
    out.insert("format".into(), FORMAT.into());
    out.extend(std::mem::take(obj));
    serde_json::to_string_pretty(&out).map_err(|e| e.to_string())
}

/// Parse a project file, an exported 2.0 project or a v1 `config.json`.
/// Returns the project and whether it came from a v1 config.
pub fn parse_file(text: &str, file_stem: &str) -> Result<(Project, bool), String> {
    let text = text.trim_start_matches('\u{feff}');
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("not valid JSON: {e}"))?;
    if value.get("format").and_then(|f| f.as_str()) == Some(FORMAT) || value.get("schema").is_some() {
        let mut project: Project =
            serde_json::from_value(value).map_err(|e| format!("not a valid project: {e}"))?;
        project.id.clear();
        return Ok((project.normalized(), false));
    }
    if legacy::looks_legacy(&value) {
        let name = if file_stem.eq_ignore_ascii_case("config") {
            "Imported config"
        } else {
            file_stem
        };
        return Ok((legacy::import(text, name)?, true));
    }
    Err("neither a VRC MIDI Transposer project nor a config.json".into())
}

impl Store {
    /// The per-user data directory, e.g. `%APPDATA%\de.marcus-universe.vrc-midi-transposer`.
    pub fn default_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(info::IDENTIFIER)
    }

    /// Where new projects are saved by default: `Documents/VRC MIDI Transposer`.
    pub fn default_documents_dir() -> PathBuf {
        dirs::document_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(info::APP_NAME)
    }

    pub fn open(dir: PathBuf) -> Result<Store, String> {
        create_private_dir(&dir)
            .map_err(|e| format!("Cannot create data directory {}: {e}", dir.display()))?;
        Ok(Store {
            dir,
            recent_cache: Mutex::new(None),
        })
    }

    fn recent_cache(&self) -> MutexGuard<'_, Option<(Vec<PathBuf>, Vec<RecentFile>)>> {
        self.recent_cache.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Take the single-instance lock. The returned file must be kept alive.
    /// Retries for a moment, so a restart (after an update) can take over.
    pub fn lock_instance(&self) -> Result<File, String> {
        let path = self.dir.join("instance.lock");
        let file = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
        for attempt in 0..20 {
            if file.try_lock().is_ok() {
                return Ok(file);
            }
            if attempt < 19 {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        Err(format!(
            "{} is already running with this data directory ({}).",
            info::APP_NAME,
            self.dir.display()
        ))
    }

    // -- Settings -------------------------------------------------------------

    pub fn settings(&self) -> Settings {
        fs::read_to_string(self.dir.join("settings.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<(), String> {
        let path = self.dir.join("settings.json");
        let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
        write_atomic(&path, text.as_bytes(), Access::Normal)
            .map_err(|e| format!("Cannot write {}: {e}", path.display()))
    }

    /// Put `path` at the top of the recent files and remember it as the open file.
    pub fn remember(&self, path: &Path) {
        let mut settings = self.settings();
        settings.recent.retain(|p| p != path);
        settings.recent.insert(0, path.to_path_buf());
        settings.recent.truncate(RECENT_MAX);
        settings.last_file = Some(path.to_path_buf());
        let _ = self.save_settings(&settings);
        // The file may have just been saved under a new project name.
        self.recent_cache().take();
    }

    /// The open project has no file (new or imported and not saved yet).
    pub fn forget_last(&self) {
        let mut settings = self.settings();
        if settings.last_file.take().is_some() {
            let _ = self.save_settings(&settings);
        }
    }

    pub fn remove_recent(&self, path: &Path) {
        let mut settings = self.settings();
        settings.recent.retain(|p| p != path);
        let _ = self.save_settings(&settings);
        self.recent_cache().take();
    }

    /// The recent files with their project names. The files are read only when
    /// the list changed or one of them was saved, not on every call.
    pub fn recent(&self) -> Vec<RecentFile> {
        let paths = self.settings().recent;
        let mut cache = self.recent_cache();
        if let Some((cached_paths, list)) = cache.as_ref() {
            if *cached_paths == paths {
                return list.clone();
            }
        }
        let list: Vec<RecentFile> = paths
            .iter()
            .map(|path| {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
                RecentFile {
                    path: path.display().to_string(),
                    name: project_name_in(path).unwrap_or_else(|| stem.to_string()),
                    exists: path.is_file(),
                }
            })
            .collect();
        *cache = Some((paths, list.clone()));
        list
    }

    // -- Secrets --------------------------------------------------------------

    fn secrets(&self) -> BTreeMap<String, String> {
        fs::read_to_string(self.dir.join("secrets.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn save_secrets(&self, secrets: &BTreeMap<String, String>) -> Result<(), String> {
        let path = self.dir.join("secrets.json");
        let text = serde_json::to_string_pretty(secrets).map_err(|e| e.to_string())?;
        write_atomic(&path, text.as_bytes(), Access::Private)
            .map_err(|e| format!("Cannot write {}: {e}", path.display()))
    }

    /// Fill in the MQTT password kept on this computer for the project's broker and user.
    pub fn apply_secret(&self, project: &mut Project) {
        if project.mqtt.password.is_empty() {
            if let Some(password) = secret_key(project).and_then(|k| self.secrets().remove(&k)) {
                project.mqtt.password = password;
            }
        }
    }

    /// Keep the project's MQTT password on this computer (or forget it when empty).
    pub fn keep_secret(&self, project: &Project) -> Result<(), String> {
        let Some(key) = secret_key(project) else {
            return Ok(());
        };
        let mut secrets = self.secrets();
        let changed = if project.mqtt.password.is_empty() {
            secrets.remove(&key).is_some()
        } else {
            secrets.insert(key, project.mqtt.password.clone()).as_deref()
                != Some(project.mqtt.password.as_str())
        };
        if changed {
            self.save_secrets(&secrets)?;
        }
        Ok(())
    }

    // -- Project files --------------------------------------------------------

    /// Read a project file (or a v1 `config.json`), with the MQTT password filled in.
    pub fn read_file(&self, path: &Path) -> Result<Project, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Project");
        let (mut project, _) = parse_file(&text, stem).map_err(|e| format!("{} is {e}", path.display()))?;
        self.apply_secret(&mut project);
        Ok(project)
    }

    /// Write a project file. The password is kept on this computer, not in the file.
    pub fn write_file(&self, path: &Path, project: &Project) -> Result<(), String> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            fs::create_dir_all(dir).map_err(|e| format!("Cannot create {}: {e}", dir.display()))?;
        }
        let text = file_text(project)?;
        write_atomic(path, text.as_bytes(), Access::Normal)
            .map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
        self.keep_secret(project)
    }

    // -- Startup and migration ------------------------------------------------

    /// Convert the projects of version 2.0 (`projects/<id>.json` in the data
    /// directory) into `.vrcmt` files in `documents`. Runs once. The old folder
    /// is left as it is, so going back to 2.0 keeps working.
    pub fn migrate(&self, documents: &Path) -> Vec<String> {
        let mut settings = self.settings();
        if settings.migrated {
            return Vec::new();
        }
        let mut notes = Vec::new();
        let mut converted: Vec<(String, PathBuf)> = Vec::new();
        if let Ok(entries) = fs::read_dir(self.dir.join("projects")) {
            let mut files: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
                .collect();
            files.sort();
            for old in files {
                let id = old
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
                    .to_string();
                let Ok(text) = fs::read_to_string(&old) else {
                    continue;
                };
                let Ok((project, _)) = parse_file(&text, &id) else {
                    continue;
                };
                let target = unique_path(documents.join(file_name_for(&project.name)));
                match self.write_file(&target, &project) {
                    Ok(()) => converted.push((id, target)),
                    Err(e) => notes.push(format!("Could not convert project '{}': {e}", project.name)),
                }
            }
        }
        if !converted.is_empty() {
            notes.push(format!(
                "Your projects are now files in {} ({} converted).",
                documents.display(),
                converted.len()
            ));
        }
        // Newest first would need dates; keep the last opened project on top.
        let last = settings.last_project.take();
        converted.sort_by_key(|(id, _)| Some(id) != last.as_ref());
        settings.recent = converted
            .iter()
            .map(|(_, p)| p.clone())
            .take(RECENT_MAX)
            .collect();
        settings.last_file = converted
            .iter()
            .find(|(id, _)| Some(id) == last.as_ref())
            .or(converted.first())
            .map(|(_, p)| p.clone());
        settings.migrated_ids = converted.iter().cloned().collect();
        settings.migrated = true;
        let _ = self.save_settings(&settings);
        self.recent_cache().take();
        notes
    }

    /// Decide what to open at startup:
    /// 1. `requested` (a file given on the command line or by double-click),
    /// 2. the file that was open last time,
    /// 3. a v1 `config.json` found in `legacy_dirs` (imported, not yet saved),
    /// 4. a new untitled project.
    pub fn startup(&self, requested: Option<&Path>, documents: &Path, legacy_dirs: &[PathBuf]) -> Startup {
        let mut notes = self.migrate(documents);
        if let Some(path) = requested {
            match self.read_file(path) {
                Ok(project) => return self.opened(project, path, notes),
                Err(e) => notes.push(format!("Could not open {}: {e}", path.display())),
            }
        }
        if let Some(path) = self.settings().last_file {
            match self.read_file(&path) {
                Ok(project) => {
                    return Startup {
                        project,
                        path: Some(path),
                        notes,
                    }
                }
                Err(e) => notes.push(format!("Could not open the last project: {e}")),
            }
        }
        for dir in legacy_dirs {
            let path = dir.join("config.json");
            if let Ok(text) = fs::read_to_string(&path) {
                if let Ok((project, true)) = parse_file(&text, "config") {
                    notes.push(format!("Imported settings from {}", path.display()));
                    return Startup {
                        project,
                        path: None,
                        notes,
                    };
                }
            }
        }
        Startup {
            project: Project::named("Untitled"),
            path: None,
            notes,
        }
    }

    /// `project` was read from `path`. A project file keeps its path and is
    /// remembered. Anything else (a v1 `config.json`, an exported 2.0 `.json`)
    /// opens as an unsaved import, so Save never writes over it.
    fn opened(&self, project: Project, path: &Path, mut notes: Vec<String>) -> Startup {
        if is_project_path(path) {
            self.remember(path);
            return Startup {
                project,
                path: Some(path.to_path_buf()),
                notes,
            };
        }
        notes.push(format!(
            "Imported {} as project '{}'",
            path.display(),
            project.name
        ));
        Startup {
            project,
            path: None,
            notes,
        }
    }

    /// `--project`: a file path, or a project name or version 2.0 project id.
    /// Names are looked up in the recent files, the converted 2.0 projects and
    /// the project files in `documents`. The 2.0 projects are converted first
    /// if that has not happened yet. A project file is remembered as in `startup`.
    pub fn find(&self, path_or_name: &str, documents: &Path) -> Result<Startup, String> {
        let notes = self.migrate(documents);
        let as_path = PathBuf::from(path_or_name);
        if as_path.is_file() {
            let project = self.read_file(&as_path)?;
            return Ok(self.opened(project, &as_path, notes));
        }
        let wanted = path_or_name.trim();
        let settings = self.settings();
        if let Some(path) = settings.migrated_ids.get(wanted).filter(|p| p.is_file()) {
            let project = self.read_file(path)?;
            return Ok(self.opened(project, path, notes));
        }
        let mut candidates = settings.recent.clone();
        candidates.extend(settings.migrated_ids.values().cloned());
        if let Ok(entries) = fs::read_dir(documents) {
            let mut files: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| is_project_path(p))
                .collect();
            files.sort();
            candidates.extend(files);
        }
        let mut seen: Vec<PathBuf> = Vec::new();
        for path in candidates {
            if seen.contains(&path) || !path.is_file() {
                continue;
            }
            if project_name_in(&path).is_some_and(|n| n.eq_ignore_ascii_case(wanted)) {
                let project = self.read_file(&path)?;
                return Ok(self.opened(project, &path, notes));
            }
            seen.push(path);
        }
        let known: Vec<String> = self.recent().into_iter().map(|r| r.name).collect();
        Err(format!(
            "No project file, and no project named '{wanted}' in the recent files or {}. Recent: {}",
            documents.display(),
            if known.is_empty() {
                "none".to_string()
            } else {
                known.join(", ")
            }
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::now_ms;

    fn temp_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "transposer-test-{tag}-{}-{}",
            std::process::id(),
            now_ms()
        ))
    }

    fn project_with_password() -> Project {
        let mut p = Project::named("Stage piano");
        p.mqtt.broker_host = "192.168.1.5".into();
        p.mqtt.username = "ha".into();
        p.mqtt.password = "secret".into();
        p
    }

    #[test]
    fn files_round_trip_without_the_password() {
        let root = temp_dir("files");
        let store = Store::open(root.join("data")).unwrap();
        let path = root.join("docs").join("Stage piano.vrcmt");
        let project = project_with_password();
        store.write_file(&path, &project).unwrap();

        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains(r#""format": "vrcmt""#), "{text}");
        assert!(!text.contains("secret"), "no password in the file");
        assert!(!text.contains(r#""id""#));

        // Same computer: the password comes back.
        assert_eq!(
            store.read_file(&path).unwrap(),
            Project {
                id: String::new(),
                ..project.clone()
            }
        );

        // Another computer: no password.
        let other = Store::open(root.join("other")).unwrap();
        assert_eq!(other.read_file(&path).unwrap().mqtt.password, "");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reads_v1_configs_and_exported_projects_and_rejects_other_json() {
        let (p, legacy) = parse_file(r#"{"midi":{"input_port_name_substr":"MRCC"}}"#, "config").unwrap();
        assert!(legacy);
        assert_eq!(p.name, "Imported config");
        let exported = serde_json::to_string(&Project::named("Old export")).unwrap();
        let (p, legacy) = parse_file(&exported, "x").unwrap();
        assert!(!legacy);
        assert_eq!(p.name, "Old export");
        assert!(parse_file(r#"{"hello":1}"#, "x").is_err());
        assert!(parse_file("nope", "x").is_err());
    }

    #[test]
    fn recent_files_are_capped_and_deduplicated() {
        let root = temp_dir("recent");
        let store = Store::open(root.clone()).unwrap();
        for i in 0..12 {
            store.remember(&root.join(format!("{i}.vrcmt")));
        }
        store.remember(&root.join("5.vrcmt"));
        let recent = store.recent();
        assert_eq!(recent.len(), RECENT_MAX);
        assert!(recent[0].path.ends_with("5.vrcmt"));
        assert_eq!(recent.iter().filter(|r| r.path.ends_with("5.vrcmt")).count(), 1);
        assert!(!recent[0].exists);
        assert_eq!(store.settings().last_file, Some(root.join("5.vrcmt")));
        store.remove_recent(&root.join("5.vrcmt"));
        assert_eq!(store.recent().len(), RECENT_MAX - 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn version_2_0_projects_become_files_once() {
        let root = temp_dir("migrate");
        let data = root.join("data");
        fs::create_dir_all(data.join("projects")).unwrap();
        for (id, name) in [("p1", "Alpha"), ("p2", "Beta"), ("p3", "Alpha")] {
            let mut p = project_with_password();
            p.name = name.into();
            fs::write(
                data.join("projects").join(format!("{id}.json")),
                serde_json::to_string(&p).unwrap(),
            )
            .unwrap();
        }
        fs::write(
            data.join("settings.json"),
            r#"{"last_project":"p2","ui":{"view":"osc"}}"#,
        )
        .unwrap();
        let store = Store::open(data.clone()).unwrap();
        let docs = root.join("docs");

        let start = store.startup(None, &docs, &[]);
        assert_eq!(start.project.name, "Beta", "the last project of 2.0 opens");
        assert_eq!(
            start.project.mqtt.password, "secret",
            "password moved to this computer"
        );
        let mut names: Vec<_> = fs::read_dir(&docs)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, ["Alpha (2).vrcmt", "Alpha.vrcmt", "Beta.vrcmt"]);
        assert_eq!(store.recent().len(), 3);
        assert!(store.recent()[0].path.ends_with("Beta.vrcmt"));
        assert_eq!(store.settings().ui["view"], "osc", "UI settings survive");
        assert!(data.join("projects").join("p1.json").exists(), "old files stay");

        // Second start: nothing is converted again.
        assert!(store.migrate(&docs).is_empty());
        assert_eq!(fs::read_dir(&docs).unwrap().count(), 3);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn startup_order() {
        let root = temp_dir("startup");
        let store = Store::open(root.join("data")).unwrap();
        let docs = root.join("docs");
        let legacy = root.join("legacy");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(
            legacy.join("config.json"),
            r#"{"midi":{"input_port_name_substr":"MRCC"}}"#,
        )
        .unwrap();

        // Nothing yet: the v1 config is imported but not saved anywhere.
        let s = store.startup(None, &docs, std::slice::from_ref(&legacy));
        assert_eq!(
            (s.project.name.as_str(), s.path.is_none()),
            ("Imported config", true)
        );

        // A requested file wins and becomes the last file.
        let file = docs.join("Live.vrcmt");
        store.write_file(&file, &Project::named("Live")).unwrap();
        let s = store.startup(Some(&file), &docs, &[]);
        assert_eq!(s.project.name, "Live");
        let s = store.startup(None, &docs, &[]);
        assert_eq!(s.path.as_deref(), Some(file.as_path()));

        // A broken requested file falls back to the last file, with a note.
        let broken = docs.join("broken.vrcmt");
        fs::write(&broken, "{").unwrap();
        let s = store.startup(Some(&broken), &docs, &[]);
        assert_eq!(s.project.name, "Live");
        assert!(s.notes.iter().any(|n| n.contains("broken.vrcmt")));

        // Headless --project finds it by name.
        assert_eq!(store.find("live", &docs).unwrap().path, Some(file.clone()));
        assert!(store.find("nothing", &docs).is_err());

        // A v1 config.json given as the file opens as an import: no path, not remembered.
        let config = legacy.join("config.json");
        for start in [
            store.startup(Some(&config), &docs, &[]),
            store.find(config.to_str().unwrap(), &docs).unwrap(),
        ] {
            assert_eq!(start.project.name, "Imported config");
            assert_eq!(start.path, None);
        }
        assert_eq!(store.settings().last_file, Some(file.clone()));
        assert!(store.recent().iter().all(|r| !r.path.ends_with("config.json")));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn find_converts_2_0_projects_first_and_knows_their_ids() {
        let root = temp_dir("find");
        let data = root.join("data");
        fs::create_dir_all(data.join("projects")).unwrap();
        for i in 0..12 {
            let p = Project::named(&format!("Project {i}"));
            fs::write(
                data.join("projects").join(format!("id{i:02}.json")),
                serde_json::to_string(&p).unwrap(),
            )
            .unwrap();
        }
        let store = Store::open(data).unwrap();
        let docs = root.join("docs");

        // First start after the update goes straight to --project, without startup().
        let found = store.find("project 3", &docs).unwrap();
        assert_eq!(found.project.name, "Project 3");
        assert!(
            found.notes.iter().any(|n| n.contains("converted")),
            "{:?}",
            found.notes
        );
        // By 2.0 id, and beyond the ten recent entries.
        assert_eq!(store.find("id11", &docs).unwrap().project.name, "Project 11");
        let _ = fs::remove_file(docs.join("Project 11.vrcmt"));
        assert!(store.find("id11", &docs).is_err(), "a deleted file is not found");
        assert_eq!(
            store.find("Project 10", &docs).unwrap().project.name,
            "Project 10"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn recent_names_follow_saves() {
        let root = temp_dir("recent-cache");
        let store = Store::open(root.join("data")).unwrap();
        let file = root.join("a.vrcmt");
        store.write_file(&file, &Project::named("First")).unwrap();
        store.remember(&file);
        assert_eq!(store.recent()[0].name, "First");
        store.write_file(&file, &Project::named("Second")).unwrap();
        store.remember(&file);
        assert_eq!(store.recent()[0].name, "Second");
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn secrets_are_private_from_the_start() {
        use std::os::unix::fs::PermissionsExt;
        let root = temp_dir("secrets");
        let data = root.join("data");
        let store = Store::open(data.clone()).unwrap();
        store.keep_secret(&project_with_password()).unwrap();
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&data.join("secrets.json")), 0o600);
        assert_eq!(mode(&data), 0o700);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_name_for("Stage piano"), "Stage piano.vrcmt");
        assert_eq!(file_name_for("a/b:c*?"), "a_b_c__.vrcmt");
        assert_eq!(file_name_for("  "), "Project.vrcmt");
    }

    #[test]
    fn second_instance_is_refused() {
        let root = temp_dir("lock");
        let store = Store::open(root.clone()).unwrap();
        let first = store.lock_instance().unwrap();
        assert!(store.lock_instance().is_err());
        drop(first);
        assert!(store.lock_instance().is_ok());
        let _ = fs::remove_dir_all(root);
    }
}
