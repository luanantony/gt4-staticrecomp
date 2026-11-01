# Modding Scope, Data Patches & GT4.VOL Overlay Architecture (TASK-057 / Issue #87)

## 1. Executive Summary & Problem Scope

Gran Turismo 4 has an active modding community with desires ranging from visual enhancement to complete content expansions (e.g., custom vehicle physics, translated menus, Spec II beta restoration, and memory cheats).

This study delivers the architectural triage and feasibility evaluation for **TASK-057** ("Study: other mod kinds") and resolves community request **Issue #87** ("Cheats / PNACH support").

### 1.1 Summary Matrix of Mod Categories

| Mod Category | Examples | Feasibility in GT4Recomp | Verdict & Release Target | Technical Mechanism |
|---|---|---|---|---|
| **1. Texture Packs** | HD UI, 4K car liveries, track re-textures | High (implemented in GSdx) | **Accepted** (Release 1.1.0, TASK-050..055) | GSdx `[Folders] Textures` replacement pipeline |
| **2. GS Presets** | "CRT Scanlines", "Ultra 4K", "Crisp 1080p" | High (launcher configuration) | **Accepted** (Release 1.1.0, low effort) | Validated `gs-settings.toml` bundles |
| **3. VOL Data Mods** | Community pt-BR translation, car specs, AI tuning | High (via host disc reader) | **Accepted in Principle** (Phase 2 / 1.1.0+) | Read-only `GT4.VOL` virtual sector overlay |
| **4. RAM Data Patches** | Custom camera FOV factor, HUD toggles | Medium (requires typed hooks) | **Restricted to Bounded Hooks** (TASK-071 model) | Explicit native runtime hooks (no raw pnach) |
| **5. Code Patches / Cheats (Issue #87)** | Infinite credits, license auto-pass, `.pnach` MIPS writes | **Impossible / Incompatible** | **Rejected / Not Planned** | Silent no-op on AOT; violates project rules |

---

## 2. Code Patches & Memory Cheats (Issue #87 / `.pnach`) — Infeasibility Analysis

### 2.1 The Static AOT Recompilation Reality
In traditional PS2 emulators (such as PCSX2), guest MIPS code resides in virtual EE RAM (`0x00100000..0x01FFFFFF`) and is executed dynamically by a Just-In-Time (JIT) recompiler or interpreter. In that paradigm, modifying bytes in guest RAM via `.pnach` files patches the guest instructions before the JIT translates them.

In **GT4Recomp**, the execution model is completely different:
1. Gran Turismo 4's Emotion Engine code was translated **ahead-of-time (AOT)** from the reconstructed ELF (`SCUS_973.28.elf`) into 326 C++ translation units (`unit_XXXX.cpp`), compiled into native x86-64 machine code, and linked into `ps2game.lib`.
2. **There is no EE JIT compiler and no EE interpreter** on the normal execution path ([`AGENTS.md`](file:///c:/Users/Antony/Desktop/gt4-staticrecomp/AGENTS.md)).
3. Guest RAM exists strictly as a flat data backing store (`g_ram`).
4. When a `.pnach` patch or cheat engine writes MIPS machine code instructions to guest RAM addresses, **those writes are completely ignored**. The host CPU continues executing the compiled native C++ functions. Writing assembly opcodes to guest RAM is a **silent no-op**.

### 2.2 Why Runtime Code Retranslation Is Prohibited
To make guest assembly patches functional at runtime, the engine would have to:
* Re-translate MIPS basic blocks dynamically on the fly (introducing a JIT, which is strictly forbidden by project rules: *"No JIT ever: guest code (EE, VU, IOP) is compiled ahead of time per target"*).
* Or require the end user to have a full LLVM/Clang C++ build toolchain installed to recompile `unit_XXXX.cpp` on demand. This contradicts [Decision 0069](decisions/0069-own-hardware-renderer-clean-room.md), where the game ships as a pre-built native standalone executable and the player merely supplies their disc.

### 2.3 Cheats Policy (Issue #87 Disposition)
* **Save File & State Invariants:** Arbitrary memory freezing (e.g. freezing credit counters or race lap timers) bypasses the game's internal transactional checks, leading to memory card save corruption (`sio2.cpp` atomic writer, tested in TASK-025/TASK-058).
* **Physics Desynchronization:** Poking memory in the middle of a race tick can desynchronize the fixed 60 Hz physics loop (`RaceBase__updateRace`, $dt = 1/60\text{ s}$).
* **Disposition:** **Issue #87 is formally closed as Not Planned.** GT4Recomp will not implement arbitrary cheat engines or `.pnach` assembly patchers.

---

## 3. `gs-preset` Mods (Visual & Post-Processing Profiles)

### 3.1 Concept & Value
Players frequently request curated visual presets for different display setups:
* **CRT Simulation:** 1x internal resolution, TVShader scanlines, Bilinear filtering.
* **Modern Clean 4K:** 6x internal resolution, CAS sharpening, FXAA, anisotropic filtering 16x, high blending accuracy.
* **Steam Deck / Low-End:** 2x resolution, fast VSync (mailbox), reduced blending unit load.

### 3.2 Implementation Architecture
A `gs-preset` mod is a lightweight archive containing a `gt4mod.toml` manifest:
```toml
[mod]
id = "crt-trinitron"
name = "CRT Trinitron Scanlines Preset"
version = "1.0.0"
kind = "gs-preset"
author = "Community"

[gs-settings]
upscale_multiplier = 1
deinterlace_mode = 0
accurate_blending_unit = 2
tv_shader = 1
```

### 3.3 Safety & Validation
* **Schema Validation:** Ingested via `launcher/crates/gt4l-core/src/gs_table.rs`. Values must belong to the approved 58-row settings table ([`docs/GS-SETTINGS.md`](GS-SETTINGS.md)).
* **Prohibited Keys:** Presets cannot overwrite critical IPC parameters (`GT4_GS_PROCESS`, `GT4_GS_CONFIG`, internal IPC ring buffer sizing, or system display holds).
* **Effort & Risk:** Very low effort (~1 session in launcher mod manager); zero changes to the game executable.

---

## 4. Read-Only `GT4.VOL` Virtual Sector Overlay (Data Mods)

### 4.1 Anatomy of `GT4.VOL`
On the PlayStation 2 disc, all major game assets reside within the root volume container `/GT4.VOL;1` ($2,459,502,592\text{ bytes}$ at LBA 105879 for NTSC-U v2.00):
* **Format:** Polyphony Digital Read-only File System (RoFS v3.0 / v3.1).
* **Table of Contents (TOC):** Stored near the start of the volume, compressed with raw DEFLATE and obfuscated with a repeating XOR `0x55` cipher.
* **Payloads:** Contains 26,891 entry records (18,352 unique asset files):
  * Kind 1: Raw uncompressed sectors.
  * Kind 2: PS2ZIP compressed streams (header + DEFLATE payload).
* **Asset Types:** Car physics tables (`.cdat`), car color tables, engine sound samples, menu scripts (`.adc`), UI layouts (`.mproject`), localized strings (`rtext.rt2`), and track geometry.

### 4.2 The Virtual Overlay Architecture (Host Sector Interception)

Modifying the user's ISO directly is strictly avoided because:
1. It invalidates disc verification hashes and setup receipts (`docs/inputs/*.json`, `prepare.json`).
2. Disc rebuilding tools frequently introduce sector alignment errors or break PS2 layer breaks.

Instead, GT4Recomp implements a **Host-Side Virtual Sector Overlay**:

```
                              +---------------------------------------+
                              |         EE Game Engine (AOT)          |
                              |   Calls disc::read(lba, count, buf)   |
                              +-------------------+-------------------+
                                                  |
                                                  v
                              +---------------------------------------+
                              |      VolOverlayReader (Host Hook)     |
                              |       Intercepts sector requests      |
                              +-------------------+-------------------+
                                                  |
                         +------------------------+------------------------+
                         |                                                 |
                [Request for TOC / Overlaid Files]                [Unmodified Data]
                         |                                                 |
                         v                                                 v
         +-------------------------------+                 +-------------------------------+
         |     In-Memory Patched TOC     |                 |       Original ISO9660        |
         |              OR               |                 |     /GT4.VOL;1 on Disc        |
         | Loose file in %LOCALAPPDATA%  |                 | (Streamed via File Descriptor)|
         +-------------------------------+                 +-------------------------------+
```

### 4.3 Mechanics of Sector Interception
1. **Pluggable Disc Hook:** The donor runtime already provides a pluggable disc abstraction:
   ```cpp
   disc::open(name, size, Reader, Probe); // disc.cpp:86
   ```
2. **Virtual TOC Generation:**
   * At game launch, if active VOL mods exist in `%LOCALAPPDATA%\GT4Recomp\mods\active\vol\`, the launcher compiles an overlay index.
   * Modded files (e.g. `rtext.rt2` for Brazilian Portuguese translation) are compressed to PS2ZIP format.
   * A patched TOC is generated in memory: the entry for `rtext.rt2` is updated with the new compressed size and a virtual LBA offset assigned in an extended virtual address range (e.g. LBA $\ge 2,000,000$).
3. **Sector Dispatch:**
   * When `disc::read` requests TOC sectors, the virtual reader serves the patched in-memory TOC buffer.
   * When `disc::read` requests sectors within the original ISO extents, reads pass directly to `std::ifstream` of the disc.
   * When `disc::read` requests sectors within the virtual LBA range, the virtual reader reads bytes from the corresponding loose file on host disk:
     `%LOCALAPPDATA%\GT4Recomp\mods\active\vol\<mod_id>\rtext.rt2`.

### 4.4 Legal and Distribution Safety
* **Zero Asset Redistribution:** GT4Recomp distributes **no copyright-protected assets**.
* **Clean Community Mod Distribution:** Mod authors distribute only their modified files (or delta binary diffs).
* The user's original disc remains 100% pristine and unmodified.

---

## 5. RAM Data Patches vs. Bounded Runtime Hooks

### 5.1 The Danger of Unbounded Memory Poking
Some community mods request generic memory patchers to modify static variables in RAM (e.g., changing vehicle grip multipliers or camera angles).
* **Risks:** Unvalidated memory writes can point to uninitialized heap structures, corrupt pointer tables, or crash the game during menu transitions.

### 5.2 The Bounded Hook Precedent (TASK-071)
The proven, stable pattern in GT4Recomp is **Typed Native Runtime Hooks**:
* In [Decision 0071](decisions/0071-native-aspect-ratios-runtime-hooks.md) (TASK-080, Ultrawide support), rather than allowing arbitrary RAM writes, a specific hook was inserted at the camera dispatch boundary (`RaceBase::initMonitor`, address `0x00388A80`), scaling the projection matrix cleanly with mathematical precision.
* **Policy for Data Patches:** Unbounded `.pnach` RAM patchers are rejected. When a specific, high-value data modification is desired (such as custom HUD positions or camera FOV factors), it must be implemented as an audited, typed runtime contract.

---

## 6. Phased Implementation Roadmap

* **Phase 1: `gs-preset` Integration (Release 1.1.0):**
  * Support `kind = "gs-preset"` in launcher mod manager.
  * Validate against `gs_table.rs` allowed keys.
* **Phase 2: Offline TOC & PS2ZIP Tooling (`tools/vol_tool/`):**
  * Extend `scripts/gt4_volume.py` (slice 120) with TOC repacking and PS2ZIP DEFLATE compression routines.
  * Build synthetic test fixtures verifying that repacked TOCs match the game's RoFS parsing invariants.
* **Phase 3: Native `VolOverlayReader` Hook (`native/disc/`):**
  * Implement the virtual sector interceptor in C++ conforming to `disc.cpp:86`.
  * Verify seamless transparent reading of community Portuguese translation (`rtext.rt2`).
* **Phase 4: Launcher Mod Manager VOL Tab (`gt4l-tauri`):**
  * Expose VOL mods alongside Texture Packs in launcher UI.
  * Implement conflict resolution (higher mod in priority order overrides lower mods).
