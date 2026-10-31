# Gran Turismo 4 Split-Screen (2-Player) Resolution, Aspect Ratio, and Controls Study

Date: 2026-10-31. Technical investigation and architectural roadmap for TASK-130 and Issue #92.
[C] = Confirmed, [H] = Hypothesis, [U] = Unknown.

## 1. Context & Community Report (Issue #92)

In [Issue #92](https://github.com/moesuito/gt4-staticrecomp/issues/92), user HunterAshen1 reported:
"Thank you for bringing the game to pc. But for split screen the resolution is still not great for some reason
and also cant changhe option from 16;0 to 4;3 as me and brother like playing in horizontal... The resolution is
still native i believe"

The owner replied:
- "4:3: today the game is set to 16:9 when it starts. 4:3 support is coming in upcoming updates..."
- "Split screen: it currently keeps a 16:9 picture with side bars, and the upscaling doesn't look as good there
  as in single-player races. Better split-screen support (resolution and two controllers) is planned as a future
  feature, not scheduled yet."

This study documents the mechanics of split-screen rendering in Gran Turismo 4, identifies why players were
locked out of horizontal split-screen, analyzes the visual quality constraints, and lays out the roadmap
for resolution improvements and 2-player controller mapping.

---

## 2. Gran Turismo 4 Split-Screen Architecture [C]

Gran Turismo 4 provides two distinct split-screen orientations in 2-Player Battle mode, managed by
`RaceSplitDisplayBase::initialize` (0x003A29A0):

1. **Horizontal Split (4:3 / Normal Display):**
   - Orientation: Player 1 on top, Player 2 on bottom.
   - Orthographic / Viewport Setup: Two viewports of **640 x 224** pixels.
   - Projection: `setProjectionMatrix(o, 0, 640, f+240, f)` with `f = 0` or `-10`.
   - Drawing: Each player camera renders a 640-pixel wide by 224-line high half-frame.

2. **Vertical Split (16:9 / Wide Display):**
   - Orientation: Player 1 on left, Player 2 on right.
   - Orthographic / Viewport Setup: Two viewports of **320 x 448** pixels.
   - Projection: `setProjectionMatrix(o, -106.666664, 320, 480, 0)`.
   - Drawing: Each player camera renders a 320-pixel wide by 448-line high half-frame.

---

## 3. Root Cause Analysis

### 3.1 The 4:3 Lockout (Inability to select Horizontal Split) [C]
- **Mechanism:** In TASK-079, `GT4_FORCE_WIDESCREEN=1` was added to hold the game's internal `MOption`
  widescreen flag (`owner + 0x38CC4`) and published display word (`0x006184F0`) at 16:9 on every VBlank.
- **Problem:** `launcher/crates/gt4l-core/src/launch.rs` unconditionally set `GT4_FORCE_WIDESCREEN=1` and
  `GT4_DISPLAY_ASPECT=auto`. If a player went into GT4's in-game Options to change Screen from 16:9 (Wide)
  to 4:3 (Normal), the VBlank hook immediately reverted the setting back to 16:9 on the very next frame.
- **Impact:** Players were strictly locked into vertical split-screen (320x448) and could not use horizontal
  split-screen (640x224).

### 3.2 Perceived Resolution Degradation in Split-Screen [C]
- **GS eDRAM Limitations on Original Hardware:**
  To maintain 60 FPS while rendering two independent scene viewports (two camera passes, two shadow passes,
  two car reflections), Polyphony Digital halved the rasterized resolution in one dimension (320 px wide in 16:9,
  or 224 lines high in 4:3).
- **GSdx Upscaling Behavior:**
  In PCSX2 GSdx (`gt4-gs.exe`), the render scale multiplier `k` multiplies the raster target by `k`.
  At 3x scaling (1080p target):
  - Single-Player: 640x448 × 3 = 1920x1344 raster buffer.
  - Split-Screen (16:9): 320x448 × 3 = 960x1344 per player.
  - Split-Screen (4:3): 640x224 × 3 = 1920x672 per player.
  Because the base polygon and texture coordinates are computed for a 320-wide or 224-high frame, mipmap LOD
  selection and texture sampling filters produce noticeably blurrier images than full-screen 1P.

### 3.3 Pillarbox Presentation & Presentation Sink [C]
- In `native/display_aspect.cpp`, lines 489-495:
  ```cpp
  if (r.hud.last_was_split_screen()) {
    // Split screen: its own half-screen canvases, left alone, and not the race view (pillarboxed 16:9).
    return;
  }
  on_race_display_render();
  ```
- Because split-screen canvases return early, `on_race_display_render()` is bypassed.
- `ScenePolicy::race_view_wanted()` remains `false`, so the presentation sink keeps the display mode in
  `DisplayAspectMode::game_aspect` (pillarboxed 16:9 with black side bars), preventing the race view from
  filling the screen.

---

## 4. Improvements Implemented (TASK-130)

1. **Environment Passthrough for Aspect and Widescreen:**
   Updated `launcher/crates/gt4l-core/src/launch.rs` so that `GT4_FORCE_WIDESCREEN` and `GT4_DISPLAY_ASPECT`
   check for existing environment overrides:
   - Setting `GT4_FORCE_WIDESCREEN=0` allows the game to boot and stay in 4:3 mode, enabling horizontal
     split-screen (top and bottom) for players who prefer that layout.
   - Setting `GT4_DISPLAY_ASPECT=4:3` pairs correctly with 4:3 presentation.
2. **Automated Verification:**
   Added integration test `force_widescreen_and_display_aspect_honor_environment_overrides` in
   `launcher/crates/gt4l-core/tests/launch_contract.rs`. All 17 launch contract tests pass.

---

## 5. Future Roadmap for Full Split-Screen Parity

1. **Dual Controller Input (Slot 1 & Slot 2):**
   Extend the SDL3 / XInput backend to enumerate multiple controllers simultaneously and route controller 2
   to the PS2 `pad2` interface (part of the controls roadmap TASK-018 / TASK-101 / TASK-121).
2. **CoreGS 2.0 / 3.0 Viewport Scaling:**
   With our own hardware renderer (CoreGS 2.0 / 3.0), decouple internal camera rendering resolution from the
   PS2 eDRAM buffer constraints, allowing each split viewport to render at true native resolution (e.g. 1920x540
   or 960x1080 per viewport).
3. **Aspect Ratio Canvas Unlocking:**
   Expand `native/display_aspect.cpp` to recognize split-screen HUD canvases and stretch 3D viewports across
   ultrawide displays (21:9 / 32:9) while keeping player HUD elements correctly anchored.
