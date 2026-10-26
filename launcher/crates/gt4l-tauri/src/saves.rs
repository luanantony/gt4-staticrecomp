//! The Saves and Replays pages and the card around each Play, on top of the `gt4saves` library.
//!
//! The library lives in the user data folder `%APPDATA%\GT4Recomp` (`Saves`, `Replays`; TASK-091, owner 2026-10-22;
//! 1.0.0 to 1.2.0 used `Documents\Gran Turismo 4`). Before a start the launcher writes the card the game reads
//! (`runtime\card.ps2`, in the large data folder) from the library; after the game exits it harvests that card back
//! into the library. So the game never writes into the library while it runs. Imports never overwrite;
//! deletes go to the Recycle Bin (see `recycle`).

use std::fs;
use std::path::Path;

use gt4l_core::guard;
use gt4l_core::paths::{DataDirs, UserDirs, UserRootSource};
use gt4saves::card::GAME_SAVE_NAME;
use gt4saves::library::{Library, LibraryPaths, Report, SLOT_PREFIX};
use gt4saves::migration::{self, LegacySources, Outcome, Targets};

use gt4l_platform::recycle;
use serde::Serialize;

/// The opened library and what the launcher log and the Saves page should say about it.
pub struct OpenedLibrary {
    pub library: Library,
    /// The player's folder in use (the Documents one, or the data-folder fallback).
    pub user: UserDirs,
    /// Lines for the launcher log (folder choice, the one-time move).
    pub log: Vec<String>,
    /// A sentence for the Saves page when the move to Documents did not happen as planned.
    pub notice: Option<String>,
}

/// Moves a library of launchers before 1.0.0 (`library\memcard` and friends in `pre_1_0_root`, the old
/// `%LOCALAPPDATA%\GT4Recomp`) into the library once (see `gt4saves::migration`), then opens the library. When the
/// library folder cannot be used, the data-folder fallback is used, so Play never fails on it.
pub fn open(dirs: &DataDirs, user: &UserDirs, pre_1_0_root: Option<&Path>) -> Result<OpenedLibrary, String> {
    let mut log = Vec::new();
    if let UserRootSource::Fallback(reason) = user.source() {
        log.push(format!("save library: WARNING {reason}; using {}", user.root().display()));
    }
    match open_at(user, pre_1_0_root, &mut log) {
        Ok((library, notice)) => Ok(OpenedLibrary { library, user: user.clone(), log, notice }),
        Err(error) if !matches!(user.source(), UserRootSource::Fallback(_)) => {
            let fallback = user.with_library_at(&dirs.fallback_user_root(), UserRootSource::Fallback(error.clone()));
            log.push(format!("save library: WARNING {} cannot be used ({error}); using {}", user.root().display(), fallback.root().display()));
            let (library, notice) = open_at(&fallback, pre_1_0_root, &mut log)?;
            let notice = notice.or_else(|| Some(format!("Saves are kept in {} because {} could not be used.", fallback.root().display(), user.root().display())));
            Ok(OpenedLibrary { library, user: fallback, log, notice })
        }
        Err(error) => Err(error),
    }
}

fn open_at(user: &UserDirs, pre_1_0_root: Option<&Path>, log: &mut Vec<String>) -> Result<(Library, Option<String>), String> {
    guard::protect_user_library(user.root());
    let mut notice = None;
    if let Some(old_root) = pre_1_0_root {
        let targets = Targets { saves: user.saves(), replays: user.replays() };
        let outcome = migration::migrate(
            &LegacySources::of_data_root(old_root),
            &targets,
            old_root,
            &user.library_migration_marker(),
            &gt4saves::timestamp(),
            &migration::copy_file,
        );
        log.push(outcome.describe(&targets));
        let old_library = old_root.join("library");
        notice = match &outcome {
            Outcome::Conflict { .. } => Some(format!(
                "An older save library is still in {}. It was not merged; the saves shown here are in {}.",
                old_library.display(),
                user.root().display()
            )),
            Outcome::Failed(_) => Some(format!(
                "Your saves could not be moved to {}. They are safe in {}; the launcher tries again at the next start.",
                user.root().display(),
                old_library.display()
            )),
            _ => None,
        };
    }
    let paths = LibraryPaths { saves: user.saves(), replays: user.replays(), backups: user.card_backups() };
    let library = Library::open_paths(&paths).map_err(|error| format!("save library: {error}"))?;
    Ok((library, notice))
}

/// One row of the Saves or Replays page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// Library name: `Save NN` for an inactive slot, the game's save name, or a replay name.
    pub name: String,
    pub label: String,
    /// The save the next start uses.
    pub active: bool,
    /// Last change of the item's folder, ISO 8601 UTC (empty when unknown).
    pub modified: String,
}

/// The page's item shape (`ui/js/pages/library.js`): `id` is the library name the page sends back to
/// `saves_activate`, `saves_delete` and `replays_delete`; `name` is the text shown.
impl Serialize for Item {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut row = serializer.serialize_struct("Item", 4)?;
        row.serialize_field("id", &self.name)?;
        row.serialize_field("name", &self.label)?;
        row.serialize_field("active", &self.active)?;
        row.serialize_field("modified", &self.modified)?;
        row.end()
    }
}

/// ISO 8601 time of the last change of `folder`, or an empty text.
fn modified_of(folder: &Path) -> String {
    fs::metadata(folder).and_then(|metadata| metadata.modified()).map(gt4saves::iso8601_utc).unwrap_or_default()
}

/// The active save first, then the inactive slots in slot order.
pub fn saves(library: &Library) -> Result<Vec<Item>, String> {
    let mut items = Vec::new();
    if library.active_save().map_err(|error| error.to_string())?.is_some() {
        items.push(Item {
            name: GAME_SAVE_NAME.to_string(),
            label: "Active save".to_string(),
            active: true,
            modified: modified_of(&library.active_dir().join(GAME_SAVE_NAME)),
        });
    }
    for slot in library.slots().map_err(|error| error.to_string())? {
        let modified = modified_of(&library.slot_dir(&slot).join(GAME_SAVE_NAME));
        items.push(Item { label: slot.clone(), name: slot, active: false, modified });
    }
    Ok(items)
}

/// Every replay the library keeps, in name order.
pub fn replays(library: &Library) -> Result<Vec<Item>, String> {
    Ok(library
        .replays()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|name| {
            let modified = modified_of(&library.replays_dir().join(&name));
            Item { label: name.clone(), name, active: false, modified }
        })
        .collect())
}

/// Imports a file the user picked: a `.psu` save, a card image (`.ps2`, `.mcd`, `.bin`) or a folder card.
pub fn import_path(library: &Library, path: &Path) -> Result<Report, String> {
    let result = if path.is_dir() {
        library.import_folder_card(path)
    } else {
        let extension = path.extension().map(|ext| ext.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
        match extension.as_str() {
            "psu" => library.import_psu(&bytes),
            "ps2" | "mcd" | "bin" | "card" => library.import_card_image(&bytes),
            other => return Err(format!("'.{other}' is not a save or card file")),
        }
    };
    result.map_err(|error| error.to_string())
}

/// Makes a slot the active save. The save that was active moves to a new slot.
pub fn activate(library: &Library, slot: &str) -> Result<(), String> {
    library.activate_slot(slot).map_err(|error| error.to_string())
}

/// Writes the card for the next start: the active save and every replay, read back before it is
/// written. A card the game left behind is harvested first, so nothing it wrote is lost; the old file
/// is also kept under `backups` by `write_card_image`.
pub fn prepare_card(library: &Library, runtime_card: &Path) -> Result<(), String> {
    if runtime_card.is_file() {
        if let Ok(bytes) = fs::read(runtime_card) {
            // An unreadable card is not an error here: its bytes are backed up when it is replaced.
            // Only harvest if an active save already exists, to avoid resurrecting an active save that was intentionally deleted.
            if library.active_save().map_err(|error| error.to_string())?.is_some() {
                let _ = library.harvest(&bytes);
            }
        }
    }
    let replay_names: Vec<String> = library.replays().map_err(|error| error.to_string())?;
    let image = library.build_card(&replay_names).map_err(|error| match error {
        gt4saves::library::LibraryError::Card(gt4saves::CardError::Full) => {
            "The memory card is full. Delete some replays on the Replays page.".to_string()
        }
        other => format!("the memory card could not be built: {other}"),
    })?;
    if let Some(parent) = runtime_card.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("runtime folder: {error}"))?;
    }
    library.write_card_image(runtime_card, &image).map_err(|error| error.to_string())
}

/// Copies the card the game wrote back into the library. A missing card means nothing was saved.
pub fn harvest_card(library: &Library, runtime_card: &Path) -> Result<Report, String> {
    if !runtime_card.is_file() {
        return Ok(Report::default());
    }
    let bytes = fs::read(runtime_card).map_err(|error| format!("{}: {error}", runtime_card.display()))?;
    library.harvest(&bytes).map_err(|error| error.to_string())
}

/// Short sentence about what an import did, for the Saves and Replays pages.
pub fn import_summary(report: &Report) -> String {
    let mut parts = Vec::new();
    if !report.added.is_empty() {
        parts.push(format!("Added {}.", report.added.join(", ")));
    }
    if !report.unchanged.is_empty() {
        parts.push("Already in the library.".to_string());
    }
    if !report.skipped.is_empty() {
        parts.push(format!("Skipped: {}.", report.skipped.join("; ")));
    }
    if parts.is_empty() {
        parts.push("Nothing to import.".to_string());
    }
    parts.join(" ")
}

/// Sends a library item to the Recycle Bin. Active save, inactive slots and replays can be deleted.
pub fn delete(library: &Library, item: &Item) -> Result<(), String> {
    let path = if item.active {
        let game_dir = library.active_dir().join(&item.name);
        if game_dir.is_dir() {
            game_dir
        } else {
            library.active_dir()
        }
    } else if item.name.starts_with(SLOT_PREFIX) {
        library.slot_dir(&item.name)
    } else {
        library.replays_dir().join(&item.name)
    };
    if !path.is_dir() {
        return Err(format!("{} is not in the library any more", item.label));
    }
    recycle::move_to_recycle_bin(&path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gt4saves::card::{DirEntry, Node, MODE_DIR_DEFAULT, MODE_FILE_DEFAULT};
    use gt4saves::Card;
    use std::path::PathBuf;

    /// The library of a scratch data folder, with a scratch stand-in for Documents (never the real one).
    fn open_scratch(data: &Path) -> (UserDirs, Library) {
        let dirs = DataDirs::new(data);
        let user = UserDirs::new(data.join("Documents").join("Gran Turismo 4"), UserRootSource::Override);
        let opened = open(&dirs, &user, Some(data)).unwrap();
        (opened.user, opened.library)
    }

    const STAMP: [u8; 8] = [0, 39, 50, 7, 12, 10, 0xEA, 0x07];
    const REPLAY: &str = "BASCUS-97328R0001";

    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("gt4l-saves-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn file(name: &str, data: Vec<u8>) -> Node {
        Node::file(DirEntry::new(name, MODE_FILE_DEFAULT, STAMP, STAMP).unwrap(), data)
    }

    fn directory(name: &str, children: Vec<Node>) -> Node {
        Node::dir(DirEntry::new(name, MODE_DIR_DEFAULT, STAMP, STAMP).unwrap(), children)
    }

    /// A synthetic save: a few files with `seed` in their contents, so two saves differ.
    fn game_save(seed: u8) -> Node {
        let pattern = |len: usize| (0..len).map(|i| (i as u8).wrapping_mul(7).wrapping_add(seed)).collect();
        directory(GAME_SAVE_NAME, vec![file("icon.sys", pattern(964)), file("garage", pattern(20_000))])
    }

    fn replay() -> Node {
        directory(REPLAY, vec![file("icon.sys", vec![1; 964]), file("replay.rep", vec![9; 5_000])])
    }

    /// A card image holding the given items, as the game would leave it.
    fn card_with(items: &[Node]) -> Vec<u8> {
        Card::build(items).unwrap().to_image()
    }

    fn names(items: &[Item]) -> Vec<String> {
        items.iter().map(|item| item.name.clone()).collect()
    }

    #[test]
    fn a_game_card_is_harvested_and_the_next_card_is_built_from_the_library() {
        let data = scratch("harvest");
        let (user, library) = open_scratch(&data);
        let runtime_card = data.join("runtime").join("card.ps2");
        fs::create_dir_all(runtime_card.parent().unwrap()).unwrap();
        fs::write(&runtime_card, card_with(&[game_save(1), replay()])).unwrap();

        let report = harvest_card(&library, &runtime_card).unwrap();
        assert!(report.added.contains(&GAME_SAVE_NAME.to_string()));
        assert_eq!(names(&saves(&library).unwrap()), vec![GAME_SAVE_NAME.to_string()]);
        assert_eq!(names(&replays(&library).unwrap()), vec![REPLAY.to_string()]);

        fs::remove_file(&runtime_card).unwrap();
        prepare_card(&library, &runtime_card).unwrap();
        let built = Card::from_image(&fs::read(&runtime_card).unwrap()).unwrap().root().unwrap();
        let built_names: Vec<String> = built.iter().map(|node| node.name()).collect();
        assert_eq!(built_names, vec![GAME_SAVE_NAME.to_string(), REPLAY.to_string()]);
        assert!(user.saves().join("Active").join(GAME_SAVE_NAME).is_dir(), "the save is in Documents\\Gran Turismo 4\\Saves");
        assert!(user.replays().join(REPLAY).is_dir());
        assert!(!data.join("library").join("memcard").exists(), "nothing is written to the old AppData library");
        let _ = fs::remove_dir_all(&data);
    }

    #[test]
    fn an_import_never_overwrites_and_identical_saves_are_not_repeated() {
        let data = scratch("import");
        let (_, library) = open_scratch(&data);
        let first = data.join("first.ps2");
        fs::write(&first, card_with(&[game_save(1)])).unwrap();
        let second = data.join("second.ps2");
        fs::write(&second, card_with(&[game_save(2)])).unwrap();

        assert_eq!(import_path(&library, &first).unwrap().added, vec!["Save 01".to_string()]);
        // The same save again is recognised and nothing new is written.
        let again = import_path(&library, &first).unwrap();
        assert!(again.added.is_empty());
        assert_eq!(again.unchanged, vec![GAME_SAVE_NAME.to_string()]);
        // A different save becomes a new slot; the active save is untouched.
        assert_eq!(import_path(&library, &second).unwrap().added, vec!["Save 02".to_string()]);
        assert_eq!(names(&saves(&library).unwrap()), vec!["Save 01".to_string(), "Save 02".to_string()]);
        assert!(saves(&library).unwrap().iter().all(|item| !item.active));
        let _ = fs::remove_dir_all(&data);
    }

    #[test]
    fn activating_a_slot_parks_the_previous_save() {
        let data = scratch("activate");
        let (_, library) = open_scratch(&data);
        let runtime_card = data.join("runtime").join("card.ps2");
        let source = data.join("source.ps2");
        fs::write(&source, card_with(&[game_save(3)])).unwrap();
        import_path(&library, &source).unwrap();
        // Make the imported save active: the library had no active save, so the slot becomes it.
        activate(&library, "Save 01").unwrap();
        let before = saves(&library).unwrap();
        assert_eq!(names(&before), vec![GAME_SAVE_NAME.to_string()]);
        assert!(before[0].active && before[0].label == "Active save");

        let other = data.join("other.ps2");
        fs::write(&other, card_with(&[game_save(4)])).unwrap();
        import_path(&library, &other).unwrap();
        activate(&library, "Save 01").unwrap();
        let after = names(&saves(&library).unwrap());
        assert_eq!(after, vec![GAME_SAVE_NAME.to_string(), "Save 02".to_string()]);
        prepare_card(&library, &runtime_card).unwrap();
        let _ = fs::remove_dir_all(&data);
    }

    #[test]
    fn deleting_the_active_save_clears_the_active_save() {
        let data = scratch("delete-active");
        let (_, library) = open_scratch(&data);
        let card = data.join("card.ps2");
        fs::write(&card, card_with(&[game_save(5), replay()])).unwrap();
        harvest_card(&library, &card).unwrap();
        let active = Item { name: GAME_SAVE_NAME.into(), label: "Active save".into(), active: true, modified: String::new() };
        assert!(delete(&library, &active).is_ok());
        assert!(!library.active_dir().join(GAME_SAVE_NAME).is_dir(), "deleted active save is gone");
        assert!(library.active_save().unwrap().is_none());
        let _ = fs::remove_dir_all(&data);
    }

    #[test]
    fn prepare_card_does_not_resurrect_deleted_active_save() {
        let data = scratch("no-resurrect");
        let (_, library) = open_scratch(&data);
        let runtime_card = data.join("runtime").join("card.ps2");
        fs::create_dir_all(runtime_card.parent().unwrap()).unwrap();
        fs::write(&runtime_card, card_with(&[game_save(6), replay()])).unwrap();

        // With no active save in the library, prepare_card should NOT harvest the save from runtime_card.
        prepare_card(&library, &runtime_card).unwrap();
        assert!(library.active_save().unwrap().is_none(), "deleted active save was not resurrected");
        let built = Card::from_image(&fs::read(&runtime_card).unwrap()).unwrap().root().unwrap();
        let built_names: Vec<String> = built.iter().map(|node| node.name()).collect();
        assert_eq!(built_names, vec![REPLAY.to_string()], "runtime card was overwritten without the game save");
        let _ = fs::remove_dir_all(&data);
    }
}

#[cfg(test)]
mod migration_tests {
    use super::*;
    use gt4saves::card::{DirEntry, Node, MODE_DIR_DEFAULT, MODE_FILE_DEFAULT};
    use gt4saves::Card;
    use std::path::PathBuf;

    const STAMP: [u8; 8] = [0, 39, 50, 7, 12, 10, 0xEA, 0x07];

    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("gt4l-saves-move-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn old_save(data: &Path) -> Node {
        let file = Node::file(DirEntry::new("garage", MODE_FILE_DEFAULT, STAMP, STAMP).unwrap(), vec![7; 4_000]);
        let save = Node::dir(DirEntry::new(GAME_SAVE_NAME, MODE_DIR_DEFAULT, STAMP, STAMP).unwrap(), vec![file]);
        let memcard = data.join("library").join("memcard");
        fs::create_dir_all(&memcard).unwrap();
        gt4saves::folder::export_item(&memcard, &save).unwrap();
        save
    }

    #[test]
    fn the_first_start_moves_the_old_library_to_the_players_folder() {
        let data = scratch("first-start");
        let save = old_save(&data);
        let dirs = DataDirs::new(&data);
        let user = UserDirs::new(data.join("Docs").join("Gran Turismo 4"), UserRootSource::Override);
        let opened = open(&dirs, &user, Some(&data)).unwrap();
        assert!(opened.notice.is_none());
        assert!(opened.log.iter().any(|line| line.contains("moved 3 files")), "{:?}", opened.log);
        assert_eq!(saves(&opened.library).unwrap()[0].label, "Active save");
        // Play builds the card from the moved save.
        let card = dirs.runtime_card();
        prepare_card(&opened.library, &card).unwrap();
        let built = Card::from_image(&fs::read(&card).unwrap()).unwrap().root().unwrap();
        assert!(gt4saves::card::same_contents(&built, std::slice::from_ref(&save)));
        let parked: Vec<_> = fs::read_dir(&data).unwrap().filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().starts_with("library.migrated-")).collect();
        assert_eq!(parked.len(), 1, "the old folders are parked, not deleted");
        // The guard now refuses the player's folder for install and update writes.
        assert!(gt4l_core::guard::check_write(&data, &user.saves().join("x")).is_err());
        let _ = fs::remove_dir_all(&data);
    }

    #[test]
    fn an_unusable_players_folder_falls_back_to_the_data_folder() {
        let data = scratch("fallback");
        old_save(&data);
        let dirs = DataDirs::new(&data);
        // A file where the folder should be: the library cannot be made there.
        let blocked = data.join("blocked-docs");
        fs::write(&blocked, b"not a folder").unwrap();
        let user = UserDirs::new(blocked.join("Gran Turismo 4"), UserRootSource::AppData);
        let opened = open(&dirs, &user, Some(&data)).unwrap();
        assert_eq!(opened.user.root(), data.join("Gran Turismo 4"));
        assert!(opened.notice.is_some());
        assert_eq!(saves(&opened.library).unwrap().len(), 1, "the old save was moved to the fallback folder");
        let _ = fs::remove_dir_all(&data);
    }
}

#[cfg(test)]
mod page_shape_tests {
    use super::Item;

    #[test]
    fn an_item_has_the_fields_the_page_reads() {
        let item = Item { name: "Save 02".into(), label: "Save 02".into(), active: false, modified: "2026-10-21T12:00:00Z".into() };
        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json, serde_json::json!({ "id": "Save 02", "name": "Save 02", "active": false, "modified": "2026-10-21T12:00:00Z" }));
    }
}
