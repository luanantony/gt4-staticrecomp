# In-Game Controller Button Prompts (TASK-165, Issue #161)

## Overview

Gran Turismo 4 natively displays PlayStation DualShock 2 button glyphs ($\times, \bigcirc, \square, \triangle, \text{L1/R1}, \text{L2/R2}, \text{Start}, \text{Select}$).
TASK-165 introduces an option to render in-game controller button prompts matching **Xbox** or **Nintendo Switch** controllers without any game executable or AOT translation unit modifications.

## Configuration

### Launcher Settings
* Located in **Settings > Controls** (and in `settings.toml` under `[settings]`):
  * `button_prompts = "playstation"` (default)
  * `button_prompts = "xbox"`
  * `button_prompts = "switch"`

### Standalone Direct Start (`game.ini`)
* When starting the game directly without the launcher, `game.ini` specifies:
  ```ini
  button_prompts=xbox
  ```
  or
  ```ini
  button_prompts=switch
  ```
* The standalone runner exports this setting to the game environment as `GT4_BUTTON_PROMPTS=xbox` or `GT4_BUTTON_PROMPTS=switch`.

## Renderer Texture Replacement Pipeline

Button prompt replacement uses the texture cache replacement mechanism supported by both renderers:
* **GSdx (`gt4-gs.exe`):** Reads texture replacements from `[Folders] Textures` (configured in `gt4-gs.ini` with `LoadTextureReplacements = true`).
* **CoreGS 2.0:** Scans the directory provided by `GT4_HW_TEXTURES` recursively using `ScanFormatPFolder`.

### Glyph Mapping Table

| Digital Function | Original PS2 Glyph | Xbox Replacement | Nintendo Switch Replacement |
|---|---|---|---|
| **Confirm / Gas** | Blue Cross $(\times)$ | Green **(A)** | Red **(B)** |
| **Cancel / Brake** | Red Circle $(\bigcirc)$ | Red **(B)** | Yellow **(A)** |
| **Secondary / Handbrake** | Pink Square $(\square)$ | Blue **(X)** | Green **(Y)** |
| **View / Reverse** | Green Triangle $(\triangle)$ | Yellow **(Y)** | Blue **(X)** |
| **Shoulders** | `[L1]` / `[R1]` | `[LB]` / `[RB]` | `[L]` / `[R]` |
| **Triggers** | `[L2]` / `[R2]` | `[LT]` / `[RT]` | `[ZL]` / `[ZR]` |
| **Start / Select** | `[Start]` / `[Select]` | `[Menu]` / `[View]` | `[+]` / `[-]` |

### Asset Storage & Layout
Replacement PNG textures are placed under:
* `data/prompts/xbox/`
* `data/prompts/switch/`

Each replacement image conforms to the Format P naming convention:
`<textureHash>[-<clutHash>][-r<W>x<H>]-<bits>.png`

When Xbox or Switch prompts are enabled, the corresponding textures directory is linked or copied to `Textures/prompts/`, immediately overriding the original button glyphs upon the next game boot. Setting the option back to `playstation` removes `Textures/prompts/`, restoring native PlayStation 2 glyphs.
