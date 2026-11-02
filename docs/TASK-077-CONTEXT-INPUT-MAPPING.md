# TASK-077: Context-Aware Input Mapping (Menus vs Race)

## Overview
- **Task ID:** TASK-077
- **Goal:** Context-aware input mapping separating race and menu controls (e.g. `Esc` acting as `Start` / pause in races, while functioning as `Triangle` / back in menus and car garages), with seamless schema 2 configuration and state transition filtering (re-arm rule and clean transition frames).
- **Study Reference:** [`docs/CONTEXT-INPUT-STUDY.md`](CONTEXT-INPUT-STUDY.md)
- **Controls Reference:** [`docs/CONTROLS.md`](CONTROLS.md)
- **Status:** Implemented, verified with unit fixtures and integration suites.

---

## Architecture & Memory Detection

### 1. Host-Side VBlank Probe (`native/context_probe.{hpp,cpp}`)
Following the proven, range-checked memory read template established by `GT4_FORCE_480P` (`native/race_display.cpp`), the host performs safe, read-only inspection of guest RAM at VBlank boundaries without patching game code or translated unit functions:
- **Game Owner Pointer:** Fixed guest address `0x622F4C` (word checked with `word_in_ram`).
- **Race Object Discovery:**
  - Bounded inspection from `owner_pointer`: searches candidate object slots for known virtual table pointers (`vtable`).
  - **Identified Race Vtables** (from decompilation headers and reconstructed ELF `SCUS_973.28`):
    - `RaceSinglePlayer` (`0x682A20`)
    - `RaceArcade` (`0x67BFA8`)
    - `RaceArcadeSingle` (`0x67BAD8`)
    - `RaceGTmode` (`0x684678`)
    - `RaceFreeRun` (`0x680140`)
    - `RaceLicense` (`0x6807A8`)
    - `RaceSolitaire` (`0x683420`)
    - `RacePhotoDevelop` (`0x682310`)
    - `RacePhotoMode` (`0x681C08`)
    - `RacePS2Base` (`0x679378`)
  - **Attract Demo:** `RaceArcadeDemo` (`0x67C520`) is recognized and logically mapped to `Menu` (attract sequences should not hijack menu keys).
  - **Cached Offset Acceleration:** Once the race object is discovered at `owner + offset`, the offset is cached and validated in O(1) on subsequent frames.
- **Pause & Photo Mode Detection:**
  - Active pause object pointer lives at `race_object + 0xE418`.
  - Pause vtables inspected at `pause_object + 0x8`:
    - `RacePause` (`0x681B28`) -> `GameContext::Pause`
    - `SimplePause` (`0x681B60`) -> `GameContext::Pause`
    - `PhotoPause` (`0x681B98`) -> `GameContext::PhotoPause`
- **Debounce State Machine:**
  - Transitions require 3 consecutive identical VBlank samples (`debounce_threshold = 3`) to eliminate transient intermediate states across race start/finish frames.

---

## Context Transition Rules (`native/context_input.{hpp,cpp}`)

To ensure no button ever gets stuck or accidentally doubles its action across scene transitions:
1. **Re-arm Rule:**
   - Any physical key (keyboard VK) or gamepad button held down at the exact frame of a context switch is marked as requiring re-arm (`needs_rearm`).
   - It will not trigger its new context mapping until it has been physically released (`raw == 0`) and pressed again.
   - *Example:* Holding `Esc` to pause a race will NOT immediately trigger `Triangle` / back in the pause menu. The player must release `Esc` and press it again.
2. **Clean Transition Frame:**
   - On the exact frame a context transition occurs, all logical outputs are emitted as neutral (all buttons released).
3. **Loading Screen Neutral State:**
   - While context is `Loading`, all inputs are filtered out to prevent input accumulation across load boundaries.

---

## Configuration Schema 2 (`native/controls.{hpp,cpp}`)

Backward compatibility is preserved 100%: `schema = 1` files continue to function identically with a single `[bindings]` table.
Under `schema = 2`, optional context override tables are supported:
- `[bindings]` (base table, required)
- `[bindings.race]`
- `[bindings.pause]`
- `[bindings.menu]`
- `[bindings.photo_pause]` (or `[bindings.photo]`)
- `[bindings.replay]`
- `[bindings.showroom]`

Inputs not specified in a context table automatically inherit from `[bindings]`. An empty array `[]` explicitly unbinds the input for that context.

### Built-in Default Mappings:
- **Base / Menu:**
  - `triangle`: `["xinput:Y", "key:Backspace", "key:Esc"]`
  - `start`: `["xinput:Start", "key:P"]`
- **Race Context:**
  - `start`: `["key:Esc", "xinput:Start", "key:P"]` (`Esc` opens pause)
  - `triangle`: `["xinput:Y", "key:Backspace"]` (`Esc` removed so it does not trigger rear view/look back)
- **Pause Context:**
  - `triangle`: `["key:Esc", "key:Backspace", "xinput:Y"]` (`Esc` backs out or resumes)
  - `start`: `["key:Esc", "xinput:Start", "key:P"]`

---

## Verification Evidence

1. **Native C++ Fixtures (`clang-cl /std:c++20 /W4 /WX`):**
   - `context_probe_fixture.exe`:
     - `test_names_and_parsing`: PASSED
     - `test_vtable_predicates`: PASSED (all 10 race vtables, attract demo, pause vtables verified)
     - `test_synthetic_ram_detection`: PASSED (Menu, GTmode, cached offset, Pause, PhotoPause, Attract demo)
     - `test_debounce_state_machine`: PASSED (3-frame debounce verified, glitch rejection verified)
     - `test_context_input_filter`: PASSED (re-arm rule, transition clean frame, loading lock verified)
     - `test_default_context_controls_mappings`: PASSED
   - `controls_fixture.exe`:
     - All 8 controls test suites passed (defaults, triggers, axes, remaps, schema 2 context tables, refusals).
2. **Launcher / Rust Test Suite:**
   - `cargo test --manifest-path launcher/Cargo.toml -p gt4controls`: 17 passed, 0 failed.
   - `cargo test --manifest-path launcher/Cargo.toml -p gt4controls --test docs_match`: 4 passed, 0 failed.
3. **Python Verification Suite:**
   - `python -m unittest tests/python/test_controls_probe.py`: 7 tests passed (covering schema 1, schema 2, context subtables, and refusal of invalid tables/schemas).
