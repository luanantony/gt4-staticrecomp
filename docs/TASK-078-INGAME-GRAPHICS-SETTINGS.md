# TASK-078: In-Game Graphics Settings and Permanent 480p Hold

## 1. Overview & Goals

TASK-078 delivers host graphics settings synchronization with the game's Options menu while establishing a permanent progressive 480p hold across all game modes:

1. **Permanent 480p Hold (`native/race_display.{hpp,cpp}`)**:
   - `should_force_480p_index(uint32_t index)` now returns `true` for any value other than `race_display_index_480p` (1).
   - Ensures both legacy 480i normal (0) and 1080i (2) index values are forced to 480p progressive at VBlank.
   - Guarantees progressive rendering across all races even when loading existing PS2 saves that originally configured 1080i.

2. **Decoupling from Memory Card (`native/ingame_settings.{hpp,cpp}`)**:
   - The PS2 memory card save format only knows modes 0 (480i), 1 (480p), and 2 (1080i). Storing host upscale resolutions into the card would corrupt save portability to physical PS2 consoles.
   - `handle_guest_option_write` intercepts option choice writes, records the host resolution preset, and immediately ensures the guest RAM option structure at `+0x16B0` stays locked to `1` (480p).
   - Guest saves on the memory card remain 100% clean, standard, and portable.

3. **In-Game Settings Manager**:
   - Manages six internal resolution presets:
     - `480p`: 1x internal scale (640x480)
     - `720p`: 2x internal scale (1280x720)
     - `1080p`: 3x internal scale (1920x1080)
     - `1440p`: 4x internal scale (2560x1440)
     - `4K`: 6x internal scale (3840x2160)
     - `8K`: 12x internal scale (7680x4320)
   - Synchronizes fullscreen mode (`GT4_FULLSCREEN`, Alt+Enter, F11).
   - Synchronizes anti-aliasing (FXAA).

4. **Atomic Feedback Persistence (`game-changes.toml`)**:
   - The game writes changes atomically using a temporary file (`<path>.tmp`) and atomic rename (`MoveFileExA` with `MOVEFILE_REPLACE_EXISTING`).
   - The file location is configured via the `GT4_SETTINGS_FEEDBACK` environment variable passed by the launcher.

5. **Launcher Synchronization (`gt4l-core` / `gt4l-tauri`)**:
   - `gt4l-core::config::merge_game_changes` reads, validates, normalizes, and merges `game-changes.toml` into `settings.toml`.
   - On game exit (and on launcher startup if recovering from an unexpected exit), changed keys are integrated and the feedback file is deleted.

---

## 2. Technical Architecture

### 2.1 Engine-Side Components (`native/`)

- [`native/race_display.hpp`](file:///c:/Users/Antony/Desktop/gt4-staticrecomp/native/race_display.hpp) & [`native/race_display.cpp`](file:///c:/Users/Antony/Desktop/gt4-staticrecomp/native/race_display.cpp):
  - `should_force_480p_index(uint32_t index)`: returns `index != race_display_index_480p`.
  - `force_race_480p_at_vblank`: stores `1` into guest RAM `[0x622F4C] + 0x38CB0 + 0x16B0` whenever current index is not 1.
- [`native/ingame_settings.hpp`](file:///c:/Users/Antony/Desktop/gt4-staticrecomp/native/ingame_settings.hpp) & [`native/ingame_settings.cpp`](file:///c:/Users/Antony/Desktop/gt4-staticrecomp/native/ingame_settings.cpp):
  - `ResolutionPreset`: enum for the 6 presets (`p480` to `p8k`).
  - Conversion helpers: `resolution_preset_multiplier`, `resolution_preset_from_multiplier`, `resolution_preset_name`, `parse_resolution_preset`, `resolution_preset_dimensions`.
  - `InGameSettingsManager`: singleton managing state, serialization, atomic saving, and guest memory decoupling.
- [`native/window.cpp`](file:///c:/Users/Antony/Desktop/gt4-staticrecomp/native/window.cpp):
  - Hooks `set_fullscreen(bool)` to update `InGameSettingsManager::instance().set_fullscreen(...)` and trigger atomic feedback save.
  - Initializes `InGameSettingsManager::instance().init_from_env()` on `open_window`.

### 2.2 Launcher Components (`launcher/crates/`)

- [`launcher/crates/gt4l-core/src/paths.rs`](file:///c:/Users/Antony/Desktop/gt4-staticrecomp/launcher/crates/gt4l-core/src/paths.rs):
  - Added `UserDirs::game_changes_file(&self) -> PathBuf` returning `<config_dir>/game-changes.toml`.
- [`launcher/crates/gt4l-core/src/config.rs`](file:///c:/Users/Antony/Desktop/gt4-staticrecomp/launcher/crates/gt4l-core/src/config.rs):
  - `preset_to_resolution(preset: &str) -> Option<&'static str>`: maps preset identifiers (`480p`..`8k`, `1x`..`12x`) to `WxH` resolution strings.
  - `merge_game_changes(config: &mut ConfigFile, changes_path: &Path) -> Result<bool, ConfigError>`: parses and merges `[settings]` and `[gs]` tables, updates configuration, and cleans up the feedback file.
- [`launcher/crates/gt4l-tauri/src/commands.rs`](file:///c:/Users/Antony/Desktop/gt4-staticrecomp/launcher/crates/gt4l-tauri/src/commands.rs):
  - Injects `GT4_SETTINGS_FEEDBACK` into the game launch environment.
  - On game exit in the background monitoring thread, detects and merges `game-changes.toml`.
- [`launcher/crates/gt4l-tauri/src/state.rs`](file:///c:/Users/Antony/Desktop/gt4-staticrecomp/launcher/crates/gt4l-tauri/src/state.rs):
  - In `AppState::load_with`, merges any pending `game-changes.toml` at launcher startup.

---

## 3. Verification & Evidence

1. **C++ Fixtures**:
   - `native/race_display_fixture.cpp`: verified with `clang-cl /std:c++20 /W4 /WX /EHsc`. Both normal (0) and 1080i (2) indices are confirmed to be forced to 480p (1).
   - `native/ingame_settings_fixture.cpp`: 6/6 test groups pass:
     - `test_presets_and_multipliers`: 100% verified.
     - `test_preset_names_and_parsing`: 100% verified.
     - `test_dimensions`: 100% verified.
     - `test_toml_serialization_and_parsing`: 100% verified.
     - `test_feedback_file_roundtrip`: atomic disk write, flush, and reload verified.
     - `test_guest_decoupling_and_480p_hold`: guest RAM option index verified to hold 480p (1) while host records requested preset and emits `game-changes.toml`.

2. **Launcher Test Suites**:
   - `cargo test --manifest-path launcher/Cargo.toml -p gt4l-core`: 105 core unit tests + 10 `settings_config` integration tests passing (including `in_game_settings_feedback_merges_and_removes_file`).
   - `cargo test --manifest-path launcher/Cargo.toml -p gt4controls`: 17 unit tests + 4 docs match tests passing.
   - `tests/python/test_controls_probe.py`: 6/6 passing.
