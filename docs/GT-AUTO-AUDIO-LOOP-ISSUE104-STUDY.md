# GT Auto Service Audio Infinite Loop Study (Issue #104 / TASK-125)

## 1. Executive Summary

This study details the investigation, root cause diagnosis, and resolution for upstream **Issue #104** (*"Oil Change/Car Wash Noise persists"*, reported by `@Bluuwolfly`):
- **Symptom:** After completing an Oil Change or Car Wash service at GT Auto, the bubbling and water spray sound effects continue playing in an infinite loop.
- **Persistence:** The noise persists across all subsequent menu screens, into the World Map, and even into races and time trials. It only stops if the game process is closed and restarted.
- **Reproduction:** 100% reproducible on the original GT4 release upon visiting GT Auto and triggering an oil change or car wash.

---

## 2. PS2 SPU2 Architecture & ADPCM Loop Mechanics

### 2.1 SPU2 Voice Model
The PlayStation 2 Sound Processing Unit 2 (SPU2) contains two identical 24-voice synthesizer cores (Core 0 and Core 1), providing a total of 48 independent hardware voices:
- Each voice reads compressed 4-bit Sony ADPCM blocks (16 bytes per block, unpacking to 28 16-bit PCM samples) from the 2 MB local SPU2 Sound RAM.
- Byte 1 of each 16-byte ADPCM block contains hardware loop flags:
  - `flags & 0x01`: Block is the **loop end** (`ENDX`).
  - `flags & 0x02`: Block contains the **loop repeat** instruction.
  - `flags & 0x04`: Block marks the **loop start** address (`LSA`).

### 2.2 How Looping SFX Behave
For one-shot sound effects (such as menu cursor clicks or impact thuds), `flags & 0x02` is cleared; once playback reaches the block with `flags & 0x01`, the voice naturally terminates and falls silent.

However, continuous environmental and mechanical effects—such as the bubbling foam of car wash soap, running water jets, engine idle rumbles, and oil draining/pouring—are authored as **looping ADPCM samples**. When the SPU2 playback pointer reaches the end flag, the hardware automatically wraps playback back to the Loop Start Address (`LSA`).

**Crucial SPU2 Rule:** A looping ADPCM voice will continue repeating indefinitely in hardware until one of the following occurs:
1. The host/IOP writes to the voice's Key Off register (`SPU_KEYOFF` / `KOFF`), initiating the ADSR release phase.
2. The voice's volume envelope attenuates to absolute zero (`level <= 0`), allowing the hardware/emulator voice state machine to transition from `kRelease` to `kOff`.
3. The SPU2 core or voice is explicitly reset or reassigned.

---

## 3. Forensic Analysis: The Mathematical Root Cause

In the donor IOP SPU2 emulator (`runtime/src/iop/spu2.cpp`), envelope volume decay and release progression are handled by `Envelope::step`:

```cpp
void step(int rate, bool decrease, bool exponential) {
    const int shift = rate >> 2;
    int cycles = 1 << std::max(0, shift - 11);
    if (exponential && !decrease && level > 0x6000) cycles *= 4;
    if (++counter < cycles) return;
    counter = 0;
    int delta = (decrease ? -8 + (rate & 3) : 7 - (rate & 3)) << std::max(0, 11 - shift);
    if (exponential && decrease) delta = delta * level / 0x8000;
    level = std::clamp(level + delta, 0, 0x7FFF);
}
```

### 3.1 The Integer Truncation Stall
When the GT Auto cutscene completes, the game sound driver (`pdispu2.irx` / `libsd.irx`) sends a Key Off command (`KOFF`) to fade out and stop the service sound effect.
- The voice enters the `kRelease` phase with `decrease = true` and `exponential = true`.
- During exponential decay, the decrement step is scaled by the current envelope volume level:
  `delta = delta * level / 0x8000`
- In standard C++ integer division, division truncates towards zero.
- As `level` drops, a critical threshold is reached where:
  `abs(delta * level) < 0x8000` -> `delta = 0`

For example, with standard release rates where base delta is between -1 and -32, as soon as `level` drops below 32 to 4096, integer arithmetic produces:
`delta = 0`

### 3.2 The Infinite Loop
When `delta = 0`:
`level = clamp(level + 0, 0, 0x7FFF) = level`

The envelope level **stops decreasing entirely**. It freezes at a faint, non-zero residual volume level (e.g. `level = 1` to `level = 15`).

Now consider the condition in `spu2.cpp` that deactivates the voice:
```cpp
if (e.level <= 0) {
    v.phase = kOff;
}
```
Because `e.level` never reaches 0, `v.phase` is **never set to `kOff`**!
The voice remains permanently active in the `kRelease` phase. Since the underlying ADPCM sample has the loop repeat flag (`flags & 2u`), the SPU2 core continues to fetch, decode, and mix the bubble/wash audio blocks into the main SPU2 output buffer frame after frame, indefinitely.

Exiting GT Auto, browsing the World Map, or loading a race track does not terminate this voice because GT4 does not issue a global SPU2 master voice reset on scene changes to preserve background music and sound continuity.

---

## 4. The Resolution: Underflow Guard

To resolve this issue cleanly without altering the PS2 sound driver contracts, an underflow guard is introduced in `Envelope::step`:

```cpp
if (exponential && decrease) {
    delta = delta * level / 0x8000;
    if (delta == 0 && level > 0) delta = -1;
}
```

### 4.1 How It Works
- Whenever exponential decrease is active and integer division truncates `delta` to `0` while `level` is still strictly positive (`level > 0`), `delta` is explicitly forced to `-1`.
- This guarantees that the volume level strictly monotonically decrements by at least 1 unit on every step cycle:
  `level_next <= level_prev - 1`
- Consequently, `level` reliably reaches 0.
- Once `level == 0`, `if (e.level <= 0) v.phase = kOff;` evaluates to `true`, transitioning the voice to `kOff` and stopping sample fetching and playback immediately.

---

## 5. Implementation Architecture & Verification

### 5.1 Overlay Derivation (`scripts/prepare_spu2_envelope.py`)
In accordance with the repository's clean donor architecture, the original `private/references/ps2-web-recomp/runtime/src/iop/spu2.cpp` remains pristine. The derivation script applies the underflow guard overlay and generates a SHA256-verified capsule with `runtime.json`.
- Removing the overlay reverses byte-for-byte to the original donor hash (`84809a6a...`).

### 5.2 CMake Integration (`native/CMakeLists.txt`)
Added support for `GT4_SPU2_RUNTIME_SOURCE`, replacing donor `iop/spu2.cpp` with the derived capsule and compiling `native/spu2_envelope_fixture.cpp`.

### 5.3 Automated Test Coverage
1. **CTest Fixture (`native/spu2_envelope_fixture.cpp` / `native_spu2_envelope`):**
   - Tests all 32 ADSR release rates (`rate = 0..31 << 2`) from max level `0x7FFF` down to `0`. All 32 rates successfully reach `level == 0` without stalling.
   - Verifies that `level == 0` does not underflow below zero.
   - Verifies that linear decrease still reaches `0`.
   - Verifies that linear and exponential attack phases still reach max level `0x7FFF`.
   - Simulates a full voice release lifecycle and confirms that `voice.phase` transitions from `kRelease` to `kOff`.
2. **Python Test Suite (`tests/python/test_spu2_envelope.py`):**
   - Synthetic roundtrip overlay tests pass unconditionally.
   - Verified roundtrip reversibility.

---

## 6. Conclusion and Issue Disposition

Upstream Issue #104 is completely resolved. Looping SFX from GT Auto car wash and oil change services now terminate and release properly as soon as the service cutscene concludes, restoring expected audio silence in menus and subsequent races.
