//! Project and settings files in the application data directory.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use super::legacy;
use super::model::Project;
use crate::info;
use crate::state::now_ms;

#[derive(Clone, Debug, Serialize)]
pub struct ProjectInfo {
    pub id: String,
    pub name: String,
    /// Milliseconds since the Unix epoch.
    pub modified: u64,
}

/// Application-level settings, not part of any project.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub last_project: Option<String>,
    /// Free-form UI state (console height, last view, ...), owned by the frontend.
    pub ui: serde_json::Map<String, serde_json::Value>,
}

pub struct Store {
    dir: PathBuf,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    {
        let mut file = File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)
}

impl Store {
    /// The per-user data directory, e.g. `%APPDATA%\de.marcus-universe.vrc-midi-transposer`.
    pub fn default_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(info::IDENTIFIER)
    }

    pub fn open(dir: PathBuf) -> Result<Store, String> {
        fs::create_dir_all(dir.join("projects"))
            .map_err(|e| format!("Cannot create data directory {}: {e}", dir.display()))?;
        Ok(Store { dir })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn project_path(&self, id: &str) -> PathBuf {
        self.dir.join("projects").join(format!("{id}.json"))
    }

    /// Take the single-instance lock. The returned file must be kept alive.
    pub fn lock_instance(&self) -> Result<File, String> {
        let path = self.dir.join("instance.lock");
        let file = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
        match file.try_lock() {
            Ok(()) => Ok(file),
            Err(_) => Err(format!(
                "{} is already running with this data directory ({}).",
                info::APP_NAME,
                self.dir.display()
            )),
        }
    }

    pub fn new_id() -> String {
        format!("p{:x}", now_ms())
    }

    /// All projects, sorted by name.
    pub fn list(&self) -> Vec<ProjectInfo> {
        let mut out = Vec::new();
        let Ok(entries) = fs::read_dir(self.dir.join("projects")) else {
            return out;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|s| s.to_str()).filter(|s| valid_id(s)) else {
                continue;
            };
            let Ok(project) = self.load(id) else { continue };
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            out.push(ProjectInfo {
                id: id.to_string(),
                name: project.name,
                modified,
            });
        }
        out.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.id.cmp(&b.id))
        });
        out
    }

    pub fn load(&self, id: &str) -> Result<Project, String> {
        if !valid_id(id) {
            return Err(format!("Invalid project id '{id}'"));
        }
        let path = self.project_path(id);
        let text = fs::read_to_string(&path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
        let mut project: Project = serde_json::from_str(text.trim_start_matches('\u{feff}'))
            .map_err(|e| format!("Cannot parse {}: {e}", path.display()))?;
        // The file name is the identity; a copied file must not keep a stale id.
        project.id = id.to_string();
        Ok(project.normalized())
    }

    /// Write the project. Assigns an id if it has none.
    pub fn save(&self, project: &mut Project) -> Result<(), String> {
        if !valid_id(&project.id) {
            project.id = Self::new_id();
            while self.project_path(&project.id).exists() {
                project.id.push('x');
            }
        }
        let path = self.project_path(&project.id);
        let text = serde_json::to_string_pretty(project).map_err(|e| e.to_string())?;
        write_atomic(&path, text.as_bytes()).map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
        restrict_permissions(&path);
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        if !valid_id(id) {
            return Err(format!("Invalid project id '{id}'"));
        }
        let path = self.project_path(id);
        fs::remove_file(&path).map_err(|e| format!("Cannot delete {}: {e}", path.display()))
    }

    /// Find a project by id or by name (case-insensitive).
    pub fn find(&self, name_or_id: &str) -> Option<Project> {
        let list = self.list();
        let wanted = name_or_id.trim();
        let hit = list
            .iter()
            .find(|p| p.id == wanted)
            .or_else(|| list.iter().find(|p| p.name.eq_ignore_ascii_case(wanted)))?;
        self.load(&hit.id).ok()
    }

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

    pub fn remember_last(&self, id: &str) {
        let mut settings = self.settings();
        if settings.last_project.as_deref() != Some(id) {
            settings.last_project = Some(id.to_string());
            let _ = self.save_settings(&settings);
        }
    }

    /// Read a file chosen by the user: a v1 `config.json` or an exported project.
    /// The result has no id; saving it creates a new project.
    pub fn read_import(path: &Path) -> Result<Project, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
        let text = text.trim_start_matches('\u{feff}');
        let value: serde_json::Value =
            serde_json::from_str(text).map_err(|e| format!("{} is not valid JSON: {e}", path.display()))?;
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Imported");
        if legacy::looks_legacy(&value) {
            let name = if stem.eq_ignore_ascii_case("config") {
                "Imported config"
            } else {
                stem
            };
            return legacy::import(text, name);
        }
        if value.get("schema").is_none() || value.get("osc").is_none() {
            return Err(format!(
                "{} is neither a config.json nor an exported project.",
                path.display()
            ));
        }
        let mut project: Project =
            serde_json::from_value(value).map_err(|e| format!("Cannot read project: {e}"))?;
        project.id.clear();
        Ok(project.normalized())
    }

    /// The project to open at startup: the last one used, else the first one,
    /// else an import of a `config.json` found in `legacy_dirs`, else a new
    /// default project. The result is always saved.
    pub fn startup_project(&self, legacy_dirs: &[PathBuf]) -> Result<(Project, Option<String>), String> {
        if let Some(id) = self.settings().last_project {
            if let Ok(project) = self.load(&id) {
                return Ok((project, None));
            }
        }
        if let Some(first) = self.list().first() {
            return Ok((self.load(&first.id)?, None));
        }
        for dir in legacy_dirs {
            let path = dir.join("config.json");
            if path.is_file() {
                if let Ok(mut project) = Self::read_import(&path) {
                    self.save(&mut project)?;
                    return Ok((
                        project,
                        Some(format!("Imported settings from {}", path.display())),
                    ));
                }
            }
        }
        let mut project = Project::named("Default");
        self.save(&mut project)?;
        Ok((project, None))
    }
}

/// Project files can hold the MQTT password; keep them private to the user.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(tag: &str) -> Store {
        let dir = std::env::temp_dir().join(format!(
            "transposer-test-{tag}-{}-{}",
            std::process::id(),
            now_ms()
        ));
        Store::open(dir).unwrap()
    }

    #[test]
    fn save_load_list_delete() {
        let store = temp_store("crud");
        let mut a = Project::named("Beta");
        let mut b = Project::named("alpha");
        store.save(&mut a).unwrap();
        store.save(&mut b).unwrap();
        assert_ne!(a.id, b.id);
        let names: Vec<_> = store.list().into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["alpha", "Beta"]);
        assert_eq!(store.load(&a.id).unwrap(), a);
        assert_eq!(store.find("BETA").unwrap().id, a.id);
        store.delete(&a.id).unwrap();
        assert_eq!(store.list().len(), 1);
        assert!(store.load("../evil").is_err());
        let _ = fs::remove_dir_all(store.dir());
    }

    #[test]
    fn startup_imports_config_json_once() {
        let store = temp_store("startup");
        let legacy_dir = store.dir().join("legacy");
        fs::create_dir_all(&legacy_dir).unwrap();
        fs::write(
            legacy_dir.join("config.json"),
            r#"{ "midi": { "input_port_name_substr": "MRCC", "output_port_name_substr": "OUT" }, "mqtt": { "broker_host": "h" } }"#,
        )
        .unwrap();
        let (project, note) = store.startup_project(std::slice::from_ref(&legacy_dir)).unwrap();
        assert_eq!(project.name, "Imported config");
        assert_eq!(project.midi.input.as_ref().unwrap().name, "MRCC");
        assert!(note.is_some());
        store.remember_last(&project.id);
        let (again, note) = store.startup_project(&[legacy_dir]).unwrap();
        assert_eq!(again.id, project.id);
        assert!(note.is_none());
        assert_eq!(store.list().len(), 1);
        let _ = fs::remove_dir_all(store.dir());
    }

    #[test]
    fn startup_without_anything_creates_default() {
        let store = temp_store("default");
        let (project, _) = store.startup_project(&[]).unwrap();
        assert_eq!(project.name, "Default");
        assert_eq!(project.osc.listening_addresses.len(), 7);
        assert_eq!(store.list().len(), 1);
        let _ = fs::remove_dir_all(store.dir());
    }

    #[test]
    fn second_instance_is_refused() {
        let store = temp_store("lock");
        let first = store.lock_instance().unwrap();
        assert!(store.lock_instance().is_err());
        drop(first);
        assert!(store.lock_instance().is_ok());
        let _ = fs::remove_dir_all(store.dir());
    }
}
