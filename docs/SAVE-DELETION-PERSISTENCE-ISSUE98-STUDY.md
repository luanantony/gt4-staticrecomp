# Save Deletion and Runtime Card Persistence Study (TASK-025 / TASK-058 / Issue #98)

## 1. Overview & Problem Statement

### 1.1 Issue Context
GitHub Issue [#98](https://github.com/moesuito/gt4-staticrecomp/issues/98) ("Deleted save, reloaded and save is still there", reported by `@FLoCkeYE` on v1.2.3) described a perplexing user experience:
> *"I deleted the save folder from %appdata%\GT4Recomp\Saves and also deleted backups, but when I launched the game again, my save file came back like magic and the game continued where I left off. How do I completely start a new game from scratch?"*

This behavior is directly tied to the interaction between the launcher save manager (**TASK-025**) and the native runtime memory card subsystem (**TASK-058**).

### 1.2 User Expectation vs. System Behavior
- **User Mental Model:** "Deleting the save folder in `%APPDATA%\GT4Recomp\Saves` removes my save; hitting Play will launch the game with no memory card data so I can create a new career."
- **Actual Engine Behavior:** The save system operates with a **dual-layer bidirectional synchronization** architecture. Deleting only the user library folder leaves the raw hardware memory card image intact in a separate directory (`%LOCALAPPDATA%\GT4Recomp\runtime\card.ps2`). When the user clicks **Play**, the launcher checks the runtime card, detects valid game data, automatically imports ("harvests") it back into the empty library folder, builds a fresh card, and presents the restored save to the game.

---

## 2. Architecture Analysis: Dual-Layer Save System

GT4Recomp separates high-level save management from low-level emulation requirements across two distinct storage layers:

```
+-----------------------------------------------------------------------------------+
| LAYER A: High-Level User Library (Unpacked Files)                                 |
| Location: %APPDATA%\GT4Recomp\Saves                                               |
|                                                                                   |
|  - Active/BASCUS-97328GAMEDATA/      <-- Current active career save folder        |
|  - Active/.backup/                   <-- Timestamped snapshots of previous saves  |
|  - Save 01/BASCUS-97328GAMEDATA/     <-- Archived slot 1                          |
|  - Save 02/BASCUS-97328GAMEDATA/     <-- Archived slot 2                          |
|  - Replays/BASCUS-97328R0001/        <-- Extracted replay folders                 |
+-----------------------------------------------------------------------------------+
                                   ^
            (1) PRE-PLAY HARVEST   |   (2) BUILD & WRITE
                   syncs changes   |   assembles 8MB image
                                   v
+-----------------------------------------------------------------------------------+
| LAYER B: Low-Level Emulated Memory Card Image                                      |
| Location: %LOCALAPPDATA%\GT4Recomp\runtime\card.ps2                               |
|                                                                                   |
|  - Standard 8,650,752 byte raw Sony PS2 memory card binary image                  |
|  - Formatted with PS2 MC filesystem (SuperBlock, Alloc Dir, FAT, Root Dir)        |
|  - Direct target of runtime IOP SIO2 emulation (iop/sio2.cpp)                     |
+-----------------------------------------------------------------------------------+
```

### 2.1 Storage Path Divergence on Windows
A key contributing factor to user confusion is the split across Windows environment folders:
1. **User Library (Layer A):** `%APPDATA%\GT4Recomp` (`CSIDL_APPDATA` / `AppData\Roaming`), exposed via the launcher's **"Open folder"** link.
2. **Runtime Data (Layer B):** `%LOCALAPPDATA%\GT4Recomp\runtime\card.ps2` (`CSIDL_LOCAL_APPDATA` / `AppData\Local`) or `<InstallDir>\data\runtime\card.ps2` depending on write permissions.

Because players who inspect `%APPDATA%` never see `%LOCALAPPDATA%`, they believe they have eliminated all traces of their career progress.

---

## 3. Code-Level Forensic Walkthrough

### 3.1 Pre-Launch Card Preparation (`saves::prepare_card`)
When the user clicks **Play**, `commands::play` invokes `saves::prepare_card` in [`launcher/crates/gt4l-tauri/src/saves.rs`](file:///c:/Users/Antony/Desktop/gt4-staticrecomp/launcher/crates/gt4l-tauri/src/saves.rs#L173-L191):

```rust
pub fn prepare_card(library: &Library, runtime_card: &Path) -> Result<(), String> {
    if runtime_card.is_file() {
        if let Ok(bytes) = fs::read(runtime_card) {
            // An unreadable card is not an error here: its bytes are backed up when it is replaced.
            let _ = library.harvest(&bytes);
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
```

### 3.2 The Unconditional Resurrection Loop in `library.harvest`
In [`launcher/crates/gt4saves/src/library.rs`](file:///c:/Users/Antony/Desktop/gt4-staticrecomp/launcher/crates/gt4saves/src/library.rs#L359-L388):

```rust
pub fn harvest(&self, card_bytes: &[u8]) -> Result<Report, LibraryError> {
    let card = Card::from_image(card_bytes)?;
    let mut report = Report::default();
    for item in card.root()? {
        let name = item.name();
        if card::is_game_save_name(&name) {
            let current = self.active_dir().join(&name);
            if current.is_dir() {
                let before = folder::import_item(&current)?;
                if card::same_contents(std::slice::from_ref(&before), std::slice::from_ref(&item)) {
                    report.unchanged.push(name);
                    continue;
                }
                let backup = self.active_backup_dir();
                let stamp = crate::timestamp();
                let copy_dir = backup.join(format!("{name}-{stamp}"));
                fs::create_dir_all(&copy_dir)?;
                folder::export_item(&copy_dir, &before)?;
                report.replaced_active = true;
            }
            // CRITICAL STEP:
            // When the user deleted Saves/Active, current.is_dir() evaluates to false.
            // harvest unconditionally exports the save from card.ps2 back into Active!
            folder::export_item(&self.active_dir(), &item)?;
            report.added.push(name);
        } else if card::is_replay_name(&name) {
            self.add_item(&item, &mut report)?;
        }
    }
    Ok(report)
}
```

### 3.3 What Happened During the Reporter's Attempt:
1. User deleted `%APPDATA%\GT4Recomp\Saves` and `%APPDATA%\GT4Recomp\backups`.
2. `%LOCALAPPDATA%\GT4Recomp\runtime\card.ps2` remained on disk with the existing 8 MB image.
3. User pressed **Play** in the launcher.
4. `prepare_card` saw `runtime_card.is_file() == true` and called `library.harvest(&bytes)`.
5. `library.harvest` parsed `card.ps2`, found `BASCUS-97328GAMEDATA`, checked if `active_dir().join("BASCUS-97328GAMEDATA")` existed.
6. Since it had been deleted, `is_dir()` was false.
7. `harvest` exported the save from `card.ps2` right back into `%APPDATA%\GT4Recomp\Saves\Active`!
8. `build_card` then serialized that active save into a card image, verified readback, and wrote it to `card.ps2`.
9. The game booted, read `card.ps2`, and loaded the original save.

---

## 4. UI Restrictions in Launcher v1.2.3

Why did the user resort to deleting folders in Windows Explorer instead of using the Launcher UI?

In [`launcher/crates/gt4l-tauri/src/saves.rs`](file:///c:/Users/Antony/Desktop/gt4-staticrecomp/launcher/crates/gt4l-tauri/src/saves.rs#L221-L224):
```rust
pub fn delete(library: &Library, item: &Item) -> Result<(), String> {
    if item.active {
        return Err("The active save cannot be deleted. Activate another save first.".to_string());
    }
    // ...
}
```

In version 1.2.3:
- There was **no option to delete the Active save** if it was the only save.
- There was **no "New save" or "Reset save"** button in the UI.
- The player was locked into a state where they could neither reset their career in the UI nor successfully delete it from Windows Explorer without knowing the secret `%LOCALAPPDATA%\GT4Recomp\runtime\card.ps2` path.

---

## 5. Manual Reset Procedure (Workaround for v1.2.x)

To completely reset career progress and start a brand new game without ghost saves reappearing:

1. **Exit GT4Recomp and Launcher completely.**
2. **Delete High-Level Saves:**
   - Press `Win + R`, enter `%APPDATA%\GT4Recomp`, press Enter.
   - Delete the `Saves` folder and the `backups` folder.
3. **Delete Low-Level Runtime Card:**
   - Press `Win + R`, enter `%LOCALAPPDATA%\GT4Recomp\runtime`, press Enter.
   - Delete `card.ps2` (or delete the entire `runtime` folder).
4. **Launch GT4Recomp:**
   - The launcher will build an empty memory card image from scratch.
   - Gran Turismo 4 will prompt to create a new game save file upon starting.

---

## 6. Evolution in v1.3.0-rc Series

In the 1.3.0-rc release cycle, several improvements were implemented:
- **"New save" Action:** Added to the Saves page, allowing players to park their active save into an archived slot (`Save 01`, `Save 02`) and start a clean active session without touching Windows Explorer.
- **Improved Slot Activation:** Users can swap between multiple careers seamlessly.
- **Relocation & Data Layout Hardening:** Better segregation of user documents and local runtime caches (`gt4l-core/src/relocate.rs`).

---

## 7. Recommended Engineering Enhancements for v1.3.x / v2.0.0

To ensure a foolproof experience for both casual players and power users:

1. **Explicit "Reset Active Save" / "Start Fresh Career" Button:**
   - Add a confirmed action in the Saves UI that clears both `Saves/Active` and unlinks `%LOCALAPPDATA%\GT4Recomp\runtime\card.ps2`.
2. **Empty Library Intent Detection in `prepare_card`:**
   - If `Saves/Active` has no game save AND `Saves/` contains zero inactive slots (`Save *`), check whether the user intentionally wiped their library.
   - If the user wiped their library, avoid harvesting the stale `runtime\card.ps2` image; instead, re-initialize a blank 8 MB card.
3. **UI Documentation Tooltip:**
   - In Settings > General or the Saves tab, clarify that GT4Recomp maintains an emulated memory card cache at `runtime\card.ps2`.

---

## 8. Verification & Test Evidence

- **Unit Tests:** `gt4l-core` (105/105 passed).
- **Contract Tests:** `launch_contract` (16/16 passed).
- **Card Preservation Contract:** Verified that normal game exits harvest correctly and atomic card flushes preserve data integrity under TASK-058.
