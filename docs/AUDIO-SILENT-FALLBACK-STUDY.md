# Silent Audio Fallback and Headless Output Study (TASK-129 / Issue #93)

## 1. Executive Summary & Problem Context

GitHub Issue [#93](https://github.com/moesuito/gt4-staticrecomp/issues/93) ("Crashing", reported by `@deviljinsuresh`) documents an immediate startup crash when Gran Turismo 4 is launched on systems without an active audio output endpoint:
```text
[window] fatal error: waveOutOpen stereo16/48000: Specified device identifier is out of range (2)
```

The error identifier `(2)` corresponds to WinMM's `MMSYSERR_BADDEVICEID`. This crash occurs whenever:
1. A PC is connected to a monitor over HDMI/DisplayPort that does not report audio support (or has audio passthrough disabled).
2. The user has no speakers or headphones plugged in, or all audio output endpoints in Windows are disabled.
3. Bluetooth headphones or wireless headsets are paired but powered off.
4. The game is executed in virtualized, containerized, or Remote Desktop (RDP) environments lacking a forwarded audio driver.

A duplicate report ([Issue #94](https://github.com/moesuito/gt4-staticrecomp/issues/94)) confirmed the same behavior and was closed as a duplicate.

---

## 2. Windows Multimedia (WinMM) Audio Architecture & Failure Mechanism

### 2.1 The `WAVE_MAPPER` Role
In Windows multimedia programming, passing `WAVE_MAPPER` (`(UINT)-1`) to `waveOutOpen` instructs the Windows Audio Endpoint Builder and WinMM subsystem to automatically bind to the user's default audio playback device.

When at least one playback device is active:
* `waveOutGetNumDevs()` returns $\ge 1$.
* `waveOutOpen` returns `MMSYSERR_NOERROR` (`0`), allocating a valid `HWAVEOUT` handle.

When **zero playback endpoints** are active:
* `waveOutGetNumDevs()` returns `0`.
* Calling `waveOutOpen` with `WAVE_MAPPER` fails with `MMSYSERR_BADDEVICEID` (`2`) or `MMSYSERR_NODRIVER` (`6`).

### 2.2 The Original Exception Trap in `native/host_io.cpp`
In GT4Recomp's native host adapter:
```cpp
static void check_audio(const char *action, MMRESULT result) {
  if (result != MMSYSERR_NOERROR)
    throw std::runtime_error(std::string(action) + ": " + wave_out_error_text(result));
}
```

During initialization of `AudioOutput`:
```cpp
check_audio("waveOutOpen stereo16/48000",
            waveOutOpen(&device_, WAVE_MAPPER, &format, 0, 0, CALLBACK_NULL));
```

Because `check_audio` threw an unhandled `std::runtime_error`, execution was aborted immediately inside `run_window()`. The window failed to display, and the guest Emotion Engine was never started.

---

## 3. Pacing and Timing Dependencies (`GT4_PACING=audio`)

A critical consideration in resolving this issue is the interaction between audio output and frame pacing:

### 3.1 Audio-Slaved Frame Clock
In release builds, GT4Recomp defaults to `GT4_PACING=audio` (TASK-006):
* Frame pacing is governed by audio buffer consumption in `pacing.cpp` and `frame_clock.cpp`.
* The game presents a video frame each time the audio queue consumes 800 stereo samples ($48,000 / 60 = 800$), ensuring perfectly stutter-free sound without buffer underruns or drift.

### 3.2 The Need for Clock Demotion in Silent Mode
If the game runs silently without an audio device:
* There is **no physical hardware audio clock** to consume samples.
* If `pacing_mode` remains set to `Mode::audio`, the frame clock would stall waiting for audio hardware backpressure.
* **Resolution:** When initializing `run_window()`, if no physical audio device is available (`!audio.has_device()`), the pacer must automatically **fall back from `Mode::audio` to `Mode::paced`**:
  ```cpp
  if (pacing_mode == gt4::pacing::Mode::audio && !audio.has_device()) {
    std::fprintf(stderr, "[pacing] no audio device; falling back from audio pacing to paced mode\n");
    pacing_mode = gt4::pacing::Mode::paced;
  }
  ```
* In `Mode::paced`, the high-resolution multimedia timer (`QueryPerformanceCounter`) maintains a strict 16.666 ms (60.0 FPS) cadence independently of audio hardware.

---

## 4. Architectural Solution: The Silent Dummy Sink

The fix implemented in `task/129-silent-audio-fallback` (PR #119) introduces a resilient silent fallback in `AudioOutput`:

### 4.1 Graceful Initialization
```cpp
const bool device_available = allow_device && waveOutGetNumDevs() > 0;
if (device_available) {
  const MMRESULT open_result = waveOutOpen(&device_, WAVE_MAPPER, &format, 0, 0, CALLBACK_NULL);
  if (open_result == MMSYSERR_NOERROR) {
    has_device_ = true;
  } else {
    std::fprintf(stderr, "[audio] waveOutOpen failed (%u): %s; running silently\n",
                 open_result, wave_out_error_text(open_result).c_str());
  }
} else {
  std::fprintf(stderr, "[audio] no audio output device available; running silently\n");
}
```

### 4.2 Environmental Override
Players can also explicitly force silent execution via environment variable:
`GT4_AUDIO=none`, `GT4_AUDIO=silent`, or `GT4_AUDIO=off`.

### 4.3 Dummy Sink Pipeline Contract
In silent mode (`!has_device_`):
1. **`push(const std::int16_t *samples, std::size_t frames)`:**
   - Validates lifetime (`!closed_`).
   - Updates accounting metrics (pushed frames count).
   - Returns `true` immediately without touching WinMM headers.
2. **`pump()`:**
   - Returns immediately; no hardware headers to unprepare or reclaim.
3. **`queued_frames()`:**
   - Returns `0`.
4. **`close()`:**
   - Marks `closed_ = true` and resets state without attempting `waveOutReset` or `waveOutClose`.

---

## 5. Verification & Test Coverage

### 5.1 Unit Tests in `native/host_io_fixture.cpp`
The solution is verified by automated test cases:
* **Silent Initialization:** Verifies that `AudioOutput(..., allow_device=false)` constructs without error and logs silent operation.
* **Sample Push Acceptance:** Verifies that `push()` accepts stereo samples and returns `true`.
* **Clean State Transitions:** Verifies that calling `close()` transitions state properly and causes subsequent `push()` calls to return `false`.
* **Pacing Fallback:** Verifies that `Mode::audio` seamlessly transitions to `Mode::paced` without timing drift or hangs.

### 5.2 Test Results
* `cargo test -p gt4l-core --lib`: 105/105 passed.
* `cargo test -p gt4l-core --test launch_contract`: 16/16 passed.
* Native CTest test suites pass cleanly.
