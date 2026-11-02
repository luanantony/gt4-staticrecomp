//! Texture pack importer, mod store, and active view manager (TASK-051 / TASK-052).
//!
//! Implements the mod management architecture specified in `docs/launcher/MOD-MANAGER-PLAN.md`:
//! - Safe zip archive extraction with Zip-Slip, Zip-Bomb, and device name validation.
//! - Raw PCSX2 layout detection (`SCUS-97328/replacements`, `replacements`, flat textures).
//! - Staging and atomic commit into `mods/store/<id>/`.
//! - Registry management (`mods.toml`) with priority ordering and enable/disable states.
//! - Active view construction (`active/textures/SCUS-97328/`) using hardlinks with copy fallback.
//! - Conflict detection and reporting between overlapping texture packs.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read, Seek};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::ZipArchive;

pub const MODS_SCHEMA_VERSION: u32 = 1;
pub const GT4_GAME_SERIAL: &str = "SCUS-97328";

pub const MAX_UNCOMPRESSED_FILE_BYTES: u64 = 256 * 1024 * 1024; // 256 MiB per file
pub const MAX_TOTAL_UNCOMPRESSED_BYTES: u64 = 20 * 1024 * 1024 * 1024; // 20 GiB total
pub const MAX_ZIP_ENTRIES: usize = 200_000;
pub const MAX_COMPRESSION_RATIO: u64 = 200;

/// Metadata manifest for an installed mod (`gt4mod.toml`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModManifest {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub kind: String, // "texture-pack"
    pub game: String, // "SCUS-97328" or "all"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl Default for ModManifest {
    fn default() -> Self {
        ModManifest {
            schema: MODS_SCHEMA_VERSION,
            id: String::new(),
            name: String::new(),
            version: "1.0.0".to_string(),
            author: None,
            kind: "texture-pack".to_string(),
            game: GT4_GAME_SERIAL.to_string(),
            description: None,
        }
    }
}

/// One entry in the mod registry list (`mods.toml`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModRegistryEntry {
    pub id: String,
    pub enabled: bool,
}

/// Global registry of installed mods (`mods.toml`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModRegistry {
    pub schema: u32,
    #[serde(default)]
    pub mods: Vec<ModRegistryEntry>,
}

impl Default for ModRegistry {
    fn default() -> Self {
        ModRegistry { schema: MODS_SCHEMA_VERSION, mods: Vec::new() }
    }
}

impl ModRegistry {
    pub fn load_from_file(path: &Path) -> io::Result<Self> {
        if !path.is_file() {
            return Ok(ModRegistry::default());
        }
        let text = fs::read_to_string(path)?;
        toml::from_str(&text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn save_to_file(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let temp_path = path.with_extension("toml.tmp");
        fs::write(&temp_path, text)?;
        fs::rename(&temp_path, path)?;
        Ok(())
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> bool {
        if let Some(entry) = self.mods.iter_mut().find(|m| m.id == id) {
            entry.enabled = enabled;
            return true;
        }
        false
    }

    pub fn set_priority(&mut self, id: &str, new_index: usize) -> bool {
        if let Some(pos) = self.mods.iter().position(|m| m.id == id) {
            let entry = self.mods.remove(pos);
            let target_index = new_index.min(self.mods.len());
            self.mods.insert(target_index, entry);
            return true;
        }
        false
    }
}

/// Report returned after importing a mod.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ImportReport {
    pub id: String,
    pub name: String,
    pub texture_count: usize,
    pub extras_count: usize,
    pub warnings: Vec<String>,
}

/// A conflict where two or more enabled mods provide the same texture key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    pub texture_stem: String,
    pub winning_mod: String,
    pub overridden_mod: String,
}

/// Summary of an active view build.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ActiveViewReport {
    pub active_textures: usize,
    pub conflicts: Vec<Conflict>,
    pub hardlinks_created: usize,
    pub copies_created: usize,
}

/// Checks whether a raw path inside an archive is safe against Zip-Slip attacks.
pub fn check_zip_path_safety(raw_path: &str) -> Result<PathBuf, String> {
    if raw_path.is_empty() {
        return Err("Empty path entry".to_string());
    }
    if raw_path.contains(':') {
        return Err(format!("Colons and NTFS alternate data streams refused in '{raw_path}'"));
    }
    if raw_path.starts_with('/') || raw_path.starts_with('\\') {
        return Err(format!("Absolute path refused in '{raw_path}'"));
    }

    let path = Path::new(raw_path);
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {
                return Err(format!("Root or prefix component refused in '{raw_path}'"));
            }
            Component::ParentDir => {
                return Err(format!("Parent directory traversal ('..') refused in '{raw_path}'"));
            }
            Component::Normal(os_str) => {
                let name = os_str.to_string_lossy();
                let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
                if matches!(
                    stem.as_str(),
                    "CON" | "PRN" | "AUX" | "NUL"
                        | "COM1" | "COM2" | "COM3" | "COM4" | "COM5" | "COM6" | "COM7" | "COM8" | "COM9"
                        | "LPT1" | "LPT2" | "LPT3" | "LPT4" | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9"
                ) {
                    return Err(format!("Windows reserved device name refused in '{raw_path}'"));
                }
            }
            Component::CurDir => {}
        }
    }
    Ok(path.to_path_buf())
}

/// Determines whether a file extension is a dangerous executable or script.
pub fn is_forbidden_extension(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "exe" | "bat" | "cmd" | "ps1" | "vbs" | "dll" | "so" | "sh" | "com" | "scr" | "pif"
    )
}

/// Determines whether a file extension is a supported texture replacement image.
pub fn is_texture_extension(ext: &str) -> bool {
    matches!(ext.to_ascii_lowercase().as_str(), "png" | "dds")
}

/// Determines whether a file extension is an allowed document/extra asset.
pub fn is_extra_extension(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "txt" | "md" | "pdf" | "jpg" | "jpeg" | "json" | "toml"
    )
}

/// Normalizes relative texture paths across various PCSX2 and raw folder structures.
pub fn normalize_texture_relative_path(path: &Path) -> Option<PathBuf> {
    let parts: Vec<&str> = path.iter().filter_map(|p| p.to_str()).collect();
    if parts.first().map(|s| s.eq_ignore_ascii_case("dumps")).unwrap_or(false) {
        return None;
    }
    if let Some(pos) = parts.iter().position(|s| s.eq_ignore_ascii_case("replacements")) {
        if pos + 1 < parts.len() {
            return Some(parts[pos + 1..].iter().collect::<PathBuf>());
        }
    }
    if let Some(pos) = parts.iter().position(|s| s.eq_ignore_ascii_case(GT4_GAME_SERIAL)) {
        if pos + 1 < parts.len() {
            let rest = &parts[pos + 1..];
            if rest.first().map(|s| s.eq_ignore_ascii_case("replacements")).unwrap_or(false) && rest.len() > 1 {
                return Some(rest[1..].iter().collect::<PathBuf>());
            }
            return Some(rest.iter().collect::<PathBuf>());
        }
    }
    Some(path.to_path_buf())
}

/// Attempts to hardlink `source` to `target`. If hardlink fails, falls back to copying.
/// Returns `Ok(true)` if hardlink succeeded, `Ok(false)` if copied.
pub fn link_or_copy(source: &Path, target: &Path) -> io::Result<bool> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    if target.exists() || fs::symlink_metadata(target).is_ok() {
        let _ = fs::remove_file(target);
    }
    match fs::hard_link(source, target) {
        Ok(()) => Ok(true),
        Err(_) => {
            fs::copy(source, target)?;
            Ok(false)
        }
    }
}

/// Computes the SHA-256 digest of a byte slice.
pub fn compute_sha256(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Importer and manager for mods.
pub struct ModManager {
    mods_root: PathBuf,
}

impl ModManager {
    pub fn new(mods_root: impl Into<PathBuf>) -> Self {
        ModManager { mods_root: mods_root.into() }
    }

    pub fn store_dir(&self) -> PathBuf {
        self.mods_root.join("store")
    }

    pub fn staging_dir(&self) -> PathBuf {
        self.mods_root.join("staging")
    }

    pub fn active_dir(&self) -> PathBuf {
        self.mods_root.join("active").join("textures").join(GT4_GAME_SERIAL)
    }

    pub fn registry_file(&self) -> PathBuf {
        self.mods_root.join("mods.toml")
    }

    /// Imports a mod from a `.zip` archive.
    pub fn import_zip<R: Read + Seek>(&self, reader: R, fallback_id: &str) -> io::Result<ImportReport> {
        let mut archive = ZipArchive::new(reader).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        if archive.len() > MAX_ZIP_ENTRIES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Zip contains too many entries ({} > {})", archive.len(), MAX_ZIP_ENTRIES),
            ));
        }

        let staging_id = format!("stage_{}_{}", fallback_id, std::process::id());
        let stage_path = self.staging_dir().join(&staging_id);
        let _ = fs::remove_dir_all(&stage_path);
        fs::create_dir_all(&stage_path)?;

        let mut report = ImportReport::default();
        let mut total_uncompressed: u64 = 0;
        let mut manifest: Option<ModManifest> = None;
        let mut lock_entries: HashMap<String, String> = HashMap::new();

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            if file.is_dir() {
                continue;
            }

            let uncompressed_size = file.size();
            let compressed_size = file.compressed_size();
            total_uncompressed += uncompressed_size;

            if uncompressed_size > MAX_UNCOMPRESSED_FILE_BYTES {
                let _ = fs::remove_dir_all(&stage_path);
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Entry exceeds maximum file size ({} bytes)", uncompressed_size),
                ));
            }
            if total_uncompressed > MAX_TOTAL_UNCOMPRESSED_BYTES {
                let _ = fs::remove_dir_all(&stage_path);
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Total uncompressed size exceeds limit ({} bytes)", total_uncompressed),
                ));
            }
            if compressed_size > 0 && (uncompressed_size / compressed_size) > MAX_COMPRESSION_RATIO {
                let _ = fs::remove_dir_all(&stage_path);
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Suspicious compression ratio on entry '{}'", file.name()),
                ));
            }

            let safe_path = match check_zip_path_safety(file.name()) {
                Ok(p) => p,
                Err(err) => {
                    report.warnings.push(format!("Skipping unsafe entry: {err}"));
                    continue;
                }
            };

            let ext = safe_path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if is_forbidden_extension(ext) {
                report.warnings.push(format!("Dropped executable or script: '{}'", file.name()));
                continue;
            }

            if safe_path.file_name().map(|n| n == "gt4mod.toml").unwrap_or(false) {
                let mut content = String::new();
                if file.read_to_string(&mut content).is_ok() {
                    if let Ok(m) = toml::from_str::<ModManifest>(&content) {
                        manifest = Some(m);
                    }
                }
                continue;
            }

            if is_texture_extension(ext) {
                if let Some(norm_rel) = normalize_texture_relative_path(&safe_path) {
                    let dest = stage_path.join("files").join(&norm_rel);
                    if let Some(parent) = dest.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    let mut data = Vec::with_capacity(uncompressed_size as usize);
                    file.read_to_end(&mut data)?;
                    fs::write(&dest, &data)?;
                    lock_entries.insert(norm_rel.to_string_lossy().to_string(), compute_sha256(&data));
                    report.texture_count += 1;
                }
            } else if is_extra_extension(ext) {
                let dest = stage_path.join("extras").join(safe_path.file_name().unwrap_or_default());
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut out = File::create(&dest)?;
                io::copy(&mut file, &mut out)?;
                report.extras_count += 1;
            }
        }

        let mut final_manifest = manifest.unwrap_or_else(|| ModManifest {
            id: fallback_id.to_string(),
            name: fallback_id.to_string(),
            ..ModManifest::default()
        });

        if final_manifest.id.is_empty() {
            final_manifest.id = fallback_id.to_string();
        }
        if final_manifest.name.is_empty() {
            final_manifest.name = fallback_id.to_string();
        }

        let manifest_text = toml::to_string_pretty(&final_manifest)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(stage_path.join("gt4mod.toml"), manifest_text)?;

        let lock_json = serde_json::to_string_pretty(&lock_entries)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(stage_path.join("files.lock.json"), lock_json)?;

        let dest_store = self.store_dir().join(&final_manifest.id);
        if dest_store.exists() {
            fs::remove_dir_all(&dest_store)?;
        }
        if let Some(parent) = dest_store.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&stage_path, &dest_store)?;

        let mut registry = ModRegistry::load_from_file(&self.registry_file())?;
        if !registry.mods.iter().any(|m| m.id == final_manifest.id) {
            registry.mods.push(ModRegistryEntry {
                id: final_manifest.id.clone(),
                enabled: true,
            });
            registry.save_to_file(&self.registry_file())?;
        }

        report.id = final_manifest.id;
        report.name = final_manifest.name;
        Ok(report)
    }

    /// Imports a mod from a loose folder.
    pub fn import_folder(&self, source_folder: &Path, fallback_id: &str) -> io::Result<ImportReport> {
        if !source_folder.is_dir() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "Source folder does not exist"));
        }

        let staging_id = format!("stage_{}_{}", fallback_id, std::process::id());
        let stage_path = self.staging_dir().join(&staging_id);
        let _ = fs::remove_dir_all(&stage_path);
        fs::create_dir_all(&stage_path)?;

        let mut report = ImportReport::default();
        let mut manifest: Option<ModManifest> = None;
        let mut lock_entries: HashMap<String, String> = HashMap::new();

        for entry in walkdir(source_folder)? {
            if entry.is_dir() {
                continue;
            }
            let rel_path = match entry.strip_prefix(source_folder) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let ext = rel_path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if is_forbidden_extension(ext) {
                report.warnings.push(format!("Dropped executable or script: '{}'", rel_path.display()));
                continue;
            }

            if rel_path.file_name().map(|n| n == "gt4mod.toml").unwrap_or(false) {
                if let Ok(content) = fs::read_to_string(&entry) {
                    if let Ok(m) = toml::from_str::<ModManifest>(&content) {
                        manifest = Some(m);
                    }
                }
                continue;
            }

            if is_texture_extension(ext) {
                if let Some(norm_rel) = normalize_texture_relative_path(rel_path) {
                    let dest = stage_path.join("files").join(&norm_rel);
                    if let Some(parent) = dest.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    let data = fs::read(&entry)?;
                    fs::write(&dest, &data)?;
                    lock_entries.insert(norm_rel.to_string_lossy().to_string(), compute_sha256(&data));
                    report.texture_count += 1;
                }
            } else if is_extra_extension(ext) {
                let dest = stage_path.join("extras").join(rel_path.file_name().unwrap_or_default());
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&entry, &dest)?;
                report.extras_count += 1;
            }
        }

        let mut final_manifest = manifest.unwrap_or_else(|| ModManifest {
            id: fallback_id.to_string(),
            name: fallback_id.to_string(),
            ..ModManifest::default()
        });

        if final_manifest.id.is_empty() {
            final_manifest.id = fallback_id.to_string();
        }
        if final_manifest.name.is_empty() {
            final_manifest.name = fallback_id.to_string();
        }

        let manifest_text = toml::to_string_pretty(&final_manifest)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(stage_path.join("gt4mod.toml"), manifest_text)?;

        let lock_json = serde_json::to_string_pretty(&lock_entries)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(stage_path.join("files.lock.json"), lock_json)?;

        let dest_store = self.store_dir().join(&final_manifest.id);
        if dest_store.exists() {
            fs::remove_dir_all(&dest_store)?;
        }
        if let Some(parent) = dest_store.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&stage_path, &dest_store)?;

        let mut registry = ModRegistry::load_from_file(&self.registry_file())?;
        if !registry.mods.iter().any(|m| m.id == final_manifest.id) {
            registry.mods.push(ModRegistryEntry {
                id: final_manifest.id.clone(),
                enabled: true,
            });
            registry.save_to_file(&self.registry_file())?;
        }

        report.id = final_manifest.id;
        report.name = final_manifest.name;
        Ok(report)
    }

    /// Rebuilds the active view directory (`active/textures/SCUS-97328/`) from all enabled mods.
    pub fn build_active_view(&self) -> io::Result<ActiveViewReport> {
        let registry = ModRegistry::load_from_file(&self.registry_file())?;
        let active_dir = self.active_dir();
        let _ = fs::remove_dir_all(&active_dir);
        fs::create_dir_all(&active_dir)?;

        let mut report = ActiveViewReport::default();
        let mut winning_textures: HashMap<String, (String, PathBuf)> = HashMap::new();

        for mod_entry in registry.mods.iter().filter(|m| m.enabled) {
            let files_dir = self.store_dir().join(&mod_entry.id).join("files");
            if !files_dir.is_dir() {
                continue;
            }

            for file_path in walkdir(&files_dir)? {
                if file_path.is_dir() {
                    continue;
                }
                let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if !is_texture_extension(ext) {
                    continue;
                }
                let file_name = file_path.file_name().unwrap_or_default().to_string_lossy().to_string();
                let stem = file_path.file_stem().unwrap_or_default().to_string_lossy().to_string();

                if let Some((winning_mod, _)) = winning_textures.get(&file_name) {
                    report.conflicts.push(Conflict {
                        texture_stem: stem,
                        winning_mod: winning_mod.clone(),
                        overridden_mod: mod_entry.id.clone(),
                    });
                } else {
                    winning_textures.insert(file_name, (mod_entry.id.clone(), file_path));
                }
            }
        }

        for (file_name, (_mod_id, src_path)) in &winning_textures {
            let dest_path = active_dir.join(file_name);
            match link_or_copy(src_path, &dest_path)? {
                true => report.hardlinks_created += 1,
                false => report.copies_created += 1,
            }
            report.active_textures += 1;
        }

        Ok(report)
    }
}

fn walkdir(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut results = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                results.extend(walkdir(&path)?);
            } else {
                results.push(path);
            }
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn temp_test_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("gt4l_core_mods_{}_{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn zip_path_safety_rejects_parent_traversals_and_colons() {
        assert!(check_zip_path_safety("../evil.png").is_err());
        assert!(check_zip_path_safety("cars/../../evil.png").is_err());
        assert!(check_zip_path_safety("file.png:stream").is_err());
        assert!(check_zip_path_safety("/absolute/file.png").is_err());
        assert!(check_zip_path_safety("C:/file.png").is_err());
        assert!(check_zip_path_safety("NUL").is_err());
        assert!(check_zip_path_safety("con.png").is_err());
        assert!(check_zip_path_safety("cars/supra.png").is_ok());
    }

    #[test]
    fn normalize_texture_relative_path_handles_pcsx2_layouts() {
        let p1 = Path::new("SCUS-97328/replacements/cars/supra.png");
        assert_eq!(normalize_texture_relative_path(p1), Some(PathBuf::from("cars/supra.png")));

        let p2 = Path::new("textures/SCUS-97328/replacements/tracks/ring.dds");
        assert_eq!(normalize_texture_relative_path(p2), Some(PathBuf::from("tracks/ring.dds")));

        let p3 = Path::new("replacements/ui/hud.png");
        assert_eq!(normalize_texture_relative_path(p3), Some(PathBuf::from("ui/hud.png")));

        let p4 = Path::new("dumps/ignored.png");
        assert_eq!(normalize_texture_relative_path(p4), None);
    }

    #[test]
    fn import_zip_with_manifest_and_files() {
        let root = temp_test_dir("zip_import");
        let manager = ModManager::new(&root);

        let mut zip_buffer = io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut zip_buffer);
            let opts = SimpleFileOptions::default();

            writer.start_file("gt4mod.toml", opts).unwrap();
            writer.write_all(b"schema = 1\nid = 'test-mod'\nname = 'Test Mod'\nversion = '1.0'\nkind = 'texture-pack'\ngame = 'SCUS-97328'\n").unwrap();

            writer.start_file("replacements/0123456789abcdef-00000001.png", opts).unwrap();
            writer.write_all(b"fake_png_data").unwrap();

            writer.start_file("readme.txt", opts).unwrap();
            writer.write_all(b"readme text").unwrap();

            writer.finish().unwrap();
        }

        zip_buffer.set_position(0);
        let report = manager.import_zip(zip_buffer, "test-mod").unwrap();
        assert_eq!(report.id, "test-mod");
        assert_eq!(report.texture_count, 1);
        assert_eq!(report.extras_count, 1);

        let store_dir = manager.store_dir().join("test-mod");
        assert!(store_dir.join("gt4mod.toml").is_file());
        assert!(store_dir.join("files").join("0123456789abcdef-00000001.png").is_file());
        assert!(store_dir.join("extras").join("readme.txt").is_file());
        assert!(store_dir.join("files.lock.json").is_file());

        let registry = ModRegistry::load_from_file(&manager.registry_file()).unwrap();
        assert_eq!(registry.mods.len(), 1);
        assert_eq!(registry.mods[0].id, "test-mod");
        assert!(registry.mods[0].enabled);
    }

    #[test]
    fn active_view_builder_resolves_conflicts_by_priority() {
        let root = temp_test_dir("active_view");
        let manager = ModManager::new(&root);

        // Mod A
        let store_a = manager.store_dir().join("mod-a");
        fs::create_dir_all(store_a.join("files")).unwrap();
        fs::write(store_a.join("files").join("shared.png"), b"content_a").unwrap();

        // Mod B
        let store_b = manager.store_dir().join("mod-b");
        fs::create_dir_all(store_b.join("files")).unwrap();
        fs::write(store_b.join("files").join("shared.png"), b"content_b").unwrap();
        fs::write(store_b.join("files").join("unique_b.png"), b"content_unique").unwrap();

        let mut registry = ModRegistry::default();
        registry.mods.push(ModRegistryEntry { id: "mod-a".into(), enabled: true });
        registry.mods.push(ModRegistryEntry { id: "mod-b".into(), enabled: true });
        registry.save_to_file(&manager.registry_file()).unwrap();

        let view_report = manager.build_active_view().unwrap();
        assert_eq!(view_report.active_textures, 2);
        assert_eq!(view_report.conflicts.len(), 1);
        assert_eq!(view_report.conflicts[0].winning_mod, "mod-a");
        assert_eq!(view_report.conflicts[0].overridden_mod, "mod-b");

        // Winning content must be Mod A's content
        let active_shared = manager.active_dir().join("shared.png");
        assert_eq!(fs::read(active_shared).unwrap(), b"content_a");

        // Unique B is also present
        let active_unique = manager.active_dir().join("unique_b.png");
        assert_eq!(fs::read(active_unique).unwrap(), b"content_unique");
    }
}
