# 0073 — Modding scope, data patches, and GT4.VOL virtual overlay

Date: 2026-11-01. Status: Accepted (owner policy & triage disposition).
Study: [TASK-057-MOD-SCOPE-STUDY](../TASK-057-MOD-SCOPE-STUDY.md) (TASK-057, Issue #87).

**Context.** The Gran Turismo 4 community requests expanded modding capabilities beyond external texture
replacement (TASK-050..055), including graphics presets, translations, custom car physics, community content
expansions, and memory cheats (Issue #87). The repository requires a definitive architectural disposition on what
mod types are technically feasible and permitted under GT4Recomp's static recompilation architecture, and what
remains permanently excluded.

**Decision.**
1. **Code patches (`.pnach` EE MIPS writes) and Cheats (Issue #87) are REJECTED and NOT PLANNED.**
   - Gran Turismo 4's Emotion Engine code is compiled ahead-of-time (AOT) into native C++ units (`unit_XXXX.cpp`).
     There is no EE JIT or interpreter on the normal path. Writing MIPS assembly opcodes to guest RAM is a silent
     no-op; dynamic re-translation at runtime violates project rules (`AGENTS.md`: "No JIT ever") and distribution
     architecture ([Decision 0069](0069-own-hardware-renderer-clean-room.md)).
   - Memory cheats (infinite credits, physics freezes) corrupt memory card saves, desynchronize the fixed 60 Hz
     physics loop (`RaceBase__updateRace`), and conflict with game preservation. Issue #87 is formally closed.
2. **`gs-preset` mods (Graphics Profiles) are ACCEPTED for Release 1.1.0.**
   - Curated bundles of approved GS settings rows (`gs-settings.toml`) validated strictly against the 58-row schema
     in `gt4l-core::gs_table`. Prohibited internal IPC and executable path keys remain locked out. Handled entirely
     within the launcher with zero game executable changes.
3. **Read-only `GT4.VOL` Virtual Sector Overlay is ACCEPTED IN PRINCIPLE for Phase 2 / Spec II / 1.1.0+.**
   - Supports data mods (translations such as pt-BR `rtext.rt2`, car physics `.cdat`, menu scripts `.adc`, custom
     HUD `.gpb`) without altering the user's ISO image.
   - Implemented via a host-level sector interception wrapper in `disc::read` (`disc.cpp:86` custom reader seam).
     The virtual reader serves an in-memory patched RoFS Table of Contents (TOC) and redirects requests for modified
     file extents to loose mod files in `%LOCALAPPDATA%\GT4Recomp\mods\active\vol\`.
   - The original user disc remains bit-exact and untouched, preserving input verification pins (`docs/inputs/*.json`).
   - The repository never ships derived game assets; mod authors distribute patches/loose files independently.
4. **RAM Data Patches are RESTRICTED to Bounded Typed Hooks.**
   - Generic, unrestricted RAM patchers are rejected. When high-value configuration adjustments are needed (such as
     custom camera FOV scaling or HUD layout parameters), they must be implemented as audited, typed native runtime
     contracts (following the pattern of [Decision 0071](0071-native-aspect-ratios-runtime-hooks.md)).

**Consequences.**
- Issue #87 is formally closed with documented technical justification.
- The launcher mod manager roadmap (1.1.0) is bounded to: (1) Texture Packs, (2) GS Presets, and (3) Virtual VOL Overlay.
- Preserves the integrity of the fixed 60 Hz physics loop, AOT recompilation model, and pre-built distribution pipeline.
- Modders gain a clean, non-destructive path to translate menus and tune vehicle physics without modifying disc images.
