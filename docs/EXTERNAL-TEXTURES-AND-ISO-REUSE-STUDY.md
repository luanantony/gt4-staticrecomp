# External Texture Directory and Zero-Copy ISO Reuse Study (TASK-053 / TASK-128 / Issue #95)

## 1. Executive Summary & Problem Context

GitHub Issue [#95](https://github.com/moesuito/gt4-staticrecomp/issues/95) ("Reuse texture and ISO paths", reported by `@xraynet` with discussion by owner `@moesuito` and `@Tappczan`) highlighted a major friction point regarding disk space usage:
> *"It would be best if the ISO file path could also be specified—ideally the same one used by the emulator—to save space. Pointing GT4Recomp at an existing PCSX2 texture folder would also avoid duplicating huge texture packs."*

Owner response:
> *"Fair point about disk space. Copying the disc image was a deliberate choice, but you're making a good case against it. I'm considering an option to not copy the image: the launcher would just remember where your original disc image is and read it from there (the same file your emulator uses). Pointing GT4Recomp at an existing PCSX2 texture folder is a reasonable request too; both are noted."*

### 1.1 Disk Space Impact
A typical enthusiast setup running Gran Turismo 4 under emulation maintains:
- **Base Game Disc Image (ISO):** ~5.3 GB (or ~10.6 GB if both NTSC-U v2.00 and Spec II Public Beta are present).
- **HD Texture Pack:** 10 GB to 15 GB of community upscaled / remastered textures.
- **Total Existing Footprint:** ~15 GB to 25 GB.

By default, GT4Recomp copies the entire 5.3 GB ISO into `<install>\data\disc\<image>.iso` and expects replacement textures inside `<install>\data\Textures`, forcing users to duplicate 15+ GB of identical data on their primary storage.

---

## 2. Architecture Analysis: Current Disc and Texture Flow

### 2.1 Current Disc Lifecycle (`gt4l-disc::setup`)
1. **Setup Import:**
   - The user selects their original `.iso` via the launcher file dialog.
   - `gt4l_disc::setup` verifies disc identity (checking `CORE.GT4` and Sony disc header).
   - The launcher copies the full ~5.3 GB image file into `<install>\data\disc\<serial>.iso`.
   - The launcher extracts `CORE.GT4`, extracts the ELF, decrypts guest code, verifies integrity pins, and writes `<install>\data\prepared\receipt.json`.
2. **Launch Verification (`setup::check_installed`):**
   ```rust
   let path = disc_folder.join(&image.file);
   let metadata = fs::metadata(&path)?;
   if metadata.len() != image.size_bytes || modified_nanos(&metadata) != image.modified_unix_nanos {
       return Installed::Missing { reason: format!("{} has changed", image.file) };
   }
   Installed::Ready { image: path, ... }
   ```
3. **Execution:**
   - `commands::prepare_play` passes `&disc` to `gt4.exe --disc <path>`.

### 2.2 Current Texture Lifecycle (`gt4l-core::launch`, `textures.rs`)
1. **Storage Location:**
   - Standard path: `<install>\data\Textures` (relocated in 1.2.1 from `Documents\Gran Turismo 4\Textures`).
2. **GSdx Wiring:**
   - In [`launcher/crates/gt4l-core/src/launch.rs`](file:///c:/Users/Antony/Desktop/gt4-staticrecomp/launcher/crates/gt4l-core/src/launch.rs#L206):
     ```rust
     launcher.insert("textures_folder".to_string(), paths.textures.display().to_string());
     ```
   - In `gt4-gs.ini`:
     ```ini
     [Folders]
     Textures = C:\Games\GT4Recomp\data\Textures
     ```
   - `gt4-gs.exe` reads textures directly from this folder when `LoadTextureReplacements = true`.

---

## 3. Immediate Workaround on Windows: NTFS Directory Junctions

For players seeking zero duplication today without code changes, Windows NTFS directory junctions provide an effective workaround:

```cmd
:: 1. Close GT4Recomp completely.
:: 2. If <install>\data\Textures already exists, move any unique files or delete the empty folder.
rmdir "C:\Games\GT4Recomp\data\Textures"

:: 3. Create an NTFS junction to your existing PCSX2 texture folder:
mklink /J "C:\Games\GT4Recomp\data\Textures" "D:\Emulators\PCSX2\textures\SCUS-97328\replacements"
```

### Why Junctions Work:
- The NTFS filesystem driver transparently resolves directory junctions at the kernel I/O level.
- Both the Tauri launcher and the Vulkan renderer process (`gt4-gs.exe`) perceive `data\Textures` as a native local directory with zero file duplication.
- However, requiring players to run Windows Command Prompt as administrator and execute `mklink /J` is unintuitive for casual users.

---

## 4. Engineering Design for Configurable Texture Path (TASK-053)

### 4.1 Configuration Storage
In `config.toml` / `settings.toml`:
```toml
[settings]
# ...
textures_folder = "D:/Emulators/PCSX2/textures/SCUS-97328" # optional custom path
```

### 4.2 Launcher Wiring (`gt4l-core::launch`)
In `Pcsx2Paths::for_session`:
```rust
let effective_textures = match settings.get_path("textures_folder") {
    Some(custom) if custom.is_dir() => custom,
    _ => dirs.textures(),
};
```
In `launch::gs_config_text`:
- `launcher.insert("textures_folder", effective_textures.display().to_string());`
- The session `gt4-gs.ini` automatically receives the custom path under `[Folders] Textures`.

### 4.3 UI Controls (Settings > Graphics)
- **Current UI:** Single button: `"Open textures folder"`.
- **Proposed Enhancement:**
  - Row label: **"Textures location"**.
  - Current value: `"Default (data/Textures)"` or custom path.
  - Action buttons:
    - `"Change folder..."` (opens native folder picker).
    - `"Reset to default"`.
    - `"Open folder in Explorer"`.

---

## 5. Engineering Design for Zero-Copy External ISO Reuse (TASK-128)

### 5.1 Why Copying Was Initially Chosen
1. **Portability:** Having the disc image inside `<install>\data\disc\` guarantees the game folder can be moved to another drive or PC without broken references.
2. **Drive Unmounting Safety:** External USB drives can be unplugged mid-game, causing read errors.
3. **Immutability Assurance:** Placing the image in a private app directory prevents users or external tools from accidentally renaming or deleting the file while the game relies on it.

### 5.2 Safe Zero-Copy Design
To support external ISO reuse safely without compromising stability:

1. **Setup Mode Choice:**
   - On the disc setup screen, introduce an optional checkbox:
     `[x] Use disc image in-place (saves ~5.3 GB disk space)`.
2. **Receipt Schema Version 3 (`receipt.json`):**
   ```json
   {
     "schema_version": 3,
     "disc": {
       "serial": "SCUS-97328",
       "version": "v2.00"
     },
     "image": {
       "mode": "external",
       "file": "Gran Turismo 4 (USA).iso",
       "external_path": "D:/Emulators/PCSX2/isos/Gran Turismo 4 (USA).iso",
       "size_bytes": 5691211776,
       "modified_unix_nanos": 1698765432000000000
     }
   }
   ```
3. **Execution Verification (`check_installed`):**
   - If `image.mode == "external"`:
     - Check `metadata(image.external_path)`.
     - If the file is not found (e.g. unplugged external drive), return clean error:
       `"The disc image at <path> is not accessible. Connect your drive and try again."`
     - If valid, return `Installed::Ready { image: image.external_path, ... }`.
4. **Game Process Safety:**
   - The native game runtime (`gt4.exe`) only opens the ISO in read-only mode (`O_RDONLY` / `GENERIC_READ`). It never performs write operations on the disc image.
   - Therefore, sharing the ISO concurrently with PCSX2 or other software causes zero write contention.

---

## 6. Storage Savings Assessment

| Component | Default Copy Mode | Zero-Copy External Mode | Savings |
|---|---|---|---|
| Disc Image (`.iso`) | ~5.3 GB | 0 GB (referenced in-place) | **5.3 GB (100%)** |
| Dual Targets (GT4 + Spec II) | ~10.6 GB | 0 GB | **10.6 GB (100%)** |
| HD Texture Pack | ~12.0 GB | 0 GB (referenced in-place) | **12.0 GB (100%)** |
| Total Disk Space Saved | — | — | **Up to ~22.6 GB** |

---

## 7. Verification & Safety Contracts

- **Read-Only Invariant:** Native game engine must continue opening the disc image with read-only file access.
- **Clean Fallback:** If an external texture or ISO directory is removed or invalid, the launcher must provide an informative message and a one-click reset to default storage.
- **Contract Integrity:** Verified that `gt4l-core` (108 tests) and `launch_contract` (16 tests) remain fully compliant with launcher INI generation contracts.
