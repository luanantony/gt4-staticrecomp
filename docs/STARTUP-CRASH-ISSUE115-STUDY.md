# Startup Crash Forensic Analysis & Study (Issue #115 / TASK-143)

## 1. Executive Summary

Upstream Issue **#115** (*"game won't start"*, reported by `@Stingerman14` on `moesuito/gt4-staticrecomp`) detailed an immediate crash on launch where the launcher reported:
```
ERROR game ended: Crashed { reason: None, exit_code: Some(1) }
```
The user provided the official crash bundle: `GT4Recomp-logs-20261026-134521.zip`.

By inspecting the user's crash logs, session histories, and host environment, the exact root cause was isolated:
**The host system has no active audio output device (speakers/headphones/monitor audio) enabled or connected in Windows.**
During startup, the native game process (`gt4-staticrecomp.exe`) initializes its host audio driver via WinMM `waveOutOpen(&device_, WAVE_MAPPER, ...)`. When no playback device exists, `waveOutOpen` fails with `MMSYSERR_BADDEVICEID (2)`:
```
[host] audio/input/window failure: waveOutOpen stereo16/48000: A device ID has been used that is out of range for your system. (2)
```
This uncaught exception caused `native/window.cpp` to abort before the EE could boot. Furthermore, because the crash happened before EE startup, no `[ee] stopped after` telemetry line existed, causing the launcher to classify the outcome as `Crashed { reason: None, exit_code: Some(1) }` with the unhelpful message *"Gran Turismo 4 stopped unexpectedly (exit code 1)"*.

---

## 2. Forensic Analysis of User Logs

### 2.1 Hardware and System Configuration (`system.txt`)
- **OS:** Windows 11 26H2 (Build 26300)
- **CPU:** Intel Core i5-14600KF (14 cores / 20 threads) — *Note: "KF" processors have no integrated graphics or integrated audio.*
- **GPU:** AMD Radeon RX 7800 XT (16 GB VRAM), Driver 32.0.32015.7029 (Vulkan 1.4.359)
- **Display:** ASUS ROG Strix OLED XG27AQDMG (2560x1440 @ 240Hz) — *High-end OLED monitor without integrated speakers.*
- **Launcher Version:** GT4Recomp 1.3.0-rc6 (Installer build at `D:\game\GT4Recomp`)

### 2.2 Setup & Disc Triage (`latest.log`)
1. **Disc Rejections:** The user initially attempted to supply `Gran Turismo 4 Spec II (1.06.1 Full).iso`. The launcher properly rejected this modified beta revision (`disc rejected ... This SCUS-97436 disc has a different game program than Spec II v1.10`).
2. **Disc Acceptance:** The user then supplied an authentic dump: `Gran Turismo 4 (USA) (v2.00).iso`. Disc setup completed cleanly:
   ```
   INFO disc accepted as gt4: SCUS-97328 v2.00 (D:\game\GT4Recomp\Gran Turismo 4 (USA) (v2.00).iso)
   INFO setup complete (gt4): SCUS-97328 v2.00 image SCUS-97328.iso elf 10f82e2231a51404b95682ed3ea81171100a1af2fefdeed3391943016c7c935c
   ```
   The game ELF matches the verified golden release hash `10f82e22...`.

### 2.3 Game Execution & Failure Point (`game.log`)
Three distinct play attempts were logged:
- `session-1793018097` (13:34:57) -> Exit code 1
- `session-1793018109` (13:35:09) -> Exit code 1
- `session-1793018161` (13:36:01) -> Exit code 1
- `session-1793018297` (13:38:17) -> Exit code 1
- `session-1793018713` (13:45:13) -> Exit code 1

In every session:
1. `gt4-gs.exe` (PCSX2 GS Vulkan x4, 2560x1440) launched cleanly:
   ```
   [0.4266] (GSStreamReplayer) READY posted (renderer PCSX2 GS Vulkan x4, gt4gs 1.2.1 (PCSX2 v2.8.2, no JIT))
   [0.4279] gt4-gs: window attached to 0x810648 (2560x1440): ok; input queues detached
   ```
2. PCSX2 GS SPU2 sound initialization detected no audio endpoints, falling back to Null audio:
   ```
   [0.4239] Creating Null audio stream, sample rate = 48000, expansion = Disabled, buffer = 50, latency = 20, stretching enabled, driver = , device = 
   ```
3. Native host engine (`native/window.cpp:571`) attempted to initialize `AudioOutput audio(pump_messages, pacing_mode)`:
   ```cpp
   // native/host_io.cpp:298
   check_audio(waveOutOpen(&device_, WAVE_MAPPER, &format,
                           reinterpret_cast<DWORD_PTR>(event_), 0,
                           CALLBACK_EVENT),
               "waveOutOpen stereo16/48000");
   ```
4. `waveOutOpen` failed with `MMSYSERR_BADDEVICEID (2)` because `waveOutGetNumDevs() == 0`:
   ```
   [perf] session end: partial interval omitted; use F8 before closing
   [host] audio/input/window failure: waveOutOpen stereo16/48000: A device ID has been used that is out of range for your system. (2)
   ```
5. `native/window.cpp` caught the exception at line 613, printed the error to `stderr`, and exited with `return 1;`.

---

## 3. Root Cause & Architectural Diagnosis

### 3.1 Why does `waveOutOpen` return `MMSYSERR_BADDEVICEID (2)`?
In the Windows Multimedia API (MME):
- `WAVE_MAPPER` is defined as `(UINT)-1`, directing Windows to route audio to the default audio output device.
- If no active, enabled playback devices exist in Windows (e.g. headphone unplugged, Bluetooth headset disconnected, monitor audio disabled, or `Audiosrv` service stopped), `waveOutGetNumDevs()` returns `0`.
- Calling `waveOutOpen` with `WAVE_MAPPER` when `waveOutGetNumDevs() == 0` returns error code `2` (`MMSYSERR_BADDEVICEID`), with description: *"A device ID has been used that is out of range for your system."*

### 3.2 Resilience Disparity
- **`gt4-gs.exe` (PCSX2 GS):** SPU2 checks available audio drivers; when none are present, it initializes a null audio stream (`Creating Null audio stream, driver = , device = `) and continues running without interruption.
- **`gt4-staticrecomp.exe` (Native Game Host):** In `native/host_io.cpp`, `AudioOutput` unconditionally treats a `waveOutOpen` error as fatal and throws `std::runtime_error`, crashing the entire game before the first frame can be rendered.
- **Pacing Independence:** In default paced mode (`gt4::pacing::Mode::paced`), game frame pacing is governed by the high-precision wall-clock timer (`frame_clock->wait_until(frame_pacer->deadline())`), not audio backpressure. Therefore, running without audio or with a silent dummy sink is completely safe for game pacing and logic.

---

## 4. Implemented Fixes & Mitigations

### 4.1 Enhanced Exit Classification & Actionable User Guidance (`gt4l-core::exit`)
Previously, `gt4l-core::exit::classify` only searched for `[ee] stopped after ...`. When a startup crash occurred before EE execution, `reason` remained `None`, resulting in:
`Gran Turismo 4 stopped unexpectedly (exit code 1). The session folder has the details.`

We updated `launcher/crates/gt4l-core/src/exit.rs`:
1. **Host Failure Parsing:** Added parsing for `[host] audio/input/window failure: <reason>` when no EE stop line is present.
2. **Actionable Diagnostics:** If the failure indicates `waveOutOpen` or `A device ID has been used that is out of range`, `user_message` now produces:
   ```
   Gran Turismo 4 could not start because no audio output device was found. Connect headphones or speakers, or enable an audio output device in Windows.
   ```
3. **General Host Errors:** For any other host-side startup failures (such as Vulkan presentation refusal or exclusive lock conflicts), the exact host error string is surfaced in the UI dialog rather than an opaque exit code.

### 4.2 Comprehensive Unit Testing
Added unit tests in `launcher/crates/gt4l-core/src/exit.rs`:
- `host_audio_failure_is_classified_with_actionable_user_message`: Validates exact user log line from Issue #115 and asserts correct classification and actionable advice.
- `other_host_failure_is_classified`: Validates general host failures.
All 107 library tests and 16 launch contract tests pass cleanly.

---

## 5. Recommendation for Native Recomp Engine (Phase 2 Native Slices)

In `native/host_io.cpp`:
1. Check `waveOutGetNumDevs()`. If `0`, or if `waveOutOpen` returns `MMSYSERR_BADDEVICEID` / `MMSYSERR_NODRIVER`:
   - Log a diagnostic warning: `[audio] no Windows waveOut playback device found; running with silent dummy audio sink`.
   - Set `device_ = nullptr`.
   - In `push()`, discard sample buffers safely without throwing `Invalid audio sink input/lifetime`.
   - In `close()`, skip device reset and unprepare calls.
This will allow GT4Recomp to run smoothly even on headless test rigs, machines with unplugged headphones, or monitors without built-in audio.
