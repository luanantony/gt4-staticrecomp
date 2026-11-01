# Launcher "Prepare Shaders" & Pipeline Cache Warming Architecture (TASK-046)

## 1. Executive Summary & Problem Scope

In modern graphic APIs (Vulkan and Direct3D 12), graphics pipeline state objects (PSOs) encapsulate vertex layout, primitive topology, rasterization state, depth/stencil handling, blend equations, render pass formats, and compiled SPIR-V / DXIL shader stages. When a rendering engine encounters a state combination not previously compiled by the host graphics driver, the host driver must invoke its compiler backend synchronously on the calling thread.

In GT4Recomp's multi-process architecture ([Decision 0070](decisions/0070-pcsx2-gs-separate-process.md)), `gt4-gs.exe` (GPL GSdx fork) renders PS2 Graphic Synthesizer (GS) primitives submitted via shared-memory IPC by the statically recompiled game executable (`gt4vulkan.exe`).

### 1.1 Measured Impact of Cold Pipeline Cache
As benchmarked in [`docs/GS-LIVE.md`](GS-LIVE.md) (Section 9.3) and [`docs/FRAME-COST-STUDY.md`](FRAME-COST-STUDY.md) (Section 4.2), JIT shader compilations on an unprimed pipeline cache generate severe frame hitches:

| Run Condition | Readback Wait Mean | Readback Wait Max | Spikes $\ge 8\text{ ms}$ | Spikes $\ge 16\text{ ms}$ |
|---|---|---|---|---|
| **Cold Cache (Empty)** | $1.56\text{ ms}$ | **$58.0\text{ ms}$** | 12 of 294 readbacks | 5 of 294 readbacks |
| **Warm Cache (Seeded)** | $0.60\text{ ms}$ | **$2.7\text{ ms}$** | **0** | **0** |
| **Warm + GS Spin (300 $\mu\text{s}$)** | $0.72\text{ ms}$ | $3.2\text{ ms}$ | **0** | **0** |

### 1.2 The Flare Readback Coupling Mechanism
The PlayStation 2 Emotion Engine (EE) game thread in Gran Turismo 4 performs synchronous readbacks (`READ_FIFO` or `GS_CSR` waits) to sample headlight flare intensity and environment occlusion:
1. During night races (Special Stage Route 5, SSR11, George V Paris) and tunnel transitions (Trial Mountain, Deep Forest), the EE dispatches VU1 geometry and requests a $64 \times 8$ pixel transfer of rendered luminance.
2. The EE thread halts waiting for the GS process to complete all preceding draw calls up to the transfer boundary (`download sync`).
3. If an uncompiled pipeline selector is encountered during preceding draws (e.g. car headlights, particle sparks, specular reflections, motion blur passes), the host Vulkan/D3D12 driver compiles the pipeline on-the-fly.
4. This driver compilation stalls the GS worker thread for $8\text{ ms}$ to $150\text{ ms}$, which propagates immediately to the EE thread, causing visible micro-stutters and audio dropouts on the player's screen.

**TASK-046 Goal:** Implement an offline / launcher-driven shader pre-compilation pipeline ("Prepare shaders") that feeds a pre-extracted corpus of Gran Turismo 4 pipeline selectors to `gt4-gs.exe --warm-selectors`, populating `vulkan_pipelines.bin` before gameplay commences.

---

## 2. GS Pipeline Selector Architecture

### 2.1 The GSdx State Permutation Matrix
In PCSX2 GSdx (`GSDeviceVK.cpp` / `GSTextureCache.cpp`), rendering pipelines are identified by a 64-bit compact selector key (`GSPipelineSelector`). This key encodes:

```
+-------------------------------------------------------------------------------+
|                        GS PIPELINE SELECTOR BITFIELD                          |
+---------+---------+---------+---------+---------+---------+---------+---------+
| Bits 0-3| Bits 4-7|Bits 8-11|Bit 12-15|Bit 16-19|Bit 20-23|Bit 24-27|Bit 28-63|
|  PRIM   |  TEX0   |  FRAME  |  ALPHA  |   TEST  |   FOG   |   CLUT  | EXT/FLG |
+---------+---------+---------+---------+---------+---------+---------+---------+
```

Key dimensions generating permutation explosions:
* **Primitive Topology (`PRIM`):** Point, Line, LineStrip, Triangle, TriangleStrip, TriangleFan, Sprite.
* **Texture Storage Format (`TEX0.PSM`):** PSMCT32, PSMCT24, PSMCT16, PSMT8 (CLUT8), PSMT4 (CLUT4).
* **Alpha Blending Equations (`ALPHA`):** $(A - B) \times C + D$ with guest-selectable sources:
  * $A, B, D \in \{\text{Cs}, \text{Cd}, 0\}$, $C \in \{\text{As}, \text{Ad}, \text{FIX}\}$.
  * Color clamping, color wrapping, and dual-source blending configurations.
* **Pixel Testing (`TEST`):** Alpha test methods (NEVER, ALWAYS, LESS, LEQUAL, EQUAL, GEQUAL, GREATER, NOTEQUAL) with fail actions (KEEP, FB_ONLY, ZB_ONLY, RGB_ONLY), Depth test (ZTST), and Stencil write masks.
* **CLUT (Color Look-Up Table) Management:** 16-color / 256-color palette index formats, CLUT offset shifts, and entry masks.
* **Framebuffer Destination (`FRAME`):** 32-bit RGBA, 16-bit RGB555A1, half-pixel offsets, and channel masking.

### 2.2 Domain Bounding for Gran Turismo 4
While GSdx is capable of theoretically compiling millions of pipeline permutations across the entire PS2 library, Gran Turismo 4's engine (`CORE.GT4`) utilizes a fixed, highly optimized subset:
* Menus & UI: ~85 unique selectors (mostly 2D sprites, font glyph atlases, alpha-tested widgets).
* Day Time Trial / Arcade: ~240 unique selectors (asphalt, grass, skybox dome, car body shaders, tire treads, shadow projection).
* Night Races / Wet Weather: ~180 additional selectors (volumetric headlight cones, additive flare quads, wet road reflection masks, motion blur accumulation).
* **Total GT4 Selector Corpus:** Approximately **600 to 850 unique pipeline variants**.

---

## 3. Offline Cache Warming Protocol

```
+-------------------------------------------------------------+
|               Offline Corpus Extraction (Host)              |
|   Headless runs of Day, Night, Menus, Replays -> .gscap     |
|   tools/extract_selectors.py -> gt4_pipeline_selectors.bin  |
+------------------------------+------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|               Launcher / Setup ("Prepare Shaders")           |
|   Inspects [Folders] Cache / driver_fingerprint.json        |
|   Spawns: gt4-gs.exe --warm-selectors selectors.bin        |
+------------------------------+------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|                gt4-gs.exe (Headless GS Process)             |
|   1. Initializes Vulkan Instance & PhysicalDevice           |
|   2. Loads or creates VkPipelineCache (vulkan_pipelines.bin)|
|   3. Loops through selectors: compiles SPIR-V via shaderc   |
|   4. Calls vkCreateGraphicsPipelines()                      |
|   5. Flushes cache to disk -> Exits 0                       |
+------------------------------+------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|                     Gameplay Execution                      |
|   gt4vulkan.exe boots with hot, pre-compiled pipeline cache |
|   Zero driver compilation hitches during races / flares     |
+-------------------------------------------------------------+
```

### 3.1 Binary Corpus Specification (`gt4_pipeline_selectors.bin`)
The corpus file is a compact binary array bundled with the game distribution or generated during installation:
* **Header (16 bytes):**
  * Magic: `0x34534750` (`PGS4` in ASCII).
  * Version: `1` (uint32).
  * Selector Count: $N$ (uint32).
  * Reserved / Checksum: CRC32 of payload (uint32).
* **Payload ($N \times 16$ bytes):**
  * `selector_low`: uint64 (GS primitive, texture format, blending equation).
  * `selector_high`: uint64 (depth, stencil, fog, framebuffer format, multisample flags).

### 3.2 Headless GS Execution Mode (`--warm-selectors`)
When invoked with `--warm-selectors <path>`, `gt4-gs.exe`:
1. Activates `Surfaceless = true` (no Win32 window creation, no swapchain allocation).
2. Sets `[Folders] Cache` to `%LOCALAPPDATA%\GT4Recomp\gs\cache` (or `<install>\data\gs\cache`).
3. Reads the binary selector records sequentially.
4. Generates GLSL/SPIR-V vertex and fragment modules via embedded `shaderc`.
5. Submits `VkGraphicsPipelineCreateInfo` to the Vulkan driver backed by `VkPipelineCache`.
6. Calls `vkGetPipelineCacheData` and commits `vulkan_pipelines.bin` to disk upon completion.
7. Logs execution metrics to `gs_warm.log`:
   ```
   [GS-WARM] Loaded 742 selectors from gt4_pipeline_selectors.bin.
   [GS-WARM] Host GPU: NVIDIA GeForce RTX 3080 (Driver 560.81.0).
   [GS-WARM] Compiled 742 pipelines in 1,842 ms (0 failed).
   [GS-WARM] Pipeline cache written to C:/Users/.../vulkan_pipelines.bin (14.2 MB).
   ```

---

## 4. Launcher Integration Architecture

### 4.1 Driver Invalidation & Fingerprint Detection
Host graphics drivers invalidate internal pipeline caches whenever the GPU driver is updated or hardware changes. To maintain cache freshness, `gt4l-core` maintains a lightweight fingerprint file:
* File path: `%LOCALAPPDATA%\GT4Recomp\gs\cache\driver_fingerprint.json`.
* Schema:
  ```json
  {
    "vendor_id": 4318,
    "device_id": 8708,
    "driver_version": "560.81.0",
    "driver_name": "NVIDIA",
    "vulkan_api_version": "1.3.280",
    "selectors_sha256": "3a8f9c1b...",
    "cache_timestamp": 1730491200
  }
  ```

Before launching the game:
1. `gt4l-core` queries the current host GPU using Vulkan instance enumeration ([`system_check.rs`](file:///c:/Users/Antony/Desktop/gt4-staticrecomp/launcher/crates/gt4l-core/src/system_check.rs)).
2. Compares the live driver version and selector hash against `driver_fingerprint.json`.
3. If mismatched, or if `vulkan_pipelines.bin` is absent:
   * Status flags `ShaderCacheState::NeedsWarming`.
   * Prompts the player or schedules background warming.

### 4.2 User Experience & UI Workflow
The feature provides two operational paths:
1. **Initial Setup / Post-Update Prompt:**
   * After game installation or detected driver update, a non-intrusive banner appears:
     > *"New graphics driver detected. Optimizing shaders for stutter-free gameplay..."*
   * Displays an animated progress bar (0% to 100%, completing in ~2–4 seconds).
2. **Settings Screen Manual Trigger:**
   * Under **Settings > Graphics**, add a dedicated action:
     * **Button:** `[ Prepare Shaders ]` (`Otimizar Shaders`)
     * **Subtitle:** *"Pre-compiles graphics pipelines to eliminate in-race stuttering."*
3. **Non-Blocking Safety Rule:**
   * If the player chooses to click **Play** immediately, the background warming task terminates gracefully or yields; the game launches unhindered (missing pipelines will compile JIT as before).

### 4.3 Safety, Isolation & Licensing (Decision 0070 Compliance)
* **GPL Clean Room Boundary:** The launcher (`gt4l-core`, MIT) never links or shares code with PCSX2. It invokes `gt4-gs.exe` strictly via external child process execution (`std::process::Command`).
* **Fault Isolation:** If a buggy vendor driver crashes inside `vkCreateGraphicsPipelines` during `--warm-selectors`, the child process aborts with a non-zero exit code. The launcher catches the failure, records the error in `logs/shader_warm.log`, sets `ShaderCacheState::WarmingFailed`, and avoids crash-looping the user.

---

## 5. Acceptance Criteria & Quality Verification

| Requirement | Metric / Target | Validation Method |
|---|---|---|
| **Night Race Flare Stutter** | Max readback wait $< 3.0\text{ ms}$ on cold launch | Headless schedule (Arcade, El Capitan / SSR5, 16,000 frames). Zero frame waits $\ge 8\text{ ms}$. |
| **Race Start Stutter** | Initial grid render wait $< 5.0\text{ ms}$ | F8 performance probe interval on frame 0..300. |
| **Warm Execution Speed** | $\le 4.0\text{ seconds}$ on Ryzen 5 / Core i5 desktop | Automated benchmark timer during `--warm-selectors`. |
| **Idempotency** | Second run finishes in $< 300\text{ ms}$ | Re-running `--warm-selectors` detects warm `VkPipelineCache` and hits driver disk cache. |
| **Vendor Compatibility** | Zero validation errors or crashes | Tested on NVIDIA (Turing/Ampere/Ada), AMD (RDNA2/RDNA3), and Intel (Arc Alchemist). |

---

## 6. Phased Implementation Roadmap

* **Phase 1: Pipeline Selector Harvesting (`tools/gs_selectors/`):**
  * Implement `scripts/harvest_selectors.py` to parse recorded `.gscap` files and production dumps.
  * Emit deduplicated `gt4_pipeline_selectors.bin`.
* **Phase 2: GS Process CLI Flag (`gt4recomp-gs` Fork):**
  * Add `--warm-selectors <file>` handling in `pcsx2-gt4gs/Main.cpp`.
  * Wire headless pipeline generation through `GSDeviceVK::CreatePipeline`.
* **Phase 3: Launcher Detection & Worker (`gt4l-core`):**
  * Implement `gt4l_core::shaders::CacheManager` and fingerprint validation in Rust.
  * Add unit tests for fingerprint serialization, hash comparison, and path resolution.
* **Phase 4: Launcher UI Integration (`gt4l-tauri`):**
  * Expose `prepare_shaders` command in Tauri backend.
  * Add UI button in Settings > Graphics with bilingual labels (`en-US` and `pt-BR`).
