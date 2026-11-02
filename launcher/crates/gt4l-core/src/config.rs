//! `settings.toml`: settings, the chosen disc image and the optional development game folder.
//!
//! The file is written by hand (one `key = "value"` line per setting) so it stays readable, and it
//! is parsed with the `toml` crate. A file with a newer schema is never overwritten.

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::migrate::{self, Verdict};
use crate::settings::{self, Settings};
use crate::version::Version;

/// Schema of this file (`migrate::SETTINGS`). Files without a schema number are schema 0 and are migrated.
pub const SCHEMA_VERSION: u32 = 7;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigFile {
    pub settings: Settings,
    pub disc_path: Option<PathBuf>,
    pub dev_game_dir: Option<PathBuf>,
    /// "Skip this version" choices of the update popup: no offer again for exactly this version.
    pub skipped_launcher: Option<Version>,
    pub skipped_renderer: Option<Version>,
}

impl Default for ConfigFile {
    fn default() -> Self {
        ConfigFile {
            settings: Settings::default(),
            disc_path: None,
            dev_game_dir: None,
            skipped_launcher: None,
            skipped_renderer: None,
        }
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Syntax(String),
    NewerSchema(u32),
    Io(std::io::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Syntax(detail) => write!(formatter, "settings file is not valid TOML: {detail}"),
            ConfigError::NewerSchema(version) => {
                write!(formatter, "settings file uses schema {version}, newer than this launcher")
            }
            ConfigError::Io(error) => write!(formatter, "settings file: {error}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(error: std::io::Error) -> Self {
        ConfigError::Io(error)
    }
}

/// Parses the file text. Invalid setting values fall back to their defaults and are reported.
pub fn parse(text: &str) -> Result<(ConfigFile, Vec<String>), ConfigError> {
    let table: toml::Table = text
        .parse()
        .map_err(|error: toml::de::Error| ConfigError::Syntax(error.to_string()))?;
    let schema = table
        .get("schema_version")
        .and_then(|value| value.as_integer())
        .unwrap_or(SCHEMA_VERSION as i64);
    if schema > SCHEMA_VERSION as i64 {
        return Err(ConfigError::NewerSchema(schema as u32));
    }
    let mut warnings = Vec::new();
    let mut config = ConfigFile::default();
    if let Some(section) = table.get("settings").and_then(|value| value.as_table()) {
        for (key, value) in section {
            match (settings::definition(key), value.as_str()) {
                (None, _) => match settings::retired(key) {
                    // Load-time rule (no schema step): a retired setting is fixed now; its stored value never matters.
                    Some(reason) => warnings.push(format!("setting '{key}' is no longer used ({reason}); ignored")),
                    None => warnings.push(format!("unknown setting '{key}' ignored")),
                },
                (Some(_), None) => warnings.push(format!("setting '{key}' is not text; using the default")),
                (Some(_), Some(text)) => {
                    if let Some(warning) = config.settings.set_or_default(key, text) {
                        warnings.push(warning);
                    }
                }
            }
        }
    }
    // `[choices]`: settings the user set on the Settings page (see `Settings::set_by_user`).
    if let Some(section) = table.get("choices").and_then(|value| value.as_table()) {
        for (key, value) in section {
            if settings::definition(key).is_some() && value.as_bool() == Some(true) {
                config.settings.mark_chosen(key);
            }
        }
    }
    if let Some(section) = table.get("gs").and_then(|value| value.as_table()) {
        for (key, value) in section {
            match value.as_str() {
                None => warnings.push(format!("gs setting '{key}' is not text; using the default")),
                Some(text) => {
                    if let Err(error) = config.settings.set_gs(key, text) {
                        warnings.push(format!("{error}; using the default"));
                    }
                }
            }
        }
    }
    config.disc_path = table
        .get("install")
        .and_then(|value| value.as_table())
        .and_then(|section| section.get("disc_path"))
        .and_then(|value| value.as_str())
        .filter(|text| !text.is_empty())
        .map(PathBuf::from);
    config.dev_game_dir = table
        .get("dev")
        .and_then(|value| value.as_table())
        .and_then(|section| section.get("game_dir"))
        .and_then(|value| value.as_str())
        .map(PathBuf::from);
    if let Some(section) = table.get("updates").and_then(|value| value.as_table()) {
        let skipped = |key: &str| section.get(key).and_then(|value| value.as_str()).and_then(Version::parse);
        config.skipped_launcher = skipped("skip_launcher");
        config.skipped_renderer = skipped("skip_renderer");
    }
    Ok((config, warnings))
}

/// Renders the file. Output is deterministic: settings in table order, then the other sections.
pub fn render(config: &ConfigFile) -> String {
    let mut text = String::new();
    text.push_str("# GT4Recomp settings. Changes apply the next time you play.\n");
    text.push_str(&format!("schema_version = {SCHEMA_VERSION}\n\n[settings]\n"));
    for definition in settings::SETTINGS {
        text.push_str(&format!("{} = {}\n", definition.id, quote(config.settings.get(definition.id))));
    }
    let chosen = config.settings.chosen_ids();
    if !chosen.is_empty() {
        text.push_str("
[choices]
");
        for id in chosen {
            text.push_str(&format!("{id} = true
"));
        }
    }
    if !config.settings.gs_overrides().is_empty() {
        // PCSX2 GS table values (decision 0070): only the values the user set are stored.
        text.push_str("\n[gs]\n");
        for (key, value) in config.settings.gs_overrides() {
            text.push_str(&format!("{key} = {}\n", quote(value)));
        }
    }
    if config.skipped_launcher.is_some() || config.skipped_renderer.is_some() {
        text.push_str("\n[updates]\n");
        if let Some(version) = config.skipped_launcher {
            text.push_str(&format!("skip_launcher = {}\n", quote(&version.text())));
        }
        if let Some(version) = config.skipped_renderer {
            text.push_str(&format!("skip_renderer = {}\n", quote(&version.text())));
        }
    }
    text.push_str("\n[install]\n");
    if let Some(path) = &config.disc_path {
        text.push_str(&format!("disc_path = {}\n", quote(&path.to_string_lossy())));
    }
    if let Some(path) = &config.dev_game_dir {
        text.push_str("\n[dev]\n");
        text.push_str(&format!("game_dir = {}\n", quote(&path.to_string_lossy())));
    }
    text
}

/// A TOML basic string: quotes, backslashes and control characters are escaped.
fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            control if control.is_control() => out.push_str(&format!("\\u{:04X}", control as u32)),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// Loads the config file. A missing file gives defaults. A damaged file is renamed to
/// `settings.toml.bad` and defaults are used, so a bad file never blocks the launcher.
pub fn load(path: &Path) -> (ConfigFile, Vec<String>) {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return (ConfigFile::default(), Vec::new()),
        Err(error) => return (ConfigFile::default(), vec![format!("settings file unreadable: {error}")]),
    };
    let (text, mut notes) = match prepare(path, &text) {
        Ok(prepared) => prepared,
        Err(message) => {
            let backup = path.with_extension("toml.bad");
            let _ = fs::rename(path, &backup);
            return (ConfigFile::default(), vec![format!("{message}; moved to {}", backup.display())]);
        }
    };
    match parse(&text) {
        Ok((config, warnings)) => {
            notes.extend(warnings);
            (config, notes)
        }
        Err(ConfigError::NewerSchema(version)) => (
            ConfigFile::default(),
            vec![format!("settings file uses schema {version}; defaults used and the file is kept")],
        ),
        Err(error) => {
            let backup = path.with_extension("toml.bad");
            let _ = fs::rename(path, &backup);
            (ConfigFile::default(), vec![format!("{error}; moved to {}", backup.display())])
        }
    }
}

/// Runs the settings migrations before the file is read. A file of schema 0 (no `schema_version`, from the
/// earlier dev builds) is copied to `settings.toml.bak-schema0-<time>`, migrated, and written back. A newer file
/// is left as it is (`parse` refuses it). Returns the text to parse and the notes for the log.
fn prepare(path: &Path, text: &str) -> Result<(String, Vec<String>), String> {
    // A file that does not parse as TOML is reported by `parse`; nothing is migrated.
    let Ok(table) = text.parse::<toml::Table>() else {
        return Ok((text.to_string(), Vec::new()));
    };
    let mut document = serde_json::to_value(&table).map_err(|error| format!("settings file: {error}"))?;
    match migrate::plan(&migrate::SETTINGS, &document) {
        Verdict::Current | Verdict::Newer { .. } => Ok((text.to_string(), Vec::new())),
        Verdict::Corrupt(message) => Err(message),
        Verdict::Migrate { from } => {
            let backup = path.with_file_name(migrate::backup_name("settings.toml", from, unix_now()));
            fs::copy(path, &backup).map_err(|error| format!("settings backup: {error}"))?;
            migrate::migrate(&migrate::SETTINGS, &mut document)?;
            let migrated: toml::Table =
                serde_json::from_value(document).map_err(|error| format!("settings migration: {error}"))?;
            let migrated_text = toml::to_string(&migrated).map_err(|error| format!("settings migration: {error}"))?;
            let temporary = path.with_extension("toml.tmp");
            fs::write(&temporary, &migrated_text).map_err(|error| error.to_string())?;
            fs::rename(&temporary, path).map_err(|error| error.to_string())?;
            Ok((
                migrated_text,
                vec![format!("settings migrated from schema {from} to {SCHEMA_VERSION}; copy kept at {}", backup.display())],
            ))
        }
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

/// Writes the file atomically (temporary file, then rename).
pub fn save(path: &Path, config: &ConfigFile) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("toml.tmp");
    {
        let mut file = fs::File::create(&temporary)?;
        file.write_all(render(config).as_bytes())?;
        file.sync_all()?;
    }
    fs::rename(&temporary, path)?;
    Ok(())
}

/// Preset resolution mappings to canonical WxH strings (TASK-078).
pub fn preset_to_resolution(preset: &str) -> Option<&'static str> {
    match preset.trim().to_ascii_lowercase().as_str() {
        "480p" | "1x" | "1" => Some("640x480"),
        "720p" | "2x" | "2" => Some("1280x720"),
        "1080p" | "3x" | "3" => Some("1920x1080"),
        "1440p" | "4x" | "4" => Some("2560x1440"),
        "4k" | "2160p" | "6x" | "6" => Some("3840x2160"),
        "8k" | "4320p" | "12x" | "12" => Some("7680x4320"),
        _ => None,
    }
}

/// Merges in-game settings changes feedback written by the game (`game-changes.toml`) into the config (TASK-078).
/// If the file exists, parses `[settings]` and `[gs]`.
/// Supported keys:
/// - `[settings].fullscreen`: `"on"` / `"off"`
/// - `[settings].resolution`: `"480p"`, `"720p"`, `"1080p"`, `"1440p"`, `"4k"`, `"8k"` or `"WxH"`
/// - `[gs].fxaa` / `[gs].anti_aliasing`: `"true"` / `"false"`
/// - `[gs].upscale`: `"480p"`, `"720p"`, `"1080p"`, `"1440p"`, `"4k"`, `"8k"` or `"1x"`, `"2x"`, etc. (maps to `resolution`)
/// Removes the changes file after successful processing.
/// Returns `Ok(true)` if changes were applied, `Ok(false)` if the file does not exist.
pub fn merge_game_changes(config: &mut ConfigFile, changes_path: &Path) -> Result<bool, ConfigError> {
    if !changes_path.exists() {
        return Ok(false);
    }
    let text = match fs::read_to_string(changes_path) {
        Ok(text) => text,
        Err(error) => return Err(ConfigError::Io(error)),
    };
    let table: toml::Table = match text.parse() {
        Ok(table) => table,
        Err(error) => {
            let _ = fs::remove_file(changes_path);
            return Err(ConfigError::Syntax(error.to_string()));
        }
    };
    let mut modified = false;
    if let Some(settings_sec) = table.get("settings").and_then(|v| v.as_table()) {
        for (key, val) in settings_sec {
            if let Some(val_str) = val.as_str() {
                match key.as_str() {
                    "fullscreen" => {
                        let normalized = match val_str.trim().to_ascii_lowercase().as_str() {
                            "on" | "true" | "1" => "on",
                            _ => "off",
                        };
                        if config.settings.set("fullscreen", normalized).is_ok() {
                            modified = true;
                        }
                    }
                    "resolution" => {
                        let target_res = preset_to_resolution(val_str).unwrap_or(val_str);
                        if config.settings.set("resolution", target_res).is_ok() {
                            modified = true;
                        }
                    }
                    other => {
                        if config.settings.set(other, val_str).is_ok() {
                            modified = true;
                        }
                    }
                }
            }
        }
    }
    if let Some(gs_sec) = table.get("gs").and_then(|v| v.as_table()) {
        for (key, val) in gs_sec {
            if let Some(val_str) = val.as_str() {
                match key.as_str() {
                    "fxaa" | "anti_aliasing" => {
                        let normalized = match val_str.trim().to_ascii_lowercase().as_str() {
                            "on" | "true" | "1" => "true",
                            _ => "false",
                        };
                        if config.settings.set_gs("fxaa", normalized).is_ok() {
                            modified = true;
                        }
                    }
                    "upscale" => {
                        if let Some(res) = preset_to_resolution(val_str) {
                            if config.settings.set("resolution", res).is_ok() {
                                modified = true;
                            }
                        }
                    }
                    other => {
                        let _ = config.settings.set_gs(other, val_str);
                    }
                }
            }
        }
    }
    let _ = fs::remove_file(changes_path);
    Ok(modified)
}
