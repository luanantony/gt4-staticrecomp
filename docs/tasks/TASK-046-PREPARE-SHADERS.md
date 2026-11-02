# TASK-046: Launcher "Prepare Shaders" Headless Pipeline Pre-Warming

## 1. Overview and Problem Statement

When the PCSX2 GS (`gt4-gs.exe`) runs Gran Turismo 4, draw calls encounter uncompiled shader combinations and pipeline states. With a cold pipeline cache, first readbacks (such as headlight flare queries at night and vehicle reflections) take between 8 ms and 150 ms (max measured 144 ms in `docs/GS-LIVE.md`), causing frame pacing stalls and stutter. With a warm pipeline cache (`vulkan_pipelines.bin`), readback waits drop to ~0.60–0.69 ms with 0 frames over 16.7 ms.

TASK-046 provides a launcher action "Prepare shaders" that runs `gt4-gs.exe` headless with `--warm-selectors` and the active configuration, compiling GT4's PCSX2 GS pipeline selector list ahead of time into `%LOCALAPPDATA%\GT4Recomp\gs\cache` (or `<install>\data\gs\cache`).

## 2. Architecture and Design

1. **Pure Plan Contract (`gt4l-core::launch`)**:
   - `WarmShadersPlan`:
     - `program: PathBuf` (`gt4-gs.exe`)
     - `arguments: Vec<String>` (`["--config", "<session>/gt4-gs.ini", "--warm-selectors"]`)
     - `working_directory: PathBuf`
     - `config_path: PathBuf`
     - `cache_dir: PathBuf` (`<renderer_root>/cache`)
     - `log_path: PathBuf` (`<session>/warm-shaders.log`)
   - `plan_warm_shaders(...) -> Result<WarmShadersPlan, String>`
   - `parse_pipelines_compiled(log_text: &str) -> Option<u32>`
   - `WarmReport`: serializable response `{ success, message, pipelines_compiled, cache_dir }`.

2. **Platform Execution (`gt4l-platform::game`)**:
   - `run_warm_shaders(plan: &WarmShadersPlan) -> Result<WarmReport, String>`:
     - Creates directories and session log file.
     - Spawns `gt4-gs.exe` headless with `CREATE_NO_WINDOW`.
     - Logs stdout and stderr to `warm-shaders.log`.
     - Waits for completion and parses output report.

3. **Launcher Commands & IPC (`gt4l-tauri`)**:
   - Tauri command `prepare_shaders`:
     - Verifies game is not running and data migration is not active.
     - Checks that GSdx is installed and verified.
     - Generates atomic `gt4-gs.ini`.
     - Executes `run_warm_shaders` in `spawn_blocking`.
     - Returns JSON report to the UI.

4. **Launcher UI (`settings.js` & `mock-data.js`)**:
   - Button "Prepare shaders" in Settings > Graphics footer next to "Open textures folder".
   - Shows "Preparing shaders..." and disables button during execution.
   - Toasts outcome upon completion or failure.

5. **Security and Contract Validation**:
   - Exact alignment checked across:
     - `launcher/crates/gt4l-tauri/commands.list`
     - `launcher/crates/gt4l-tauri/capabilities/default.json` (`allow-prepare-shaders`)
     - `launcher/crates/gt4l-tauri/src/main.rs` (`generate_handler!`)
     - `launcher/crates/gt4l-tauri/ui/js/pages/settings.js` (`call("prepare_shaders")`)
     - `launcher/crates/gt4l-tauri/ui/dev/mock-data.js`

## 3. Automated Verification

- `cargo test --manifest-path launcher/Cargo.toml -p gt4l-core`: 105 library unit tests and 7 integration test suites passing.
- `cargo test --manifest-path launcher/Cargo.toml -p gt4l-core --test launch_contract`: 18/18 tests passing, including:
  - `plan_warm_shaders_sets_config_and_warm_selectors_and_points_to_cache`
  - `parse_pipelines_compiled_handles_various_log_formats`
- `python verify_contract.py`: all 40 commands match 1:1 across capability, commands list, handler registry, and UI callers with 0 discrepancies.
- `python -m unittest discover -s tests/python -p test_gs_settings.py`: 72/72 tests passing.
