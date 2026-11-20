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
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    {
        let mut file = File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)
}

/// Files that hold secrets are readable by the user only.
fn restrict_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
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
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Cannot create data directory {}: {e}", dir.display()))?;
        Ok(Store { dir })
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
        write_atomic(&path, text.as_bytes()).map_err(|e| format!("Cannot write {}: {e}", path.display()))
    }

    /// Put `path` at the top of the recent files and remember it as the open file.
    pub fn remember(&self, path: &Path) {
        let mut settings = self.settings();
        settings.recent.retain(|p| p != path);
        settings.recent.insert(0, path.to_path_buf());
        settings.recent.truncate(RECENT_MAX);
        settings.last_file = Some(path.to_path_buf());
        let _ = self.save_settings(&settings);
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
    }

    pub fn recent(&self) -> Vec<RecentFile> {
        self.settings()
            .recent
            .iter()
            .map(|path| {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
                let name = fs::read_to_string(path)
                    .ok()
                    .and_then(|t| parse_file(&t, stem).ok())
                    .map(|(p, _)| p.name)
                    .unwrap_or_else(|| stem.to_string());
                RecentFile {
                    path: path.display().to_string(),
                    name,
                    exists: path.is_file(),
                }
            })
            .collect()
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
        write_atomic(&path, text.as_bytes()).map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
        restrict_permissions(&path);
        Ok(())
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
        write_atomic(path, text.as_bytes()).map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
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
        settings.migrated = true;
        let _ = self.save_settings(&settings);
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
                Ok(project) => {
                    self.remember(path);
                    return Startup {
                        project,
                        path: Some(path.to_path_buf()),
                        notes,
                    };
                }
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

    /// A project by file path, or by name among the recent files (headless `--project`).
    pub fn find(&self, path_or_name: &str) -> Result<(Project, PathBuf), String> {
        let as_path = PathBuf::from(path_or_name);
        if as_path.is_file() {
            return Ok((self.read_file(&as_path)?, as_path));
        }
        let wanted = path_or_name.trim();
        for recent in self.recent() {
            if recent.exists && recent.name.eq_ignore_ascii_case(wanted) {
                let path = PathBuf::from(&recent.path);
                return Ok((self.read_file(&path)?, path));
            }
        }
        let known: Vec<String> = self.recent().into_iter().map(|r| r.name).collect();
        Err(format!(
            "No project file or recent project named '{wanted}'. Recent: {}",
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
        assert_eq!(store.find("live").unwrap().1, file);
        assert!(store.find("nothing").is_err());
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
