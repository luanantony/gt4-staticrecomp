# TASK-095 — Ultrawide: Undistorted Pre-Race Menu and Overlay Canvas

**Status:** Completed  
**Author:** AI Pair Programmer (Antigravity)  
**Date:** 2026-11-01  
**Branch:** `task/095-pre-race-ultrawide`  
**Decision & Guard Rails Reference:** [0071 — Native aspect ratios through runtime hooks](../decisions/0071-native-aspect-ratios-runtime-hooks.md), [ULTRAWIDE-NATIVE-STUDY](../ULTRAWIDE-NATIVE-STUDY.md)

---

## 1. Problem Statement & Context

In Gran Turismo 4 running on ultrawide displays ($R > 16:9$, such as 21:9 = 2.333 or 32:9 = 3.555), the presentation stretches the 640-pixel frame buffer horizontally by:
$$s = \frac{R}{16/9}$$

During normal race gameplay, TASK-080 (P1 and P2) solved aspect distortion cleanly:
1. **3D Scene:** The camera projection matrix built in `RaceBase::initMonitor` (0x00388A80) is intercepted by `on_race_camera` (at the return address 0x00388B38), scaling its horizontal column by $(16/9) / R$. This produces correct Hor+ 3D rendering.
2. **In-Race HUD:** In-race overlays pass through `RaceDisplay::render` (0x0039F498), where `HudCanvasHook` rewrites the orthographic projection matrix from the default 16:9 canvas ($[-106.667, 746.667]$) to $[320 - 240R, 320 + 240R]$.

However, in the **pre-race lobby** (Time Trial, Arcade, and GT Mode event lobby: car name, power/weight/tire table, Session Record, icon row, and car spec lines):
- The 3D track and car models in the background render with the correct Hor+ projection (since `initMonitor` already ran).
- The presentation layer stretches the 640-wide frame buffer to fill the monitor.
- The 2D menu overlay is drawn on top of the live 3D scene using a standard 4:3 orthographic projection ($[0, 640] \times [0, 480]$ or $448$).
- Because the pre-race lobby is drawn by the QuickMenu subsystem rather than `RaceDisplay::render`, `HudCanvasHook` was not applied to it.
- As observed in owner testing (at 3440x1440 on Trial Mountain), the 2D menu overlay elements were stretched horizontally by $s$, causing distorted text, oblong icons, and wide buttons.

Similar behavior occurs in other race-related menus drawn over 3D scenes:
- **Race Pause (`RacePause`, `SimplePause`):** The frozen 3D race remains in the background while the pause menu overlay (Continue, Restart, Options, Exit) is displayed.
- **Race Results (`RaceDisplay::put_aspec_race_result`):** Track/podium background with post-race result tables.
- **Replay Controls (`RaceDisplay::init_starting_style_for_replay`):** Live 3D replay camera with overlay controls.

---

## 2. Mathematical Formulation & Canvas Derivation

### 2.1 Standard 4:3 / 16:9 Menu Orthographic Matrix
The standard GT4 2D orthographic projection matrix transforms coordinates $[l, r] \times [b, t]$ to Normalized Device Coordinates (NDC) $[-1, 1] \times [-1, 1]$.
In GT4 memory, a $4 \times 4$ column/row matrix has 16 32-bit floats:
- $m_0 = \frac{2}{r - l}$
- $m_5 = \frac{2}{b - t}$ (negative, since $b < t$)
- $m_{10} = -1.0f$
- $m_{12} = \frac{l + r}{l - r}$
- $m_{13} = \frac{t + b}{t - b}$
- $m_{15} = 1.0f$
- All other 10 words are $0.0f$.

For standard menu canvases:
- $l = 0.0$
- $r = 640.0$
- $t = 480.0$ (or $448.0$ for 448-line NTSC output)
- $b = 0.0$

### 2.2 Inverse Horizontal Scaling for Ultrawide ($R \ge 16:9$)
To keep 2D elements undistorted when the presenter stretches the frame buffer horizontally by $s = \frac{R}{16/9}$, the canvas horizontal span must be widened by $s$:
$$W_{\text{canvas}} = 640.0 \times \frac{R}{16/9} = 360.0 \times R$$

To keep all 2D elements centered in the frame:
$$X_{\text{center}} = 320.0$$
$$\text{left} = 320.0 - \frac{W_{\text{canvas}}}{2} = 320.0 - 180.0 \times R$$
$$\text{right} = 320.0 + \frac{W_{\text{canvas}}}{2} = 320.0 + 180.0 \times R$$
$$\text{top} = H_{\text{base}} \quad (480.0 \text{ or } 448.0)$$
$$\text{bottom} = 0.0$$

The rewritten matrix entries are:
$$m_0 = \frac{2}{\text{right} - \text{left}} = \frac{2}{360.0 \times R} = \frac{1}{180.0 \times R}$$
$$m_{12} = \frac{\text{left} + \text{right}}{\text{left} - \text{right}} = \frac{640.0}{-360.0 \times R} = -\frac{16.0}{9.0 \times R}$$

For $R \ge 16:9$, the vertical terms $m_5$ and $m_{13}$ remain completely unchanged.

### 2.3 Verification of Ratios
1. **$R = 16:9$ (Game Default):**
   - $\text{left} = 320.0 - 180.0 \times (16/9) = 320.0 - 320.0 = 0.0$.
   - $\text{right} = 320.0 + 180.0 \times (16/9) = 320.0 + 320.0 = 640.0$.
   - $m_0 = 2.0 / 640.0 = 0.003125f$ (exact game value).
   - $m_{12} = -1.0f$ (exact game value).
   - **Zero writes performed to RAM. Byte-identical to original.**
2. **$R = 21:9$ (2.33333...):**
   - $\text{left} = 320.0 - 420.0 = -100.0$.
   - $\text{right} = 320.0 + 420.0 = 740.0$.
   - $W_{\text{canvas}} = 840.0$.
   - The original $0..640$ UI coordinate span occupies $\frac{640}{840} \times 2 = \frac{16}{21} \times 2$ in NDC.
   - When the GS presentation stretches the frame by $\frac{21}{16}$, the screen width is $\frac{16}{21} \times \frac{21}{16} \times 2 = 2.0$ (original full width).
   - The UI elements retain their exact proportions and are centered at $X = 320$.
3. **$R = 32:9$ (3.55555...):**
   - $\text{left} = 320.0 - 640.0 = -320.0$.
   - $\text{right} = 320.0 + 640.0 = 960.0$.
   - $W_{\text{canvas}} = 1280.0$ (exact $2\times$ width).
   - Occupies $1/2$ NDC width, perfectly compensating for $2\times$ presentation stretch.
4. **$R < 16:9$ (Vert+, e.g. 16:10 = 1.6):**
   - $\text{left} = 0.0, \text{right} = 640.0$.
   - Height is expanded vertically: $H = H_{\text{base}} \times \frac{16/9}{R}$.
   - $\text{top} = 240.0 + H/2, \text{bottom} = 240.0 - H/2$.

---

## 3. Implementation Details

### 3.1 `native/display_aspect.hpp`
- Added `HudCanvas menu_canvas(double ratio, double base_height = 480.0)` pure function.
- Added `bool is_game_menu_canvas(const std::uint32_t words[16])` canvas recognizer.
- Added `MenuCanvasHook` class with 8-slot tracking:
  - Validates guest memory alignment and RAM bounds.
  - Recognizes standard 4:3 menu canvas layouts ($0..640 \times 0..480$ and $0..448$).
  - Ignores foreign matrices (split-screen half canvases, 16:9 HUD canvases, arbitrary stack/heap buffers).
  - Preserves exact original words in `slot->original` and restores them when returning to 16:9.
  - Performs 0 writes when ratio is 16:9 or unchanged.
- Added `bool on_menu_overlay_matrix(std::uint8_t *ram, std::uint32_t ram_size, std::uint32_t matrix_address)` runtime hook.

### 3.2 `native/display_aspect.cpp`
- Implemented `menu_canvas` and `is_game_menu_canvas`.
- Implemented `MenuCanvasHook::apply`.
- Integrated `MenuCanvasHook menu` into `Runtime` state.
- Implemented `on_menu_overlay_matrix` with diagnostic logging.

### 3.3 `native/display_aspect_fixture.cpp`
- Added comprehensive unit test `test_menu_canvas()`:
  - Verified edge values and dimensions for 16:9, 21:9, 32:9, 16:10, and 448-line variations.
  - Verified recognition and discrimination in `is_game_menu_canvas`.
  - Verified `MenuCanvasHook` behavior: 16:9 zero-write preservation, foreign matrix tracking, 21:9 rewrite, NDC centering at 320, NDC span $\times s == 2.0$, 32:9 half-width, 16:9 restoration, and memory bounds safety.

---

## 4. Guard Rails Compliance (Decision 0071)

| Guard Rail | Requirement | Verification / Compliance |
|---|---|---|
| **1. No Save-Data Writes** | No hook writes the option object, monitor record, or save-data structures. | `MenuCanvasHook` writes only the 4 floating-point projection matrix entries ($m_0, m_5, m_{12}, m_{13}$) at the target memory address. No game data structures or options are touched. |
| **2. 16:9 Byte-Identical** | At $R = 16:9$, no hook writes anything; frame presentation does not change. | At 16:9, `target == current` matches `slot->original` exactly, writing 0 bytes to RAM. Headless frame output is bit-for-bit identical. |
| **3. Listed Hooks & Automated Tests** | Every hook and word is documented with CTest and Python fixtures. | Full fixture coverage in `native_display_aspect` (`test_menu_canvas`), passing all assertions with 0 warnings under `/W4 /WX`. |
