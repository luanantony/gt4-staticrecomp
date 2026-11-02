# TASK-054 — Launcher Mods Page and Mod Manager UI

**Task:** TASK-054  
**Date:** 2026-11-01  
**Status:** Completed  
**Reference Document:** `docs/launcher/MOD-MANAGER-PLAN.md` (sections 4.1, 4.2)  
**Clean-room Status:** Fully compliant (No GPL code, pure MIT launcher and safe Tauri 2 architecture)

---

## 1. Overview and Intent

Following the implementation of the core mod engine in `gt4l-core` (TASK-052), TASK-054 delivers the user-facing **Mods Manager** within the GT4Recomp Tauri 2 launcher.

Inspired by Assetto Corsa\'s *Content Manager*, texture packs and user modifications are treated not merely as hidden graphics toggle options, but as distinct modular assets that players can install, prioritize, inspect, enable/disable, and remove with complete safety.

---

## 2. Mockup Specification (`mockup-mods.html`)

Located at [`launcher/crates/gt4l-tauri/ui/dev/mockup-mods.html`](file:///c:/Users/Antony/Desktop/gt4-staticrecomp/launcher/crates/gt4l-tauri/ui/dev/mockup-mods.html), the interactive mockup provides visual verification across all required operational states:

1. **Empty State:**
   - Visual dash-bordered dropzone card when no mods are installed.
   - Clear onboarding instruction for drag-and-drop or placing zip archives in the `Inbox/` folder.
2. **List State (Default):**
   - Ordered table showing priority `#1`, `#2`, ... (top priority wins in case of duplicate texture hashes).
   - Instant enable/disable checkbox toggle.
   - Mod metadata: Title, version badge, author, category (`Texture Pack`), disk usage, and total texture count.
   - Conflict chip warning for mods whose textures are partially overridden by higher-priority packs.
   - Quick action buttons: Move Up, Move Down, Details (`ℹ`), Remove (`✕`).
3. **Details Drawer:**
   - Slide-in inspector panel displaying mod thumbnail/banner preview, comprehensive description, format breakdown (BC7 DDS vs PNG, mipmaps), and physical store path.
4. **Import Progress:**
   - Modal overlay displaying extraction and SHA-256 verification progress with percentage bar, file counters, and cancel button.
5. **Conflict Resolution Modal:**
   - Dedicated dialog explaining duplicate texture key overrides, detailing winning vs overridden textures.
6. **Remove Confirmation:**
   - Safety dialog warning that the mod will be moved to the Windows Recycle Bin rather than permanently erased.
7. **Wrong Renderer Warning Banner:**
   - Contextual warning alerting players that texture replacements require the PCSX2 GS (`gt4-gs.exe`) renderer, as CoreGS 1.1 does not support texture packs.

---

## 3. Tauri Commands and Architecture

Exposed through [`launcher/crates/gt4l-tauri/src/commands.rs`](file:///c:/Users/Antony/Desktop/gt4-staticrecomp/launcher/crates/gt4l-tauri/src/commands.rs):

- `mods_list() -> Result<ModsView, String>`:
  Returns the ordered list of installed mods, active texture counts, store directory, and inbox folder path.
- `mods_import(path: Option<String>) -> Result<Option<ImportReport>, String>`:
  Accepts a path directly (from file drag-and-drop) or triggers the native OS file picker for `.zip` archives. Performs safe decompression, manifest validation, stores into `mods/store/<id>`, and automatically rebuilds the hardlinked active view.
- `mods_set_enabled(id: String, enabled: bool) -> Result<bool, String>`:
  Toggles mod activation and triggers an atomic active view rebuild.
- `mods_move(id: String, new_priority: usize) -> Result<bool, String>`:
  Reorders mod priority ranking and rebuilds the active view.
- `mods_remove(id: String) -> Result<bool, String>`:
  Uninstalls the mod, removes from `mods.toml`, cleans up disk storage, and refreshes the active view.
- `mods_details(id: String) -> Result<Option<ModDetails>, String>`:
  Fetches full metadata and conflict statistics for a given mod ID.
- `open_mods_folder() -> Result<(), String>`:
  Opens `<data>\mods` in Windows Explorer.
- `open_inbox_folder() -> Result<(), String>`:
  Opens `<data>\mods\inbox` in Windows Explorer.

---

## 4. Gamepad & Accessibility Controls

In accordance with GT4Recomp\'s single-hand and controller-first navigation design:

| Controller (SDL3 / XInput) | Keyboard | Action |
| :--- | :--- | :--- |
| **D-Pad / Left Stick** | $\uparrow$ / $\downarrow$ | Navigate through installed mods list |
| **A / South (Cross)** | Space / Enter | Toggle mod enabled / disabled |
| **Y / North (Triangle)** | `I` | Open Mod Details drawer |
| **L1 / R1** | `Q` / `E` | Increase or decrease mod priority rank |
| **X / West (Square)** | Delete | Uninstall / remove mod |
| **Start** | `Ctrl + O` | Trigger Install from file dialog |
| **B / East (Circle)** | Escape | Return to Home / previous screen |

---

## 5. Verification and Acceptance

- All 110 `gt4l-core` unit tests pass, verifying:
  - Mod import roundtrips from `.zip`
  - Safe extraction defenses against path traversal (`..`), device names, and alternate data streams
  - Dynamic listing, details queries, priority reordering, and removal
  - Hardlink active view construction and priority conflict resolution
- Interactive UI mockup verified at `launcher/crates/gt4l-tauri/ui/dev/mockup-mods.html`.
- Zero compiler warnings or security regressions.
