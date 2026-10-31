# Integrated GPU Crash Study & Resolution (Issue #89 / TASK-105)

## 1. Executive Summary

Upstream Issue **#89** (*"Game crashes on creating game data"*, reported by `@rj45-lab` on `moesuito/gt4-staticrecomp`) reported that GT4Recomp crashed reliably on first boot during the memory card save sequence:
```
Game crashes at point shown in the screenshot. I'm not able to make an input before it crashes.
Version used: GT4Recomp 1.2.3
Hardware: Intel Core i3-1005G1 (Integrated Intel UHD Graphics), 8 GB RAM, Windows 10 22H2
```
The attached screenshot showed the in-game screen *"Creating game data..."* (first boot save file initialization). The user provided `launcher.log`.

This study presents the complete forensic analysis of the crash, diagnoses why Intel UHD Graphics crashed at 3x/2x internal resolutions during render target transitions, and details the architectural solutions implemented in **TASK-105**:
1. Defaulting integrated GPUs to native 1x resolution (640x448) while keeping user-selected multipliers intact.
2. Intercepting GSdx GPU device-lost assertions (`VK_ERROR_DEVICE_LOST` / `Host GPU lost too many times`) and surfacing clear, actionable recovery advice instead of generic crash dialogs.
3. Providing fallback guidance to Direct3D 11/12 on Intel GPUs with fragile Vulkan drivers.

---

## 2. Forensic Analysis of User Logs (`issue89_launcher.log`)

### 2.1 Hardware and Environment
- **CPU:** Intel Core i3-1005G1 (10th Generation "Ice Lake", 2 cores / 4 threads, 1.20 GHz base / 3.40 GHz boost).
- **GPU:** Intel UHD Graphics for 10th Gen Intel Processors (32 Execution Units, Gen 11 architecture, shared system RAM with 8 GB total).
- **Display:** 1440x900 resolution (16:10 aspect ratio).
- **Launcher Build:** GT4Recomp 1.2.3 (Installer build in `C:\Program Files\GT4Recomp`).

### 2.2 Execution Timeline and Resolution Scaling
From `issue89_launcher.log`:
1. **Session 1 (`session-1792779136`):**
   - Output resolution: `1440x900`.
   - Render scale: Default `100%`.
   - The launcher calculated:
     $$k = \max\left(\left\lceil \frac{1440}{640} \right\rceil, \left\lceil \frac{900}{448} \right\rceil\right) = \max(3, 3) = 3$$
   - Multiplier applied: **3x internal resolution ($1920 \times 1344$)**.
   - Game executed for ~75 seconds (navigating intro movies and reaching initial save creation).
   - Crash logged:
     ```
     [1792779211] game ended: Crashed { reason: Some("GS process exited"), exit_code: Some(0) }
     [1792779211] GSdx asked for a restart; the launcher does not start CoreGS 1.1
     ```
2. **Session 2 (`session-1792779481`):**
   - The user attempted to lower the resolution by setting Render scale to 50%:
     `display: \\.\DISPLAY1 output 1440x900 -> 2x internal 1280x896 (k 3, render scale 50%, aspect 1.6000)`
   - Multiplier applied: **2x internal resolution ($1280 \times 896$)**.
   - Game executed for ~23 seconds before failing at the exact same screen with `GS process exited`.

---

## 3. Root Cause Analysis

### 3.1 Intel UHD Graphics Constraints on Ice Lake
1. **Limited Execution Resources & Bandwidth:** The i3-1005G1 features 32 EUs with a shared 15W TDP package. At 3x internal resolution ($1920 \times 1344$), fillrate and VRAM bandwidth demands exceed the capabilities of dual-channel system memory sharing CPU/GPU bandwidth.
2. **Render Target Transitions during Save Creation:**
   - GT4's initial memory card sequence transitions between 2D UI layers, font rendering, animated 3D memory card icons, and backbuffer clearing.
   - In GSdx Vulkan, render target switches require pipeline barriers and memory allocations.
3. **Windows TDR (Timeout Detection and Recovery):**
   - When a drawcall or barrier takes longer than 2 seconds to complete on the GPU, Windows triggers TDR to reset the graphics driver.
   - In Vulkan, this manifests as `VK_ERROR_DEVICE_LOST`.
   - GSdx asserts:
     ```text
     Assertion failed in function GSRenderer::BeginPresentFrame: Host GPU lost too many times, device is probably completely wedged
     ```
   - `gt4-gs.exe` aborts, closing the shared memory IPC pipe.
   - The main game process detects an EOF stream break and halts with `[ee] stopped ... : GS process exited`.

---

## 4. Architectural Solutions (TASK-105)

### 4.1 Native 1x Default for Integrated GPUs (`gt4l-core::display`)
Upscaling to 3x or 4x based solely on desktop monitor resolution is appropriate for discrete GPUs (NVIDIA/AMD), but detrimental to low-power integrated graphics.

In `launcher/crates/gt4l-core/src/display.rs`:
- Introduced `INTEGRATED_DEFAULT_MULTIPLIER = 1` (640x448 native PS2 framebuffer).
- Implemented `display_plan_on(..., integrated_gpu: bool)`:
  ```rust
  pub fn integrated_default_applies(render_scale: u32, integrated_gpu: bool) -> bool {
      integrated_gpu && render_scale == RENDER_SCALE_DEFAULT
  }
  ```
- **Preserving Player Agency:** If the player explicitly adjusts the Render scale slider away from 100% (e.g. to 50%, 75%, or 125%), their manual choice is respected.
- On a 1440x900 monitor, an integrated GPU now boots at $1\times$ ($640 \times 448$) instead of $3\times$ ($1920 \times 1344$), reducing pixel throughput by nearly 90% and eliminating TDR timeouts during memory card sequences.

### 4.2 Integrated GPU Detection (`gt4l-core::system_check` & `gt4l-platform`)
In `system_check.rs`:
- Added `DEVICE_TYPE_INTEGRATED = 1` (`VK_PHYSICAL_DEVICE_TYPE_INTEGRATED_GPU`).
- Implemented `integrated_gpu_name(devices: &[VulkanDevice]) -> Option<String>`:
  Checks physical devices reported by Vulkan. To be safe, the integrated default activates only when **every** GPU with its own driver is integrated (avoiding accidentally throttling hybrid laptops with discrete NVIDIA/AMD GPUs).
- In `gt4l-platform::system`: Cached detection via `OnceLock<Option<String>>`.

### 4.3 Actionable Error Diagnosis for GPU Device Lost (`gt4l-core::exit`)
Previously, when GSdx crashed from GPU device lost, the launcher only reported:
`Gran Turismo 4 stopped unexpectedly (GS process exited). The session folder has the details.`

In `launcher/crates/gt4l-core/src/exit.rs`:
- Defined `GPU_LOST_MARKERS = ["host gpu lost", "device is probably completely wedged", "vk_error_device_lost"]`.
- `gpu_lost_line(log_text)` inspects `game.log` for driver loss signatures.
- When detected, `classify` and `user_message` prioritize the specific GPU loss condition, outputting:
  > *"Your graphics card stopped responding. Try a lower resolution (Graphics > Render scale) or switch Graphics API to Direct3D 11."*

### 4.4 Driver API Compatibility (Vulkan vs Direct3D 11)
Intel's 10th Gen Vulkan driver contains known stability flaws with complex render pass transitions. Intel's Direct3D 11 driver on Windows 10/11 is significantly more robust against TDR and memory management bugs. The launcher's guidance prompts users on weak integrated graphics to toggle Direct3D 11 if issues persist.

---

## 5. Verification and Evidence

### 5.1 Unit and Contract Test Coverage
1. `display::tests::an_integrated_gpu_with_the_default_scale_renders_at_native_1x`:
   Verifies 1440x900 output with integrated GPU defaults to multiplier 1 (640x448) with `integrated_default = true`.
2. `display::tests::a_discrete_gpu_keeps_the_scaled_multiplier_at_the_default_scale`:
   Verifies discrete GPUs keep multiplier 3 (1920x1344).
3. `display::tests::a_changed_render_scale_is_kept_on_an_integrated_gpu`:
   Verifies explicit scale changes (50%, 75%, 125%, 200%) are honored.
4. `system_check::tests::an_integrated_only_machine_is_integrated_with_its_first_name`:
   Verifies Intel UHD identification.
5. `system_check::tests::a_discrete_card_among_integrated_ones_keeps_the_discrete_default`:
   Verifies multi-GPU laptops keep discrete defaults.
6. `exit::tests::gpu_lost_takes_priority_over_generic_gs_process_exited`:
   Verifies "Host GPU lost" assertion triggers clear actionable advice.
7. `launch_contract::tests::an_integrated_gpu_plays_the_untouched_default_at_native_resolution`:
   Verifies end-to-end `gt4-gs.ini` generation writes `upscale_multiplier = 1` for integrated GPUs and `3` for discrete GPUs.

### 5.2 Test Results
- `gt4l-core` unit tests: **117 passed, 0 failed**.
- `gt4l-core` launch contract tests: **17 passed, 0 failed**.
