# 0073 — European PAL (SCES-51719) is a separate target, not a drop-in ISO replacement

Date: 2026-10-31. Status: Accepted. **Recorded, not scheduled.**
Study: [PAL-SCES51719-STUDY](../PAL-SCES51719-STUDY.md).
Related: [Issue #128](https://github.com/moesuito/gt4-staticrecomp/issues/128), [Decision 0072](0072-spec2-second-target.md).

**Context.** In Issue #128, users asked for support for the European release of Gran Turismo 4 (PAL, `SCES-51719`).
When providing the PAL disc image to the GT4Recomp setup, it is refused with "This disc is not supported yet."

**Technical Reality.**
GT4Recomp is an ahead-of-time (AOT) static recompilation of the NTSC-U v2.00 release (`SCUS-97328`).
It contains no general-purpose MIPS EE JIT or dynamic interpreter on the gameplay path. The 326 static
C++ units in `ps2game.lib` were translated directly from the NTSC-U v2.00 `CORE.GT4` / reconstructed ELF.

The European release (`SCES-51719`):
1. **Has 0% code address parity with NTSC-U:** Functions, jump tables, and data structures are linked at
   different addresses due to multi-language string tables (English, French, German, Italian, Spanish) and
   compiler link variations.
2. **Cannot execute against NTSC-U translated units:** Calling an unmapped PAL guest address halts immediately
   with "no translated code".
3. **Hardcoded retail native hooks:** Native runtime hooks (HUD aspect ratio anchors, 480p mode overrides,
   license demo guards) inspect specific NTSC-U guest RAM offsets. Applying them to PAL memory corrupts guest state.
4. **VBlank and pacing:** PAL 50Hz mode requires dedicated 50 FPS / 20.0 ms VBlank frame clock pacing and audio
   rate adaptation to avoid desynchronization.

**Decision.**
- **European PAL (`SCES-51719`) cannot be supported by loading its ISO into the NTSC-U engine.**
- **PAL support requires a dedicated second target** (or multi-target pipeline):
  - PAL `CORE.GT4` decryption and `SCES_517.19` ELF reconstruction.
  - Generating ~330 dedicated AOT C++ units (`ps2game_pal.lib`) via the donor translator.
  - Re-targeting native VBlank hooks to PAL guest RAM addresses.
  - Calibrating 50Hz frame clock pacing and audio rate control.
- **Not scheduled.** Per AGENTS.md, Phase 2 (CoreGS 2.0, release 2.0.0) remains the priority. Like Spec II
  (Decision 0072), PAL is recorded as a future secondary/regional target.
- **Launcher Diagnostic Guidance:** The launcher explicitly recognizes known GT4 regional releases
  (`SCES-51719` Europe, `SCPS-15055` Japan, `SCKA-20001` Korea, `SCAJ-30007` Asia) and provides clear,
  polite educational guidance explaining that GT4Recomp is an AOT recompilation of the NTSC-U release.
