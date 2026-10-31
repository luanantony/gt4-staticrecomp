# Gran Turismo 4 Portable & Standalone Distribution Study

Date: 2026-10-31. Technical investigation and architectural roadmap for Issue #132 (Portable & Standalone).
Related Tasks: [TASK-097 (Self-sufficient game executable)](../TASKS.md#task-097--self-sufficient-game-executable),
[TASK-098 (Portable build zip)](../TASKS.md#task-098--portable-build-zip-beside-the-installer).

## 1. Context & User Request (Issue #132)

In [Issue #132](https://github.com/moesuito/gt4-staticrecomp/issues/132), user Viniazzolini wrote:
> "Percebi que é necessário instalar diversas ferramentas, inclusive o Edge Web view que é muito ruim,
> se possível crie uma versão instandalone! fica muito mais fácil e leve de se usar. obrigado!!"
> *(Translation: "I noticed that it is necessary to install several tools, including Edge Web view which is
> very bad, if possible create a standalone version! it's much easier and lighter to use. thank you!!")*

This report highlights two distinct community desires:
1. **Zero-Installer / Portable Execution:** Running GT4Recomp directly from an unzipped folder without
   running an NSIS installer, requiring administrative privileges, or polluting the Windows registry.
2. **Decoupling from Edge WebView2:** Avoiding the Microsoft Edge WebView2 runtime requirement, which can
   be missing on custom/stripped Windows installations (Windows LTSC, Tiny10/11) and under Wine/Proton.

---

## 2. Technical Architecture & Analysis

### 2.1 The Game Executable (`gt4vulkan.exe`) is ALREADY Standalone and WebView2-Free [C]
Crucially, **the game itself does not use or link to WebView2**:
- `gt4vulkan.exe` is a 100% native C++20 / Vulkan executable.
- Its only runtime dependencies are:
  1. A Vulkan 1.2+ capable GPU driver (`vulkan-1.dll`).
  2. `SDL3.dll` (for gamepad/keyboard input and audio).
  3. Visual C++ 2015-2022 runtime DLLs (`vcruntime140.dll`, `msvcp140.dll`, `msvcp140_atomic_wait.dll`).
     When bundled beside the executable (as done by `scripts/make_distribution.py`), no system CRT installer
     is required.
- **Why can't players currently just double-click `gt4vulkan.exe`?**
  Currently, `gt4vulkan.exe` expects about 14 command-line arguments and environment variables passed by the
  launcher (`--disc`, `--ntsc`, `--memcard`, `--misses`, `--window`, `GT4_RENDERER`, `GT4_GS_CONFIG`, etc.).
  Without these, the executable halts.

### 2.2 The Launcher (`GT4Recomp.exe`) and Edge WebView2 [C]
- The launcher UI is built with **Tauri v2** (`launcher/crates/gt4l-tauri`), which renders its frontend
  using Microsoft Edge WebView2 on Windows.
- On standard Windows 10 and Windows 11, WebView2 is pre-installed as an OS component.
- On stripped Windows editions or Wine/Proton on Linux, WebView2 is absent. When absent, the launcher's
  Win32 message box (`gt4l_platform::webview2`) prompts the user to download the Evergreen bootstrapper.
- **Shipping a Fixed-Version WebView2 runtime** (`WEBVIEW2_BROWSER_EXECUTABLE_FOLDER`) would add ~180 MB
  compressed (~450 MB uncompressed) to the download, directly contradicting the user's desire for a
  lightweight and fast package.

### 2.3 Existing Portable Foundation in GT4Recomp [C]
The codebase already has native support for portable folder paths (TASK-091, `gt4l_core::paths`):
- **Large Data (`<install>\data`):** Disc copy, prepared ELF, GS runtime (`gt4-gs.exe`), caches, logs,
  and textures are resolved relative to the executable folder, not from the Windows registry.
- **User Data (`<install>\user`):** By default, user data (settings, controls, saves) is kept in
  `%APPDATA%\GT4Recomp`. However, `paths.rs` supports `GT4RECOMP_USER_DIR`, allowing user data to be
  fully co-located within `<install>\user`.

---

## 3. Comprehensive Roadmap for Portable & Standalone Gaming

### 3.1 Task Mapping & Prioritization
The user request in Issue #132 maps directly to two existing planned tasks in [TASKS.md](../TASKS.md):

1. **TASK-098: Portable build zip (`GT4Recomp-<version>-portable.zip`)**
   - Package a standalone `.zip` containing:
     ```
     GT4Recomp/
       GT4Recomp.exe       (launcher)
       SDL3.dll            (gamepad & audio)
       vcruntime140.dll    (MSVC runtime)
       msvcp140.dll
       msvcp140_atomic_wait.dll
       game/
         gt4vulkan.exe     (native game engine)
         shaders/          (Vulkan SPIR-V compute shaders)
       user/               (empty folder: signals portable user data)
       data/               (empty folder: signals portable large data)
       licenses/           (MIT and third-party licenses)
     ```
   - When `<install>\user` exists, the launcher automatically runs in portable mode, writing saves,
     settings, and configs into `<install>\user` with zero registry writes and zero `%APPDATA%` usage.
   - Users can unzip this folder to any directory or USB drive and play immediately.

2. **TASK-097: Self-Sufficient Game Executable (`gt4vulkan.exe`)**
   - Provide built-in defaults inside `gt4vulkan.exe`:
     * Renderer: GSdx (`gt4-gs.exe`) with Vulkan backend.
     * Display: 480p mode, 16:9 widescreen hold, auto aspect ratio matching monitor output.
     * Input: SDL3 XInput / DirectInput with default controller mappings.
     * Data Paths: Auto-detect `<exe>\..\data\prepared\SCUS_973.28` and `<exe>\..\data\disc\SCUS-97328.iso`.
   - **Result:** Players who do not want or cannot use the WebView2 launcher can simply double-click
     `gt4vulkan.exe` (or run it via Steam as a non-Steam game) for 100% standalone, launcher-free gaming.

---

## 4. Current Workaround for Players without WebView2

For players who cannot or do not wish to install WebView2 today:
1. **Initial Setup:** Run the disc preparation once (or on another machine), producing `<data>\prepared\`
   and `<data>\disc\SCUS-97328.iso`.
2. **Direct Launch Batch Script:** Create a simple `play.bat` next to `gt4vulkan.exe`:
   ```bat
   @echo off
   set GT4_RENDERER=pcsx2
   set GT4_FORCE_480P=1
   set GT4_FORCE_WIDESCREEN=1
   set GT4_DISPLAY_ASPECT=auto
   set GT4_FULLSCREEN=1
   set GT4_PRESENT_MODE=fifo
   start game\gt4vulkan.exe data\prepared\SCUS_973.28 --ntsc --disc data\disc\SCUS-97328.iso --frames 0 --memcard data\card.ps2 --misses data\misses.txt --window
   ```
   This executes the recompiled game directly with zero WebView2 and zero installation.
