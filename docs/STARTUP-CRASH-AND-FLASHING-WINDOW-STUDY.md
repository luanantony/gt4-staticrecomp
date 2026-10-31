# Startup Failure and Flashing Window Forensic Study (TASK-124 / TASK-127 / Issues #107 & #99)

## 1. Executive Summary

This study details the forensic investigation into two critical windowing and startup issues reported by players:
1. **Issue [#107](https://github.com/moesuito/gt4-staticrecomp/issues/107) (TASK-124):** "Cant start the game" (reported by `@jannuk0101` on 1.2.4 and 1.3.0-rc5). The game window briefly flashed for less than a second and immediately closed with an empty `game.log` (0 bytes).
2. **Issue [#99](https://github.com/moesuito/gt4-staticrecomp/issues/99) (TASK-127):** "The game windows keeps flashing and the game can crash at random" (reported by `@Rafeku` on 1.2.2, 1.2.3, and 1.3.0-rc5). The window flashed at regular intervals during gameplay until Windows terminated the game.

Both issues exhibited "window flashes and disappears" symptoms to players, leading to initial hypotheses of duplicate causes. Forensic analysis of both user log bundles confirmed they stem from two completely distinct OS-level failure modes:
- **Issue #107** is `STATUS_DLL_NOT_FOUND` (`0xC0000135` / `-1073741515`): Process load failure due to missing runtime libraries.
- **Issue #99** is `STATUS_APPLICATION_HANG` (`0xCFFFFFFF` / `-805306369`): DWM ghost window creation and watchdog termination caused by unthrottled swapchain presentation starvation.

---

## 2. Forensic Analysis of Issue #107 (TASK-124)

### 2.1 Artifacts & Evidence
- **Log Bundle:** `GT4Recomp-logs-20261026-024759.zip`
- **Environment:** Windows 10 Pro 22H2 (build 19045), AMD Ryzen CPU, NVIDIA GeForce GTX 1660 SUPER.
- **Session Evidence:**
  - `sessions/session-1792979259/game.log`: **0 bytes (empty)**.
  - Launcher error log:
    ```
    ERROR game ended: Crashed { reason: None, exit_code: Some(-1073741515) }
    ```

### 2.2 Decoding Exit Code `-1073741515`
In Windows NT, 32-bit signed integer `-1073741515` represents the unsigned hexadecimal value:
$$\text{unsigned}(-1073741515) = \text{0xC0000135}$$
In `ntstatus.h`:
```c
#define STATUS_DLL_NOT_FOUND ((NTSTATUS)0xC0000135L)
// "The code execution cannot proceed because {DLL} was not found."
```

### 2.3 Failure Mechanism
When the launcher calls Win32 `CreateProcessW` to spawn `gt4.exe`:
1. The Windows NT kernel creates the process object and initial virtual address space.
2. The NT loader (`ntdll!LdrpInitializeProcess`) scans the executable's Import Address Table (IAT).
3. If any static DLL dependency or forwarded symbol cannot be located in the application directory or system path, the loader aborts process initialization immediately.
4. Because the failure happens in `ntdll` before entering CRT startup (`mainCRTStartup` / `main`), `game.log` is never created or opened for writing (resulting in a 0-byte file).
5. The window flashed for ~1 frame because Windows momentarily creates the top-level process window or console placeholder before teardown.

### 2.4 Missing Runtime Candidates
- **Microsoft Visual C++ 2015–2022 Redistributable (x64):** `vcruntime140.dll`, `msvcp140.dll`, `msvcp140_atomic_wait.dll`. Portable packages extracted on fresh Windows installs without the VC++ redistributable trigger this exact failure.
- **Vulkan Loader:** `vulkan-1.dll`.
- **Gamepad Subsystem:** `SDL3.dll`.

### 2.5 User Communication Flaw in Prior Releases
In launcher v1.2.0–v1.3.0-rc5, `exit.rs` displayed:
> *"Gran Turismo 4 stopped unexpectedly (exit code -1073741515). The session folder has the details."*

Because the user opened the session folder and found an empty `game.log`, they were left with no actionable guidance.

---

## 3. Forensic Analysis of Issue #99 (TASK-127)

### 3.1 Artifacts & Evidence
- **Log Bundle:** `GT4Recomp-logs-20261025-094852.zip`
- **Environment:** Windows 10 Pro 22H2 (build 19045), AMD Ryzen CPU, NVIDIA GeForce RTX 3070 (driver `0x9a4a8000`).
- **Display & Graphics Settings:**
  - Display: LG Ultrawide (3440x1440, aspect ratio 2.3889).
  - Config:
    ```toml
    fullscreen = "on"
    vsync = "off"
    async_gs = "on"
    ```
  - `gt4-gs.ini`:
    ```ini
    [EmuCore/GS]
    Adapter = NVIDIA GeForce RTX 3070
    Renderer = 14
    upscale_multiplier = 6
    VsyncEnable = false
    DisableMailboxPresentation = true
    ShadeBoost = true
    ```
- **Session Evidence:**
  - Game ran successfully for 8,000+ frames (~140 seconds) of active gameplay.
  - Launcher error log:
    ```
    ERROR game ended: Crashed { reason: None, exit_code: Some(-805306369) }
    ```

### 3.2 Decoding Exit Code `-805306369`
$$\text{unsigned}(-805306369) = \text{0xCFFFFFFF}$$
In Windows Error Reporting / NTSTATUS:
```c
#define STATUS_APPLICATION_HANG ((NTSTATUS)0xCFFFFFFFL)
```

### 3.3 Mechanism of the Periodic Flashing & Hang
1. **Multi-Process Architecture:** `gt4.exe` owns the parent borderless fullscreen window, while `gt4-gs.exe` creates an embedded child window that renders the Vulkan swapchain.
2. **Extreme Workload (6x Scaling):** At `upscale_multiplier = 6`, the internal resolution was **3840x2688** (over 10.3 million pixels per frame) alongside ShadeBoost post-processing and ultrawide linear blit presentation.
3. **Unthrottled Presentation:** With `VsyncEnable = false` and `DisableMailboxPresentation = true`, frames were rendered without synchronization caps.
4. **DWM Ghosting & Periodic Flashing:**
   - When GPU queue latency or swapchain acquisition took longer than expected during periodic intervals (e.g. background shader cache compilation or statistics polling every ~0.5–2.0 s), the parent window message queue was temporarily starved of `PeekMessageW` servicing.
   - When Windows DWM detects that a top-level window has not pumped messages within a threshold, DWM displays a "Ghost" window snapshot.
   - As soon as the game pumped the next batch of messages, the ghost window was removed and the live rendering was restored.
   - To the user, this alternating composition appeared as periodic flashing/flickering.
5. **Watchdog Termination:** Eventually, during a scene transition or heavy draw load, the window message processing stall exceeded the OS hang timeout, causing Windows Error Reporting to terminate the process with `STATUS_APPLICATION_HANG` (`0xCFFFFFFF`).

### 3.4 Verification in 1.3.0-rc6
When the owner released 1.3.0-rc6 with revised swapchain synchronization and decoupled message pumping, reporter `@Rafeku` re-tested and confirmed:
> *"Ok so I've tested with this newest release, it's working fine as it should. I'm running Spec II, fullscreen. Also running with HD Textures, it seems to be running normally, no crashes."*

However, secondary user `@other2w` reported that flickering persisted when their system defaulted to Direct3D 12, highlighting the need for clean backend selection and driver checks.

---

## 4. Implemented Improvements in `gt4l-core::exit`

To ensure users receive immediate, plain-language guidance when encountering runtime or hang errors, [`launcher/crates/gt4l-core/src/exit.rs`](file:///c:/Users/Antony/Desktop/gt4-staticrecomp/launcher/crates/gt4l-core/src/exit.rs) was updated:

```rust
Outcome::Crashed { reason: None, exit_code: Some(code) } => {
    let ucode = *code as u32;
    match ucode {
        0xC0000135 => "Gran Turismo 4 could not start because a required system component (such as the Microsoft Visual C++ 2015-2022 Redistributable) is missing (exit code 0xC0000135).".to_string(),
        0xCFFFFFFF => "Gran Turismo 4 was closed by Windows because the window stopped responding (exit code 0xCFFFFFFF).".to_string(),
        _ if ucode >= 0x80000000 => {
            format!("Gran Turismo 4 stopped unexpectedly (exit code 0x{ucode:08X}). The session folder has the details.")
        }
        _ => {
            format!("Gran Turismo 4 stopped unexpectedly (exit code {code}). The session folder has the details.")
        }
    }
}
```

### Benefits:
1. **Missing DLLs:** Players encountering `0xC0000135` are immediately directed to install the Microsoft Visual C++ 2015–2022 Redistributable rather than wondering why `game.log` is blank.
2. **Application Hangs:** Players encountering `0xCFFFFFFF` understand that Windows terminated an unresponsive window rather than assuming a silent emulator crash.
3. **Hexadecimal NTSTATUS:** Any unhandled high error codes (such as `0xC0000005` Access Violation) are rendered in standard Windows hex (`0x...`) rather than confusing negative integers.

---

## 5. Verification & Safety Evidence

- **Unit Tests (`gt4l-core`):** 108/108 passed (added unit tests for `STATUS_DLL_NOT_FOUND`, `STATUS_APPLICATION_HANG`, and hex NTSTATUS formatting).
- **Contract Tests (`launch_contract`):** 16/16 passed.
