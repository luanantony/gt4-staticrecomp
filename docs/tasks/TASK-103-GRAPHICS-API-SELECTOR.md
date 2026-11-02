# TASK-103: Graphics API Choice for GSdx (Vulkan, Direct3D 12, Direct3D 11)

## Summary

TASK-103 exposes the Graphics API backend setting for GSdx in the launcher Settings page. Players on modern systems can use Vulkan (default/recommended) or Direct3D 12 (recommended), while players on older systems or legacy GPUs can select Direct3D 11.

Software rendering (`SW = 13`) and OpenGL (`GL = 12`) are strictly excluded from the options.

## Architectural Design

1. **ID Collision Avoidance**:
   - The launcher engine setting row is `id = "renderer"` (group `"graphics"`, page `"basic"`), representing the choice between GSdx (`"pcsx2"`) and CoreGS 2.0 (`"coregs20"`).
   - The GSdx graphics backend choice is exposed in the UI as `id = "graphics_api"` to eliminate collision.
   - `DERIVED_TABLE_IDS` in `launcher/crates/gt4l-tauri/src/model.rs` retains `"renderer"`, preventing duplicate row rendering from `pcsx2_table_rows()`.

2. **INI and Backend Key Mapping**:
   - In `launcher/crates/gt4l-core/src/gs_table.rs`, `canonical_id("graphics_api")` aliases to `"renderer"`.
   - `gs_table::row` and `gs_table::check_user_value` accept both `"graphics_api"` and `"renderer"`.
   - In `gs_table::render_ini`, user overrides are keyed by canonical row ID (`"renderer"`), ensuring `[EmuCore/GS] Renderer` is output with the exact backend code:
     - Vulkan: `Renderer = 14` (default)
     - Direct3D 12: `Renderer = 15`
     - Direct3D 11: `Renderer = 3`

3. **Persistence and Backwards Compatibility**:
   - In `settings.toml`, user overrides under `[gs]` persist as `graphics_api = "vulkan" | "d3d12" | "d3d11"`.
   - `Settings::set_gs` normalizes both `"graphics_api"` and legacy `"renderer"` to `"graphics_api"`.
   - Legacy configurations with `[gs] renderer = "..."` load cleanly and migrate to `graphics_api` on next save.

4. **Localized Choice Labels**:
   - `en-US`:
     - `"vulkan"`: `"Vulkan (Recommended)"`
     - `"d3d12"`: `"Direct3D 12 (Recommended)"`
     - `"d3d11"`: `"Direct3D 11 (Older PCs)"`
   - `pt-BR`:
     - `"vulkan"`: `"Vulkan (Recomendado)"`
     - `"d3d12"`: `"Direct3D 12 (Recomendado)"`
     - `"d3d11"`: `"Direct3D 11 (PCs antigos)"`

## Automated Test Verification

- `cargo test -p gt4l-core`: 105 library unit tests and 12 `settings_config` integration tests passed.
- `cargo test -p gt4controls`: 17 unit tests and 4 doc match tests passed.
- `python -m unittest discover -s tests/python -p test_gs_settings.py`: 73 test cases passed.
