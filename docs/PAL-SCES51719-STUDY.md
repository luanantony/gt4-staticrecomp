# Gran Turismo 4 European Release (PAL SCES-51719) — Target Study

Date: 2026-10-31. Technical triage and feasibility study for Issue #128 (SCES-51719).
[C] = Confirmed, [H] = Hypothesis, [U] = Unknown.

## Context

In [Issue #128](https://github.com/moesuito/gt4-staticrecomp/issues/128), user Loseph888 reported:
"When trying to run GT4Recomp with the European release of Gran Turismo 4 (SCES-51719), the tool stops
with the error: 'This disc is not supported yet'. The USA release (SCUS-97328) works correctly on the same
setup... Would it be possible to add support for the European release (SCES-51719)?"

Many players mistakenly assume GT4Recomp functions like PCSX2 or a dynamic emulator that can load any
PS2 ISO. In reality, GT4Recomp is an **ahead-of-time (AOT) static recompilation** of Gran Turismo 4
NTSC-U (disc labeled v2.00, `SCUS-97328`).

This document records the technical reality of supporting the European release, why it cannot be loaded
into the existing NTSC-U static recompilation engine, and the architecture required for a future PAL target.

## Identity & Structural Differences [C]

- **Serial and Boot:**
  - European release serial: `SCES-51719` (retail PAL; promo discs carry `SCED-51719`).
  - Boot executable: `SCES_517.19` (or encrypted with Polyphony's Salsa-like cipher in `CORE.GT4`).
  - `SYSTEM.CNF`:
    ```ini
    BOOT2 = cdrom0:\SCES_517.19;1
    VER = 1.00 (or 1.01, 2.00)
    VMODE = PAL
    ```
- **Language and Asset Layout:**
  - PAL features Multi-5 localization (English, French, German, Italian, Spanish).
  - Localization strings and font tables shift data and text segments by tens of kilobytes compared to NTSC-U.
- **Guest Executable Parity:**
  - 0% address parity with `SCUS_973.28`. Function entry points, jump tables, and static variable addresses
    are linked at completely different virtual memory locations.
  - The gt4-decomp reference symbols and memory maps do not directly apply to `SCES_517.19`.

## Code Pipeline & Static Recompilation Constraints [C]

1. **AOT Recompiled Units:**
   - In GT4Recomp, the EE pipeline translates `CORE.GT4` ahead of time into 326 C++ units (`unit_XXXX.cpp`),
     compiled into `ps2game.lib`.
   - Dispatch tables (`gen::kUnits`) route guest program counter (PC) values to translated C++ labels.
   - Any guest PC address called that is not part of the translated unit set halts immediately with "no translated code".
   - Because `SCES_517.19` has different function boundaries and entry addresses, feeding PAL code to the
     NTSC-U translated units causes an immediate halt or illegal memory access.

2. **Retail Native Hooks:**
   - The native engine (`native/window.cpp`, `native/perf_probe.cpp`, etc.) relies on specific v2.00 NTSC-U
     memory addresses probed or patched at VBlank (e.g. aspect ratio HUD anchors, 480p mode overrides, widescreen hold,
     and license demo guards).
   - Writing to these hardcoded offsets on a PAL layout would corrupt guest RAM.

3. **Frame Timing & Pacing (50Hz vs 60Hz):**
   - European PS2 games default to PAL 50Hz (50 FPS, 20.0 ms VBlank period), though GT4 PAL also features an
     in-game 60Hz mode selector.
   - GT4Recomp's single-clock frame pacing (`native/pacing.cpp`, `native/frame_clock.cpp`) is calibrated
     strictly to 59.94 / 60 Hz NTSC timing. Running 50Hz guest code under an unadapted 60Hz clock causes
     severe audio buffer underruns, stutter, and physics rate desynchronization.

## Microcode [H]

- Similar to the Spec II findings ([SPEC2-STUDY.md](SPEC2-STUDY.md)), VU1 microcode is likely identical or nearly
  identical to retail NTSC-U, meaning the VU1 AOT table could largely carry over.
- VU0 microcode and IOP modules would require independent verification.

## Architecture Required for European (PAL) Support

European PAL support cannot be achieved by swapping ISOs or data files. It requires a dedicated
second target (or multi-target build pipeline), following the precedent set in
[Decision 0072](decisions/0072-spec2-second-target.md):

1. **Decryption and Reconstruction:** Decrypt PAL `CORE.GT4` / reconstruct `SCES_517.19` ELF.
2. **Static Translation:** Run the donor translator / EE producer to emit a separate set of ~330 C++ units
   for PAL (`ps2game_pal.lib`).
3. **Target Architecture:** Either a separate target executable (`gt4vulkan_pal.exe`) or namespaced unit
   dispatching in a unified binary.
4. **Hook Re-targeting:** Map all native VBlank hooks and aspect ratio adjustments to PAL memory addresses.
5. **Frame Pacing Adaptation:** Support 50Hz VBlank pacing and audio sample rate matching alongside 60Hz.

## Recommendation & Status

- **Status:** Recorded and triaged under TASK-141. Not scheduled for immediate implementation.
- **Priority:** Per AGENTS.md, Phase 2 (CoreGS 2.0, release 2.0.0) remains the priority. Spec II (`SCUS-97436`)
  is the recorded second target (Decision 0072), and PAL (`SCES-51719`) is a regional target candidate for post-2.0.0.
- **Launcher UX:** In the meantime, the launcher provides clear, polite, and educational diagnostics
  when an `SCES-51719` disc is selected, explaining why it cannot be booted by the NTSC-U recompilation engine.
