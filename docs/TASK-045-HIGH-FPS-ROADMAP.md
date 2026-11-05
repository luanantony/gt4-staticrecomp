# High-FPS Presentation Roadmap & GSdx Decoupling Architecture (TASK-045 / TASK-031)

## 1. Executive Summary & Problem Scope

Gran Turismo 4 was designed around a fixed 60 Hz vertical refresh rate on the PlayStation 2. The simulation, vehicle dynamics, suspension physics, tire models, opponent AI, and timekeeping are strictly coupled to a constant delta time:
$$\Delta t = \frac{1}{60} \text{ s} \approx 0.0166667 \text{ s}$$

As established in [`docs/HIGH-FPS-STUDY.md`](docs/HIGH-FPS-STUDY.md) and [`AGENTS.md`](AGENTS.md):
> *"Presenting more than 60 FPS while the game's fixed 1/60 s physics stays untouched is in scope (TASK-045 on the PCSX2 GS, after 1.0.0; TASK-031 study; owner, 2026-10-15). Decoupling physics (a different dt) is out of scope."*

This document establishes the architectural roadmap for **TASK-045** ("High frame rate on the PCSX2 GS"), adapting the earlier single-process study (TASK-031) to GT4Recomp's multi-process architecture ([Decision 0070](docs/decisions/0070-pcsx2-gs-separate-process.md)).

---

## 2. Pacing & Presentation Architecture

### 2.1 The Two-Process Split (Decision 0070)
GT4Recomp operates with clean separation of concerns across two distinct processes:
1. **Game Process (`gt4-staticrecomp.exe` - MIT):**
   - Statically recompiled Emotion Engine (EE), VU0/VU1 microcode, and native IOP services.
   - Executes guest code at 60 VBlanks per second ($4,915,200$ EE cycles per frame at $294.912$ MHz).
   - Generates Graphic Synthesizer (GS) primitive commands and serializes them into a shared-memory IPC ring buffer.
2. **Renderer Process (`gt4-gs.exe` - GPL):**
   - Fork of PCSX2 GSdx (`moesuito/gt4recomp-gs`).
   - Reads the shared-memory GS stream, rasters primitives via Vulkan or Direct3D 12, and manages the host display swapchain.

### 2.2 The Frame Clock & Pacing Grid
Host-side timing is governed by a single master clock ([`native/pacing.cpp`](file:///c:/Users/Antony/Desktop/gt4-staticrecomp/native/pacing.cpp) and [`native/frame_clock.cpp`](file:///c:/Users/Antony/Desktop/gt4-staticrecomp/native/frame_clock.cpp)):
* In `Mode::audio`, the frame clock is rate-controlled against host audio buffer consumption (800 stereo samples per frame at 48 kHz).
* In `Mode::paced`, a high-resolution waitable multimedia timer (`QueryPerformanceCounter`) maintains exact 16.666 ms frame pacing intervals.
* VBlank hooks (`window.cpp:on_vblank`) coordinate the presentation barrier.

---

## 3. High-FPS Modes of Operation

To achieve 120 Hz, 144 Hz, or Variable Refresh Rate (VRR / FreeSync / G-Sync) presentation without mutating game physics, GT4Recomp defines four distinct operational tiers:

```
                    +------------------------------------+
                    | Game Physics & EE Engine (Fixed 60Hz)|
                    |         dt = 1/60 s constant       |
                    +-----------------+------------------+
                                      |
                     [Shared-Memory IPC Stream Ring]
                                      |
               +----------------------+----------------------+
               |                                             |
     [Mode 1: VRR Mailbox]                       [Mode 2: GS-Stream AUX_FRAME]
   60 FPS Sim -> Low Latency Swap             Holds Frame N+1, Lerps Primitives
   No interpolation, minimal lag              True 120Hz/144Hz Rasterization
               |                                             |
               +----------------------+----------------------+
                                      |
                        +-------------v-------------+
                        |   gt4-gs.exe (GSdx Fork)  |
                        | Swapchain: 120Hz / 144Hz  |
                        +---------------------------+
```

### 3.1 Mode 1: FreeSync / G-Sync VRR & Mailbox Swapchain (Tier 1)
* **Principle:** Game runs at 60 FPS simulation, but the swapchain presentation mode is set to `VK_PRESENT_MODE_MAILBOX_KHR` (or `DXGI_SWAP_EFFECT_FLIP_DISCARD` with zero wait).
* **Benefits:**
  * Zero latency penalty (no held frames).
  * On a 120 Hz or 144 Hz FreeSync/G-Sync monitor, frames present immediately upon completion rather than waiting for the next 60 Hz VSync edge.
  * Reduces end-to-end controller input lag by up to ~8–12 ms.
* **Risk & Complexity:** Extremely low; configurable entirely via `GT4_PRESENT_MODE=mailbox` or launcher settings.

### 3.2 Mode 2: Decoupled GS-Stream Primitive Interpolation (Tier 2)
* **Principle:** The host retains the post-VU1 primitive stream for frame $N$ and frame $N+1$.
* **Algorithm:**
  1. The game process delays presentation by exactly one frame (buffer latency = 16.7 ms).
  2. For a target of 120 Hz, an auxiliary intermediate frame at $t = 0.5$ is synthesized:
     * Primitives are correlated by draw context signature (PRIM, FRAME, ZBUF, TEX0).
     * Vertex screen-space coordinates $(X, Y, Z)$ are linearly interpolated:
       $$P(t) = (1 - t) P_N + t P_{N+1}$$
     * Texture perspective coordinates $\left(\frac{S}{Q}, \frac{T}{Q}, \frac{1}{Q}\right)$ and vertex colors $(\text{RGBA})$ are interpolated in screen space.
  3. The synthesized stream is tagged with `STREAM_OP_AUX_FRAME` and pushed across the IPC ring.
  4. `gt4-gs.exe` rasters the auxiliary frame and executes a swapchain present.
  5. The real frame $N+1$ is then rendered, overwriting the frame buffer and executing any guest readbacks.
* **Benefits:** Real rasterized geometry; crisp high-resolution 3D edges without neural or optical flow temporal smearing.
* **Handling Special Elements:**
  * **2D HUD & OSD:** Numbers and lap timers must not be interpolated. HUD draw batches are identified by their orthographic matrix and drawn either from $N$ or $N+1$ without interpolation.
  * **Readbacks & Night Races:** Auxiliary frames do not trigger guest DMA readbacks or interrupt signals.

### 3.3 Mode 3: Late-Stage Camera Reprojection (Tier 3)
* **Principle:** A compute pass reprojects the rendered color and depth buffer using the delta camera view-projection matrix $\Delta M = M_{N+1} M_N^{-1}$.
* **Characteristics:** Extremely fast ($< 0.5\text{ ms}$ GPU time), zero extra geometry passes, but susceptible to edge disocclusion artifacts around the vehicle silhouette.

### 3.4 Mode 4: Host Driver / Presentation Interop (Tier 4)
* Interoperability with host-level frame generation (AMD FSR 3 Frame Generation, Lossless Scaling, Special K) via clean borderless window swapchain presentation.

---

## 4. Implementation Plan & Milestones

| Milestone | Component | Scope & Deliverables | Estimated Sessions |
|---|---|---|---|
| **M1: VRR Controls** | Launcher & Native | Expose `GT4_PRESENT_MODE` (`fifo`, `mailbox`, `immediate`) in launcher UI and verify frame pacing metrics. | 1–2 |
| **M2: Offline Oracle** | `tools/gs_interp` | Offline test fixture analyzing real `.gscap` race captures; measure primitive match rate and PSNR gate ($\ge 32\text{ dB}$, $\ge 95\%$ match). | 2–3 |
| **M3: Protocol Extension** | IPC Ring (`gt4-stream`) | Add `STREAM_OP_AUX_FRAME` to IPC protocol in both `native/gs_stream.hpp` and `gt4recomp-gs` fork. | 2 |
| **M4: Auxiliary Dispatch** | Native Engine | Implement frame $N+1$ hold, primitive matching, and lerp generation in `native/gs_interp.cpp`. | 4–6 |
| **M5: Pacing Scheduler** | `native/pacing.cpp` | Implement 120 Hz / 144 Hz dual-clock pacing grid, rate-controlled against host display refresh. | 2–3 |

---

## 5. Acceptance Criteria & Safety Contracts

1. **Physics Fidelity Invariant:**
   `RaceBase__updateRace` must continue executing strictly at $60\text{ Hz}$ ($dt = 1/60\text{ s}$). No vehicle handling, lap timing, tire grip, or AI physics may diverge.
2. **Visual Quality Gate:**
   PSNR between real frame $N$ and interpolated frame $N$ (from $N-1$ and $N+1$) must exceed $32\text{ dB}$ on race scenes.
3. **Menu Isolation:**
   480i interlaced menus remain locked to native 60 Hz fields (deinterlace automatic); high-FPS interpolation activates only during progressive 480p race scenes.
4. **Latency Budget:**
   Mode 2 introduces exactly $+1$ frame of presentation latency ($16.66\text{ ms}$), with total input-to-photon latency remaining under $45\text{ ms}$ at 120 Hz.
