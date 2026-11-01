# CoreGS 2.0 Resumption Roadmap: Native Variable Frame Rate & Robust Frame Skipping (TASK-082)

## 1. Executive Summary & Context

The Gran Turismo 4 static recompilation project operates under a strict, non-negotiable invariant:
$$\text{Game Physics Simulation: } \Delta t = \frac{1}{60}\text{ s} \approx 0.016667\text{ s} \text{ (Strictly Fixed)}$$
Vehicle handling, suspension articulation, tire friction models, AI pathfinding, and timekeeping are locked to 60 VBlanks per second.

**TASK-082** ("Native variable frame rate") was conceived to decouple visual presentation from simulation cadence:
* **Sub-60 FPS (P1 Render Skip):** Allows low-end hardware (e.g. 15 W ultrabooks and handhelds) to drop rendered frames while simulation maintains 100% full game speed without slowdown.
* **Super-60 FPS (P2/P3 High-FPS):** Allows high-refresh displays (120 Hz, 144 Hz) to render auxiliary passes between simulation steps without altering game physics.

### 1.1 The Experimental Test Findings & Owner Decision
On 2026-10-26, experimental testing of render-skip build `rc7-test082` on an Intel Core i5-7200U laptop (Intel HD Graphics 620) revealed a **critical visual defect**:
* Under heavy GPU load, older visual frames flashed repeatedly between advancing frames. Video evidence confirmed that scenes from seconds earlier (e.g. HUD timer `0:00.983`) intermittently flashed on screen while the car was advancing through `0:06.433` and `0:10.016`.
* Disabling host VSync did not cure the problem; it manifested as horizontal tearing bands between mismatched camera perspectives.

**Owner Decision (2026-10-26):**
The owner decided to **defer further frame-skipping implementation on the legacy multi-process GSdx stream architecture** and preserve it for resumption targeting **CoreGS 2.0** (Fase 2). All prototype work is preserved on branch `task/082-render-skip` at commit `15b7bf3` (PR #113 Draft).

This roadmap establishes the root cause diagnosis and formalizes the CoreGS 2.0 native resumption architecture.

---

## 2. Root Cause Analysis: The Double-Buffer Flashing Defect

### 2.1 The Guest Engine Double-Buffering Contract
Analysis of the recompiled Emotion Engine main loop (`func_001016E8` in `unit_0000.cpp`) revealed the internal mechanism governing frame presentation in Gran Turismo 4:

```
+-------------------------------------------------------------------------------+
|                       GT4 EE MAIN LOOP (func_001016E8)                        |
|                                                                               |
|  1. Simulation Step: Updates physics, AI, timers (dt = 1/60 s)               |
|  2. Double-Buffer Toggle (0x001077F8): D_006184C0.cur = 1 - D_006184C0.cur   |
|  3. Render Dispatch: Renders 3D world to buffer D_006184C0.cur                |
|  4. Guest Display Flip (func_00107878):                                       |
|     Writes privileged GS register DISPFB2 specifying display buffer base      |
+-------------------------------------------------------------------------------+
```

### 2.2 The Depletion and Flashing Mechanism
When render-skipping was introduced at the host level:
1. On a skipped frame, the simulation executed normally, but 3D primitive drawing was bypassed to save GPU time.
2. **The Defect:** The guest flip function (`func_00107878`) still unconditionally inverted the buffer index:
   $$\text{cur}_{\text{new}} = 1 - \text{cur}_{\text{old}}$$
   And issued a `DISPFB2` write pointing the Graphic Synthesizer to display the other buffer!
3. Because the skipped frame wrote **no new pixels** to that buffer, the buffer still contained visual data from two or more frames prior.
4. If preceding frames had also been skipped or partially rendered, the buffer held stale frames from seconds earlier.
5. In the legacy separate-process architecture ([Decision 0070](decisions/0070-pcsx2-gs-separate-process.md)), `gt4-gs.exe` processes the IPC command stream asynchronously and displays whichever buffer `DISPFB2` points to.
6. The result on screen was violent alternating flashing between the advancing current frame and stale unrendered frames.

### 2.3 Why the External GSdx Architecture Makes a Clean Fix Prohibitive
Attempting to patch this over the external IPC stream to `gt4-gs.exe` introduces unacceptable synchronization complexity:
* `gt4-gs.exe` is a GPL child process running asynchronously with its own internal swapchain thread.
* Suppressing display flips across the IPC ring introduces ring latency, synchronization race conditions, and readback deadlocks during night flare queries (`download sync`).

---

## 3. The CoreGS 2.0 In-Process Architectural Solution

**CoreGS 2.0** is GT4Recomp's custom MIT hardware Graphic Synthesizer renderer ([Decision 0069](decisions/0069-own-hardware-renderer-clean-room.md), TASK-026 clean-room specification, TASK-027 implementation).

Because CoreGS 2.0 runs **in-process** within the native executable, the host frame scheduler has direct, zero-overhead access to Vulkan swapchain presentation, VRAM page tables, and register states.

```
+-------------------------------------------------------------------------------+
|                        GT4RECOMP NATIVE ENGINE                                |
|                                                                               |
|   +------------------------------------+                                      |
|   |   Emotion Engine (Static AOT)      |                                      |
|   |   Physics fixed at 60 Hz           |                                      |
|   +-----------------+------------------+                                      |
|                     |                                                         |
|                     v                                                         |
|   +------------------------------------+                                      |
|   |      Host Frame Scheduler          |                                      |
|   |  * Intercepts DISPFB2 flips        |                                      |
|   |  * Tracks buffer dirty flags       |                                      |
|   |  * Governs present cadence         |                                      |
|   +-----------------+------------------+                                      |
|                     |                                                         |
|                     v                                                         |
|   +------------------------------------+                                      |
|   |       CoreGS 2.0 (In-Process)      |                                      |
|   |  * Direct Vulkan Swapchain Control |                                      |
|   |  * Controlled vkQueuePresentKHR()  |                                      |
|   +------------------------------------+                                      |
+-------------------------------------------------------------------------------+
```

### 3.1 Direct Swapchain Present Gate
In CoreGS 2.0, presentation is decoupled from guest VBlank registers:
1. **Present Suppression on Skip:** When the frame scheduler decides to skip rendering for frame $N$, `vkQueuePresentKHR` is simply **not invoked**.
2. **Swapchain Hold:** The display hardware continues scanning out the previous, fully rendered swapchain image (`present hold`).
3. **Flashing Eliminated:** Stale VRAM buffer pages are never exposed to the host display, completely preventing visual flashing.

### 3.2 DISPFB2 Interception and Page Pinning
1. The host runtime intercepts privileged `DISPFB2` writes dispatched by `func_00107878`.
2. On skipped frames, the active display base address in CoreGS 2.0 remains pinned to the buffer of the last valid rendered frame:
   $$\text{ActiveDisplayBase} = \text{RenderedBuffer}_{\text{last}}$$
3. Only when a frame is completely rendered does the display base update to the new buffer.

### 3.3 Flare Readback Decoupling
As established in [`docs/FRAME-COST-STUDY.md`](FRAME-COST-STUDY.md) (Section 4.3):
* During night races, flare intensity queries (`download sync`) stall the EE for up to $4.2\text{ ms}$.
* In CoreGS 2.0, skipped frames bypass the flare readback and return the cached intensity from frame $N-1$.
* This removes the dominant readback wait and allows render-skip frames to complete in $< 1.5\text{ ms}$ on the CPU.

---

## 4. High-FPS Decoupled Rendering (120 Hz / 144 Hz)

For presentation rates above 60 Hz:
1. The simulation advances at its natural $60\text{ Hz}$ ticks ($dt = 1/60\text{ s}$).
2. Between ticks $N$ and $N+1$, the engine dispatches an **auxiliary render pass** (`AUX_FRAME`):
   * Time parameter $t \in (0, 1)$ interpolates camera view-projection matrices and dynamic vehicle transforms.
   * Static track geometry and lighting are re-rasterized directly into an auxiliary swapchain buffer.
   * Guest readbacks, interrupts, and game state mutations are strictly bypassed.
3. CoreGS 2.0 submits the auxiliary frame to the swapchain, achieving true, crisp 120 Hz or 144 Hz motion fluidity without altering vehicle dynamics.

---

## 5. Resumption Acceptance Criteria & Validation Matrix

When TASK-082 resumes under CoreGS 2.0 (Phase 2), the implementation must pass the following strict quality gates:

| Verification Target | Requirement | Measurement & Validation Method |
|---|---|---|
| **Zero Flashing / Stale Frames** | Monotonically advancing visual timestamps | Automated chronometer audit across 1,000 frames of replay video. Zero inversions. |
| **Simulation Invariant** | Exactly 100% game speed down to 20 FPS | $\Delta \text{clock\_f898} / \Delta t_{\text{wall}} = 3,000\text{ units/sec} \pm 0.1\%$ |
| **Present Pacing Cadence** | Flat frametime histogram | PresentMon / RTSS frametime variance $< 1.0\text{ ms}$ on target refresh rate |
| **Audio Continuity** | Zero pops, clicks, or underruns | SPU2 buffer underrun counter $= 0$ in F8 performance probe |
| **Physics Determinism** | Replay and ghost byte identity | Replay inputs match bit-identically across skipped and non-skipped runs |

---

## 6. Project Planning & Roadmap Position

* **Phase:** Phase 2 (Release 2.0.0, CoreGS 2.0).
* **Reference Assets:** Branch `task/082-render-skip` at commit `15b7bf3` and PR #113 (Draft).
* **Pre-conditions:** CoreGS 2.0 reaches parity with GSdx in rendering accuracy and performance.
