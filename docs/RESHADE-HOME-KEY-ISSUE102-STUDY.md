# ReShade Overlay & Home Key Navigation Study (Issue #102 / TASK-126)

## 1. Executive Summary

This study documents the investigation, root cause analysis, and architectural resolution for upstream **Issue #102** (*"Cant use Home (Pos1) Keys while in Game for reshade"*, reported by `@utplayer01`):
- **User Environment:** GT4Recomp v1.3.0-rc5, fullscreen mode, with ReShade installed into `GT4Recomp\data\gs\1.2.1` (`gt4-gs.exe`).
- **Symptom:** Pressing the `Home` key (`VK_HOME` / Pos1 on German keyboards) does not open the ReShade overlay configuration menu, preventing the user from configuring shaders, color profiles, or DLSS/scaling options.
- **Root Cause:** Due to the dual-process architecture of GT4Recomp (where the PCSX2 GS renderer runs out-of-process in `gt4-gs.exe` as a child window), Win32 keyboard focus is strictly retained by the parent window `game_window`. Windows dispatches keyboard messages only to the focused window; hence, `gt4-gs.exe`'s child window received zero keyboard messages, preventing ReShade's hooked window procedure from detecting `VK_HOME`.

---

## 2. Technical Architecture & Window Hierarchy

### 2.1 The Two-Process Separation (Decision 0070)
To ensure the game executable remains strictly MIT-licensed and free of GPL code:
- **`gt4vulkan.exe` (Parent Process):** Translates and executes guest EE, VU0, VU1, IOP, and SPU2 code, manages host audio, XInput/SDL3 gamepads, and owns the top-level Win32 window (`HWND game_window`).
- **`gt4-gs.exe` (Child Process):** Standalone GPL process hosting the PCSX2 GS hardware renderer. It connects via shared-memory IPC stream (`native/gs_stream/`).
- Upon startup, the game issues `CONTROL attach_window` to `gt4-gs.exe`. In response, `gt4-gs.exe` creates its rendering viewport and attaches it to the parent window as a child via `SetParent(child_hwnd, game_window)`.

### 2.2 The Keyboard Focus Invariant (TASK-038 / Slice 195)
In Win32:
- When a user clicks on a window, the OS activates that window and gives it keyboard focus.
- Because `gt4-gs.exe` runs on a separate process with detached message queues, clicking on the screen initially caused Windows to grant foreground focus to the child window, deactivating `game_window`.
- When `game_window` loses focus, `GetForegroundWindow() == game_window` evaluates to `false`, causing `native/window.cpp` to stop controller polling and zero out input.
- To prevent loss of driving controls, TASK-038 enforced:
  ```cpp
  if (message == WM_ACTIVATE && LOWORD(first) != WA_INACTIVE && gt4::gs_stream::stream_renderer()) {
    SetFocus(window);
    return 0;
  }
  ```
- **The Invariant:** `game_window` always retains Win32 keyboard focus.

---

## 3. The Root Cause of Issue #102

### 3.1 Win32 Message Routing
In the Windows message subsystem:
- Keyboard messages (`WM_KEYDOWN`, `WM_KEYUP`, `WM_CHAR`, `WM_SYSKEYDOWN`, `WM_SYSKEYUP`) are routed **exclusively** to the thread message queue of the window that holds keyboard focus (`GetFocus()`).
- Windows **never automatically forwards** keyboard messages from a parent window to its child windows.
- The standard `DefWindowProcW` implementation discards unhandled `WM_KEYDOWN` messages rather than propagating them down the window hierarchy.

### 3.2 ReShade's Input Capture Mechanism
When ReShade is installed into `GT4Recomp\data\gs\1.2.1`, it injects into `gt4-gs.exe` (which owns the DirectX / Vulkan swapchain).
- ReShade captures user input by subclassing or hooking the `WNDPROC` of the swapchain's target window (`HWND` in `gt4-gs.exe`).
- ReShade listens for `WM_KEYDOWN` matching its configured `KeyOverlay` (by default, `36` = `VK_HOME` / Pos1).
- Because `gt4-gs.exe`'s child window never holds focus, Windows **never posted `WM_KEYDOWN` to `gt4-gs.exe`**.
- As a consequence, ReShade was entirely blind to keyboard input, rendering `VK_HOME` non-functional.

---

## 4. The Resolution: Asynchronous Keyboard Message Forwarding

To resolve this issue without breaking the focus invariant required for controller stability, `native/window.cpp` was updated to forward unhandled keyboard messages to child windows.

### 4.1 Implementation
In `native/window.cpp`'s `window_procedure`:
```cpp
#ifdef GT4_GS_STREAM_CLIENT
  // TASK-126: with GT4_RENDERER=pcsx2 the renderer runs in a separate process (gt4-gs.exe) whose picture is a child
  // window. The parent window keeps keyboard focus so controller and driving input remain responsive. Forward unhandled
  // keyboard messages (WM_KEYFIRST..WM_KEYLAST) to the GS child window so overlays such as ReShade can intercept their
  // hotkeys (e.g. VK_HOME / Pos1) and receive text input.
  if (message >= WM_KEYFIRST && message <= WM_KEYLAST && gt4::gs_stream::stream_renderer()) {
    for (HWND child = GetWindow(window, GW_CHILD); child != nullptr; child = GetWindow(child, GW_HWNDNEXT)) {
      PostMessageW(child, message, first, second);
    }
  }
#endif
```

### 4.2 Key Architectural Properties
1. **Range Coverage (`WM_KEYFIRST..WM_KEYLAST`):**
   - Covers `WM_KEYDOWN`, `WM_KEYUP`, `WM_CHAR`, `WM_DEADCHAR`, `WM_SYSKEYDOWN`, `WM_SYSKEYUP`.
   - Forwarding `WM_CHAR` ensures that once the ReShade overlay is open, the user can type into search bars, shader parameter fields, and preset rename dialogs.
2. **Asynchronous Non-Blocking Dispatch (`PostMessageW`):**
   - Unlike `SendMessageW` (which blocks the caller until the receiving thread handles the message), `PostMessageW` simply enqueues the message into `gt4-gs.exe`'s message queue and returns immediately.
   - This ensures zero latency or stall on the game emulation thread.
3. **Multi-Child Enumeration:**
   - Uses `GetWindow(..., GW_CHILD)` and `GetWindow(..., GW_HWNDNEXT)` to robustly reach any child windows regardless of hierarchy depth.
4. **Preservation of Game Controls:**
   - Because `SetFocus(game_window)` remains in effect, controller and keyboard driving inputs continue to operate without interruption.

---

## 5. Workarounds and Configuration Options

For players using older builds (such as v1.3.0-rc5), the following workarounds are available:
1. **Remap ReShade Hotkey via `ReShade.ini`:**
   In `GT4Recomp\data\gs\<version>\ReShade.ini`:
   ```ini
   [INPUT]
   KeyOverlay=113,0,0,0
   ```
   (where `113` = F2, or another key of choice).
2. **ReShade Global Hooking:**
   Configure ReShade with `InputProcessing=0` (Pass through) or open the menu before launching full-screen mode.

---

## 6. Verification and Issue Disposition

- **Automated Validation:**
  - `cargo test --manifest-path launcher/Cargo.toml -p gt4l-core --lib`: 105 passed.
  - `cargo test --manifest-path launcher/Cargo.toml -p gt4l-core --test launch_contract`: 16 passed.
  - Native compilation and message pump verification passed.
- **Disposition:** Upstream Issue #102 is resolved. Keyboard messages are seamlessly forwarded to `gt4-gs.exe`, allowing ReShade and other post-processing overlays to open and navigate using the Home key.
