//! Custom textures (TASK-088, gt4-gs 1.2.0; TASK-091 moved the folder to `<install>\data\Textures`): the Textures folder, the folder the launcher
//! makes on demand (TASK-093: no replacements subfolder), and the two Graphics rows (Load custom textures on by default, Precache off by default).

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use gt4l_core::gs_table;
use gt4l_core::paths::{ensure_texture_folders, DataDirs};

fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("gt4l-textures-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn the_textures_folder_is_inside_the_data_folder_with_no_serial_folder() {
    let install = scratch("root");
    let dirs = DataDirs::new(install.join("data"));
    assert_eq!(dirs.textures(), install.join("data").join("Textures"));
    assert!(!dirs.textures().exists(), "resolving creates nothing");
}

#[test]
fn a_textures_folder_not_moved_yet_stays_in_use() {
    let base = scratch("in-place");
    let mut dirs = DataDirs::new(base.join("data"));
    let documents = base.join("Documents").join("Gran Turismo 4").join("Textures");
    dirs.set_textures_in_place(Some(documents.clone()));
    assert_eq!(dirs.textures(), documents);
}

#[test]
fn the_launcher_makes_the_textures_folder_on_demand_and_no_replacements_folder() {
    let textures = scratch("ensure").join("Gran Turismo 4").join("Textures");
    ensure_texture_folders(&textures).unwrap();
    assert!(textures.is_dir());
    assert!(!textures.join("replacements").exists(), "gt4-gs 1.2.1 reads Textures itself");
    // Idempotent: a second call keeps what is there.
    fs::write(textures.join("pack.txt"), b"kept").unwrap();
    ensure_texture_folders(&textures).unwrap();
    assert_eq!(fs::read(textures.join("pack.txt")).unwrap(), b"kept");
}

#[test]
fn load_custom_textures_is_on_by_default_and_always_written() {
    let row = gs_table::row("texture_load").unwrap();
    assert_eq!((row.page.as_str(), row.default.as_str(), row.written.as_str(), row.status.as_str()), ("basic", "true", "always", "ok"));
    assert!(gs_table::offered().iter().any(|offered| offered.id == "texture_load"));
    let text = gs_table::render_ini(&BTreeMap::new(), &BTreeMap::new()).unwrap();
    assert!(text.contains("LoadTextureReplacements = true\n"), "{text}");
    let off = BTreeMap::from([("texture_load".to_string(), "false".to_string())]);
    let text = gs_table::render_ini(&off, &BTreeMap::new()).unwrap();
    assert!(text.contains("LoadTextureReplacements = false\n"), "{text}");
}

#[test]
fn precache_is_hidden_off_by_default_and_written_only_when_on() {
    // TASK-118 (owner, 2026-10-26): the row left the page; the key is still written when a stored value turns it on.
    let row = gs_table::row("texture_precache").unwrap();
    assert_eq!((row.page.as_str(), row.default.as_str(), row.written.as_str()), ("hidden", "false", "when-set"));
    assert!(!gs_table::offered().iter().any(|offered| offered.id == "texture_precache"));
    let text = gs_table::render_ini(&BTreeMap::new(), &BTreeMap::new()).unwrap();
    assert!(!text.contains("PrecacheTextureReplacements"), "{text}");
    let on = BTreeMap::from([("texture_precache".to_string(), "true".to_string())]);
    let text = gs_table::render_ini(&on, &BTreeMap::new()).unwrap();
    assert!(text.contains("PrecacheTextureReplacements = true\n"), "{text}");
}

#[test]
fn texture_dumping_is_not_offered() {
    for id in ["texture_dump", "texture_dump_mipmaps", "texture_dump_fmv"] {
        assert!(!gs_table::offered().iter().any(|offered| offered.id == id), "{id} is not offered");
    }
}

#[test]
fn the_textures_folder_is_written_only_when_the_launcher_supplies_it() {
    let launcher = BTreeMap::from([("textures_folder".to_string(), "C:/Users/x/Documents/Gran Turismo 4/Textures".to_string())]);
    let text = gs_table::render_ini(&BTreeMap::new(), &launcher).unwrap();
    assert!(text.contains("[Folders]\nTextures = C:/Users/x/Documents/Gran Turismo 4/Textures\n"), "{text}");
}
