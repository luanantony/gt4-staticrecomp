# PCSX2 GS as the release renderer, in a separate process: plan

Date: 2026-10-15. Status: **Approved by the owner on 2026-10-15** (all recommendations; answers in section G).
Decision record: [0070](decisions/0070-pcsx2-gs-separate-process.md). Planning only: no repository file
edited, nothing built, no game launched in the planning step.
Author: planning agent (PCSX2-tainted; must never work on CoreGS 2.0 / TASK-027).

Labels: **[C]** Confirmed (read in code, docs or logs), **[HC]** High confidence, **[H]** Hypothesis,
**[U]** Unknown.

Sources read:
- Repo `C:\Antigravity\gt4-staticrecomp`, `main` at `6eafc7e`: CLAUDE.md, README.md, TASKS.md,
  docs/RELEASE-1.0.0-PLAN.md, docs/GS-RENDERER-OPTIONS.md, docs/decisions/0066-0069, docs/ARCHITECTURE.md,
  docs/GS-ASYNC-DESIGN.md, docs/MTVU-PLAN.md (sections 10-12).
- Branches: `origin/task/019-launcher` docs/launcher/LAUNCHER-PLAN.md section 15; `origin/task/031-high-fps`
  docs/HIGH-FPS-STUDY.md and docs/NATIVE-RENDERER-STUDY.md; `origin/task/012-parallel-gs` (`54a937e`)
  docs/GSDX-STUDY.md, native/parallel_gs_backend.{hpp,cpp}; `origin/task/030-night-readback` docs/BUILD.md.
- Donor runtime `private/references/ps2-web-recomp/runtime/src/gs/gs.cpp` (entry points only).
- PCSX2: pinned copy `private/tooling/pcsx2-slice101-source` at `aa7ab4306e269075784c7ac3eb4b45e6e6c53445`,
  and a fresh read-only shallow clone `private/slice189-pcsx2-ref` at
  **`685637d30b5eaa8e5d7a587673a17577a92dcb88`** (master, 2026-10-14, one commit after tag `v2.9.127` =
  `e35cb2c8`; latest stable tag `v2.8.2` = `fd9d310c`).
- Owner F8 logs `private/manual-play/session-*/probe.log`; capture `private/slice175-capture/menu-0-1100.gscap`.

---

## 0. Summary

- The PCSX2 GS (GSdx lineage) becomes the release renderer in its own GPL process, **`gt4-gs.exe`**, built
  from a public fork (proposed: **`github.com/moesuito/gt4recomp-gs`**) and downloaded by the launcher.
- Key finding [C]: PCSX2 already has the exact seam we need. `GSDumpReplayer` replaces the EE CPU with a
  packet feeder that pushes GS dump records (Transfer path 1/2/3, VSync, ReadFIFO2, Registers) through the
  normal GIF unit and MTGS (`pcsx2/GSDumpReplayer.cpp:249-410`), and `pcsx2-gsrunner` boots it headless or
  windowed (`pcsx2-gsrunner/Main.cpp`). A live "stream replayer" that reads the same records from shared
  memory instead of a file is a small fork patch, not an extraction of 45-55k lines.
- Our `.gscap` records ('G' path data, 'P' privileged write, 'R' FIFO read, 'V' vsync;
  `native/parallel_gs_backend.cpp` on task/012) are the same four record kinds as a PCSX2 `.gs` dump
  (`pcsx2/GS/GSDump.h:9-26`, `GSLzma.h:37-51`). The live protocol is defined as that record stream: generic
  GS hardware data, which is the basis of the arm's-length position.
- SIGNAL/FINISH/LABEL, CSR and IMR stay on the game side (as in PCSX2, where `Gif_HandlerAD` runs on the EE
  thread, and as in our slice172 MTVU code). The stream is one-way except FIFO readbacks.
- Window: `gt4-gs.exe` creates a **child window inside our game window** (Chromium GPU-process model);
  input, focus, fullscreen and pacing stay in the game process. Fallback: GS-owned top-level window with input
  forwarding.
- paraLLEl-GS: **drop** from the release (do not merge its LGPL code); keep CoreGS 1.1 (MIT, in process) as
  the always-available fallback; salvage the MIT renderer seam and capture code from task/012.
- 13 tasks to release (TASK-032..044), about **30-45 agent sessions**; TASK-045 (high frame rate on the
  PCSX2 GS) is a later phase of about 10-14 sessions.

---

## A. Current architecture

### A.1 How the game process produces GS work today

| Item | Where / how | Label |
|---|---|---|
| PATH1 (XGKICK) | VU1 AOT program on the MTVU worker emits GIF bytes via `gif::transfer` -> `worker_gif`; journal keeps order with EE GS work (MTVU-PLAN 11.2) | [C] |
| PATH2 (VIF1 DIRECT) | worker output; signals scanned EE-side in `shadow_direct` (MTVU-PLAN 11.2, 12.2) | [C] |
| PATH3 (GIF DMA) | EE, `gt4::mtvu::gif_transfer`, appended to the journal or rendered directly | [C] |
| GS front end | donor `gs/gs.cpp`: `write` (`:525`), `write_packed` (`:639`), `write_image` (`:678`), `read_image` (`:682`), batches to the Vulkan compute raster (`native/native_vulkan.cpp`, `gpu_*.inc`) | [C] |
| Privileged registers | `write_privileged`/`read_privileged` (`gs.cpp:755-807`): PMODE, SMODE1/2, DISPFB/DISPLAY 1/2, BGCOLOR, CSR, IMR, SIGLBLID kept in donor globals (`gs.cpp:36-38`); about 5 privileged writes per frame, each a blocking MTVU sync today (`gs_privileged`, MTVU-PLAN 11.6) | [C] |
| SIGNAL / FINISH / LABEL | applied on the EE at acceptance for PATH2/PATH3 (slice172, `apply_accepted_signals`); PATH1 has 0 such writes in all measured scenes (MTVU-PLAN 10) | [C] |
| VSync / field | `gs::vblank` toggles `g_odd_field` (`gs.cpp:809-811`); CSR FIELD is reported by `read_privileged(0x1000)` (`:757`); pacing in `native/window.cpp` + `pacing.cpp` (single clock, audio rate control) | [C] |
| Interlace | menus 480i, races held at 480p by `--force-480p` (guest memory poke at `[0x622F4C]+0x38CB0+0x16B0`, TASK-009); renderer-independent | [C] |
| Readbacks (local->host) | TRXDIR=1 then `read_image`; MTVU `sync("download")`. Night race (owner session `session-20261015-163129-751343`): `gs_readback/frame=1.00 bytes/frame=8192 forced_flushes/frame=1.00 blocking_waits/frame=1.00`, `gs_fence_wait=4.317` ms, 59.07 VBlanks/s. Other scenes 0-0.88 readbacks/frame, 245-7,397 bytes | [C] |
| Stream size | owner F8 `mapped_write_bytes/frame` 2.28-2.67 MB in races (max interval 7.2 MB, loading); raw GIF bytes in the menu capture 514,627,776 / 1,101 frames = 0.47 MB/frame (`replay-menu.log`) | [C] |
| Save state | donor `gs.cpp:1066-1099` (`render_state`), synchronous; not user-facing in release 1.0.0 | [C] / [HC] |

### A.2 Existing seams

- **Donor GPU backend** (main): `scripts/prepare_external_vulkan.py` splices `native/gpu_*.inc` into the
  donor `gpu.cpp`; 3-slot async submit ring (GS-ASYNC-DESIGN). [C]
- **Renderer dispatch** exists only on branches [C]: `GT4_RENDERER` = donor | parallel
  (`native/parallel_gs_frontend.inc` on task/012, spliced by `prepare_external_vulkan.py --parallel-hooks`)
  and `GT4_RENDERER=hw` on task/027. Main has no `GT4_RENDERER` (grep of `native/`, `scripts/` on main).
  Hooks on task/012: `parallel_gif_transfer(path, data, qwc)`, `parallel_read_image`,
  `parallel_mirror_privileged`, `parallel_vblank`, `parallel_present`, `parallel_attach_window`,
  `parallel_resize_window`, `note_crt_mode`, `parallel_reset*`, signals via `SignalHandler` (GSDX-STUDY 2).
- **paraLLEl backend** `native/parallel_gs_backend.{hpp,cpp}` (task/012, 479 lines): Vulkan/Granite-free
  header; capture class `StreamCapture` (`:111-200`).
- **`GT4_PGS_CAPTURE` format** [C] (`parallel_gs_backend.cpp`, comment above `StreamCapture`): little-endian
  records `'G' u8 path, u32 bytes, data`; `'P' u32 guest address, u64 value`; `'R' u32 qwords`;
  `'V' u64 frame`. `GT4_PGS_CAPTURE_FRAMES=first:last`. Gaps for PCSX2: no initial GS state (fine from
  boot: zero VRAM), no field bit in 'V' (derivable: toggles per vsync), no FIFO data (not needed to replay).
- **Launcher pins**: `scripts/external_runtime_probe.py` verifies every file against `docs/inputs/*.json`
  and scrubs `GT4_*`/`PS2_*` (so `GT4_PGS_*` cannot pass through `play_performance.py`; HIGH-FPS 9.6). [C]

---

## B. PCSX2 GS extraction

### B.1 What the GS needs from the rest of PCSX2 (at `685637d`)

| Dependency | Use | Label |
|---|---|---|
| `GSopen(GSOptions, renderer, basemem, vsync, throttle)` and friends (`pcsx2/GS/GS.h:57-110`) | lifecycle, transfers, `GSvsync(field, regs_written)`, `GSInitAndReadFIFO`, `GSReadLocalMemoryUnsync`, `GSfreeze`, `GSSaveSnapshotToMemory` | [C] |
| `Host::` (`GS.h:116-137` plus about 60 functions stubbed in `pcsx2-gsrunner/Main.cpp:131-405`) | render window, fullscreen, OSD, settings layers, progress, errors | [C] |
| MTGS (`pcsx2/MTGS.cpp`) | GS thread and ring; commands GIFPath1-3, GSPacket, VSync (copies privileged regs + CSR/IMR/SIGLBLID, `:239-281`), Freeze, Reset, SoftReset, InitAndReadFIFO (`:283-298`, blocks with `WaitGS`) | [C] |
| GIF unit (`Gif_Unit.h`) | path arbitration, completed packets to MTGS; SIGNAL/FINISH/LABEL handled EE-side (`Gif_HandlerAD`, `GS_SIGNAL`, `GS_FINISH`) | [C] |
| VMManager | boot, settings, GameDB lookup by serial (`VMManager.cpp:745-750`), shader cache folders, CPU providers (`UpdateCPUImplementations`, `:2740-2760`) | [C] |
| GameDB `bin/resources/GameIndex.yaml` | per-serial GS fixes (D.4) | [C] |
| ImGui OSD, fonts | `ImGuiManager`, overlays (FPS, GS stats, messages) | [C] |
| Shader cache | runtime GLSL via shaderc (Vulkan), HLSL for D3D; cache in `[Folders] Cache` (`Pcsx2Config.cpp:2311`) | [C] |
| Texture replacements | `GSTextureReplacements.cpp`: `<Textures>/<serial>/{replacements,dumps}` or `<serial>.zip` (`:41-42, 271, 416`) | [C] |
| Recompilers / RWX memory | `VMManager::InitializeCPUProviders` reserves EE/IOP/VU recompilers (`:2700-2707`) and `Memory.cpp:98-112` maps a code area with `PageAccess_Any`, also in dump mode | [C] |
| SW scanline JIT | used by the HW renderer only through `SwPrimRender` (CPU sprite render, CPU CLUT render; `GSRendererHW.cpp:3095, 3131-3137`); a C fallback `CDrawScanline` exists for debugging (`GSDrawScanline.h:33,59`) | [C] |

### B.2 GSDumpReplayer and GSRunner (the template)

- `GSDumpReplayer` [C]: on load, copies the 8 KiB privileged block to `PS2MEM_GS` and loads the GS freeze
  state through `MTGS::Freeze` (`:233-247`); per packet: Transfer -> `Gif_AddCompletedGSPacket` (`:249-262`),
  VSync -> `MTGS::PostVsyncStart` + frame limiter, ReadFIFO2 -> `MTGS::InitAndReadFIFO`, Registers ->
  memcpy into `PS2MEM_GS` (`:370-410`). It runs as an `R5900cpu` (`GSDumpReplayerCpu`) selected by
  `UpdateCPUImplementations`; VU0/VU1 become interpreters and IOP the interpreter, never executed.
- `pcsx2-gsrunner` [C]: links the whole `PCSX2` core library (`pcsx2-gsrunner/CMakeLists.txt`), stubs
  `Host::`, forces no input, no audio, no memory cards (`SettingsOverride`, `:798-840`), and has flags
  `-dumpdir`, `-renderer`, `-upscale`, `-ini`, `-window`, `-surfaceless`, `-perf`, `-loop`. With `-dumpdir`
  it writes frame PNGs (`filename_frameN.png`).
- Build [C]: `ENABLE_GSRUNNER` option (`cmake/BuildParameters.cmake:11`); Windows CI builds CMake with MSVC
  and with clang-cl (`.github/workflows/windows_build_matrix.yml:50-95`); dependencies come from
  `.github/workflows/scripts/windows/build-dependencies.bat` (SDL3, shaderc, zstd, lz4, libpng, jpeg,
  webp, freetype, plutosvg, ryml, ffmpeg headers, Qt).

### B.3 Minimal fork surface for a live `gt4-gs.exe`

Stage 1 (MVP, small diff; all GPL, all in the fork):
1. `pcsx2-gt4gs/` (new frontend, from `pcsx2-gsrunner/Main.cpp`): command line (`--ipc <name>`,
   `--parent <pid>`, `--config <ini>`, `--replay-gscap <file>`, `--write-gsdump <file>`), Host stubs,
   child-window creation, IPC client, settings override (no input, no SPU2 output, no cards).
2. `pcsx2/GSStreamReplayer.{h,cpp}`: an `R5900cpu` like `GSDumpReplayerCpu` whose `Execute` loop pops
   protocol records from the shared ring (or from a `.gscap` file) and does what the dump replayer does
   per record; ReadFIFO calls `MTGS::InitAndReadFIFO` then copies into the shared response area.
3. VMManager: about 6-10 one-line hooks where `GSDumpReplayer::IsReplayingDump()` is tested (boot without
   BIOS, serial/CRC for GameDB, CPU provider selection; `VMManager.cpp:670-1957`). Serial comes from the
   handshake (`SCUS-97328`).
4. `gt4gs/` docs, NOTICE, build scripts, CI. The MIT protocol header is vendored by hash.

Stage 2 (hardening before release, TASK-042; **done 2026-10-16**, `gt4gs-v0.4.0`, slice197: [GS-LIVE](GS-LIVE.md) section 9,
decision 0070 follow-up): a `GT4GS_ONLY` build that does not compile or reserve the
EE/IOP/VU recompilers, never maps RWX memory, and forces `CDrawScanline` (or refuses CPU sprite/CLUT render
settings), so `gt4-gs.exe` contains no JIT code at all. Effort [H] 3-5 sessions; the diff grows to
CMake source lists plus `#if` around `InitializeCPUProviders`, `Memory.cpp` code mapping and the rec cpu
symbols. Optional later: feed `GSgifTransfer*` on the GS thread directly (skip the GIF-unit and MTGS
copies).

Rejected for now: extracting a GS-only library (GSDX-STUDY estimate 4-6 engineer-months). Stage 1/2 reach
the same result with a diff that can be rebased on each PCSX2 release.

### B.4 Toolchain and build time

- CMake + Ninja, clang-cl (matches our toolchain) or MSVC; `-DENABLE_QT_UI=OFF -DENABLE_GSRUNNER=ON`;
  deps built once by `build-dependencies.bat` (can be trimmed to skip Qt) and cached in CI. [C] recipe, [H]
  that Qt-free deps work without patching.
- Build time [H]: deps 40-90 min cold (cached afterwards), PCSX2 core + frontend 10-20 min on the owner's
  8-core machine, 25-40 min on a GitHub Windows runner.
- Upstream base: PoC on `685637d`; release pinned to a PCSX2 tag (stable `v2.8.x` or a `v2.9.x` nightly
  tag); the patch set is rebased per update with the PoC comparison re-run.

### B.5 Renderers to ship (Windows)

- **Vulkan default** (project direction; same driver path we already validate). D3D11 and D3D12 come in
  the same binary at no extra cost: expose them as options (D3D12 is a useful workaround on driver bugs).
  OpenGL hidden. Software renderer **refused** (it is the SW JIT; GT4 does not need it). [HC]
- Upscale: `GSGetMaxUpscaleMultiplier = max_texture_size / 1280` (`GS.cpp:675-679`): 12x on a 16384
  texture limit [C]; expose 1x-8x, plus up to the device limit behind `ExtendedUpscalingMultipliers`.

---

## C. IPC design

### C.1 Shared memory layout (one mapping per session, name `Local\gt4gs-<pid>-<random128>`)

| Region | Size [H] | Use |
|---|---|---|
| Control block | 4 KiB | magic, protocol major/minor, struct sizes, both PIDs, state flags (ready, sleeping, fatal), ring positions (`std::atomic<u64>`, lock-free on x64), frames queued, readback seq/req/done, heartbeat counters |
| Command ring game->GS | 32 MiB | SPSC byte ring of 16-byte-aligned records (same discipline as `native/mtvu_ring.hpp`: 16-byte header, wrap filler, largest record capacity/2, split with a MORE flag) |
| Response area | 4 MiB + 64 KiB | FIFO readback data (whole VRAM worst case), snapshot pixels (or a second mapping sized on demand) |
| Event ring GS->game | 256 KiB | back channel: READY, STATS, ERROR, WINDOW, SNAPSHOT_DONE, EXITING |

Wake-ups: `WaitOnAddress` is per process, so cross-process waits use named auto-reset events
(`data`, `space`, `response`, `event`) with spin-then-wait (20-50 us spin) and a "sleeping" flag so the
producer only calls `SetEvent` when the consumer sleeps (MTGS `SetEvent` tally pattern). Every wait also
waits on the peer process handle (no hang on peer death).

### C.2 Records (protocol v1, MIT header `native/gs_stream/gs_stream_protocol.h`)

Game -> GS (in stream order):
- `GIF {path 1..3, flags, bytes}` + qword data. One record per accepted GIF packet chunk (= capture 'G').
- `PRIV {guest address 0x12000000-0x12001FFF, u64 value}` (= 'P'). The GS side writes it into its
  8 KiB privileged block; CSR writes with reset/flush bits map to GS reset/soft reset.
- `VSYNC {frame u64, field u8, flags}` (= 'V' plus field). GS side: `PostVsyncStart`, which snapshots
  the privileged block and presents.
- `READ_FIFO {qwords, request id}` (= 'R'); GS answers in the response area and bumps `readback_done`.
- `RESET {hard|soft mask}`, `STATE_SAVE/LOAD` (reserved, not in 1.0.0), `CONTROL {resize, reload config,
  snapshot(id, w, h), screenshot to file, osd message, toggle osd, shutdown}`,
  `AUX_FRAME_BEGIN/END` (reserved for TASK-045; a frame that is not on the guest timeline).
- No game code, no guest RAM, no EE/VU state: only GS hardware registers and GIF packets.

GS -> game: `READY {fork version, upstream commit, renderer, adapter, max upscale}`, `STATS` (per F8
interval, C.6), `ERROR {code, text}`, `SNAPSHOT_DONE {id}`, `WINDOW {lost|restored}`, `EXITING`.

`.gscap` becomes "the protocol written to a file" (version header added in v2 of the capture format), so
every live session can be recorded and replayed offline (`gt4-gs.exe --replay-gscap`).

### C.3 Game-side backend (MIT, main repo)

- `GT4_RENDERER=pcsx2` (launcher value `pcsx2-gs`): the seam from task/012 is ported to main (MIT parts
  only) and generalized: GIF per path -> `GIF` record; privileged write -> keep the donor value (the EE reads
  CSR/IMR/SIGLBLID locally) and post `PRIV`; vblank -> `VSYNC`; `read_image` -> `READ_FIFO` and wait;
  present -> nothing (the GS presents); attach/resize -> handshake/`CONTROL`.
- No Vulkan device in the game process in this mode; the donor GS front end is not executed (saves the
  2.5-5.2 ms/frame PATH1 GS front-end share measured in MTVU-PLAN 10 [C], minus the copy cost).
- MTVU: the journal drain becomes "append to the IPC ring" (one memcpy); privileged writes no longer need a
  blocking sync when they only go into the stream (CSR reads stay local) [H, verify per register].
- SIGNAL/FINISH/LABEL: applied on the EE at acceptance for all paths (slice172 scanner; PATH1 invariant
  kept). The GS side also sees them in the stream and updates its own copy (harmless). [HC]

### C.4 VSync, pacing, latency, back-pressure

- The game process stays the clock owner (frame_clock, audio rate control, F7/F8). It posts `VSYNC` when
  the pacer releases the frame.
- The GS presents when it processes `VSYNC`. Present mode maps from `GT4_PRESENT_MODE`: fifo ->
  `VsyncEnable=1`, mailbox -> vsync with mailbox allowed, immediate -> vsync off. `allow_present_throttle`
  off (the game paces).
- Back-pressure: at most N=2 frames in flight (configurable 1-3, like PCSX2 `VsyncQueueSize`); the game
  blocks in the `VSYNC` post when the GS is N frames behind, or when the ring is full. Added latency versus
  today: 0-1 frame at N=2, about 0 at N=1 [H]; measured in TASK-041.

### C.5 Process lifecycle

1. The game (`gt4vulkan.exe`, later the release game exe) creates the mapping and events, spawns
   `gt4-gs.exe --ipc <name> --parent <pid> --config <ini>` with `CREATE_SUSPENDED`, assigns it to a Job
   Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` (nested under the launcher's job, Windows 8+), resumes.
2. Handshake: GS validates magic, protocol major (must match), minor (may be lower), struct sizes, game
   serial, then creates its device and child window and posts `READY`. Timeout 20 s (first-run shader cache
   may be slow), then a clear error.
3. Pin: the game refuses a `gt4-gs.exe` whose SHA-256 is not in the pinned manifest (dev launcher pins in
   `docs/inputs/`, launcher pins in its manifest).
4. Crash: any wait sees the GS process handle signalled -> the game flushes the memory card (W4 flush on
   exit), shows "GS renderer stopped (code X); restart with CoreGS 1.1?" and exits cleanly; the launcher can
   relaunch with the fallback. Live restart of the GS mid-game is not planned (the GS state lives only in
   that process).
5. Shutdown: `CONTROL shutdown`, wait 3 s, then `TerminateProcess` via the job.

### C.6 Window ownership (recommendation)

| Option | How | Pros | Cons |
|---|---|---|---|
| **1. GS child window (recommended)** | `gt4-gs.exe` creates a `WS_CHILD` window parented to our top-level HWND (cross-process parenting, the Chromium GPU-process pattern); its swapchain is on its own HWND | input, focus, title, Alt+Enter/F11 borderless, DPI, F7/F8 stay in `window.cpp` unchanged; Vulkan/DXGI surface on a same-process HWND (no cross-process swapchain question); RTSS hooks the GS swapchain | parent/child threads get their input queues attached: a hung GS window thread can stall our window (mitigate: GS window on its own pump thread that never blocks; child returns `HTTRANSPARENT`/`MA_NOACTIVATE`, never takes focus); exclusive fullscreen not available (we only use borderless) [HC] |
| 2. GS owns the top-level window | GSRunner-style window; game window hidden; keyboard forwarded over the event ring; XInput polled in the game process (works without focus) | simplest for PCSX2, exclusive fullscreen possible | rewrite of our window/hotkey/focus handling, title/icon/fullscreen move to GPL code, keyboard latency of one hop |
| 3. Shared texture | GS renders to an exported image (Vulkan external memory / DXGI NT handle) and our Vulkan presenter shows it | everything stays in our process | larger fork diff in `GSDevice*` present paths, cross-API interop for D3D11/12, extra copy; not needed now |
| Swapchain on our HWND from the GS process | Vulkan/DXGI surface on a foreign-process HWND | no child window | driver/DXGI behaviour [U]; not relied on |

Decision proposal: option 1, with option 2 as the fallback if the spike in TASK-038 shows focus or hang
problems; option 3 only if a later feature needs the image in our process.

### C.7 Expected overhead

- Copies [H]: race 2.3-2.7 MB/frame (owner F8 mapped writes; raw GIF likely lower, menus 0.47 MB/frame [C]).
  Game side: one memcpy into the ring, about 0.2-0.3 ms/frame at about 10 GB/s. GS side: dump-replayer path
  copies twice more (GIF path buffer, MTGS ring), about 0.5-1 ms on the GS process CPU thread; Stage 2 can
  remove one copy.
- Readback [H]: night race 1 x 8 KiB/frame: drain ring + PCSX2 HW download (GPU sync on the GS thread) +
  two event hops; estimate 0.5-3 ms of EE wait, versus `gs_fence_wait` 4.3 ms today. Knobs:
  `HWDownloadMode` (Enabled, EnabledForceFull, NoReadbacks, Unsynchronized, Disabled; `Config.h:399-406`),
  `HWSpinCPUForReadbacks`, `HWSpinGPUForReadbacks`.
- GS process CPU and GPU cost: [U] for GT4 on this machine; measured first offline (TASK-034 `-perf`),
  then live (TASK-041).
- F8 additions (game): `gs_ipc_write`, `gs_ipc_backpressure`, `gs_ipc_vsync_post`, `gs_ipc_readback_wait`,
  bytes/frame, records/frame, readbacks/frame. From the GS `STATS` record: GS thread busy ms/frame, GPU
  ms/frame (`GetAndResetAccumulatedGPUTime`, `GSDevice.h:1636`), draws, texture uploads, readbacks, present
  wait, ring occupancy. Printed as a `[gs-ipc]` section after the legacy lines (like `[mtvu]`).

---

## D. Settings

### D.1 Table-driven settings

One MIT table in the main repo (proposed `docs/launcher/gs-settings.toml`, consumed by `gt4l-core` and by a
Python generator for the dev launcher). Columns: `id`, label, page (Graphics/Display/Advanced), PCSX2
section/key, type, allowed values, GT4 default, visibility (basic/advanced/hidden), notes. Option names are
interface facts, not PCSX2 code. Proposed initial rows (keys from `Pcsx2Config::GSOptions::LoadSave`):

| Launcher setting | PCSX2 key `[EmuCore/GS]` | GT4 default | Page |
|---|---|---|---|
| Renderer API | `Renderer` | Vulkan (D3D12, D3D11 options) | Graphics |
| Internal resolution | `upscale_multiplier` | 3x (1x-8x; more with `ExtendedUpscalingMultipliers`) | Graphics |
| Texture filtering | `filter` | Bilinear (PS2) | Graphics |
| Trilinear | `TriFilter` | Automatic | Advanced |
| Anisotropic | `MaxAnisotropy` | Off | Graphics |
| Blending accuracy | `accurate_blending_unit` | High (GameDB recommends High) | Advanced |
| Dithering | `Dithering` | Unscaled | Graphics |
| Mipmapping | `HWMipmap` | On | Advanced |
| Texture preloading | `texture_preloading` | Full | Advanced |
| Deinterlace | `deinterlace_mode` | Automatic | Display |
| Aspect ratio | `AspectRatio` | 16:9 (game boots 16:9) | Display |
| FMV aspect switch | `FMVAspectRatioSwitch` | Off | Display |
| Anti-blur / offsets / overscan | `PCRTCAntiBlur`, `PCRTCOffsets`, `PCRTCOverscan` | On / Off / Off | Display |
| Integer scaling, bilinear present | `IntegerScaling`, `linear_present_mode` | Off, Smooth | Display |
| FXAA, CAS, CAS sharpness | `FXAA`, `CASMode`, `CASSharpness` | Off, Off, 50 | Graphics |
| TV shader, Shade boost (4 values) | `TVShader`, `ShadeBoost*` | None, Off | Advanced |
| Readback mode | `HWDownloadMode` | Enabled (accurate) | Advanced |
| Present mode, vsync | `VsyncEnable`, `DisableMailboxPresentation` | from launcher vsync setting | Display |
| Texture replacement load / async / precache | `LoadTextureReplacements`, `LoadTextureReplacementsAsync`, `PrecacheTextureReplacements` | Off / On / Off | Graphics |
| Texture dump (+mipmaps, FMV) | `DumpReplaceableTextures`, `DumpReplaceableMipmaps`, `DumpTexturesWithFMVActive` | Off | Advanced |
| OSD | `OsdShowFPS`, `OsdShowGPU`, `OsdShowResolution`, `OsdMessagesPos`, `OsdScale` | messages only | Advanced |
| Manual hardware fixes | `ManualUserHacks` + `UserHacks_*` | Off (GameDB fixes apply) | hidden |
| Adapter | `Adapter` | default | Advanced |
| Shader cache | `DisableShaderCache` | Off | hidden |

### D.2 Config file

`gt4-gs.ini` in PCSX2 INI syntax (the format PCSX2's `SettingsInterface` already loads; GSRunner `-ini`):
`[EmuCore/GS]` keys from the table, `[EmuCore] EnableGameFixes=true`, `[Folders] Cache, Textures,
Snapshots` pointing to the user data folder. Written atomically by the launcher (or the dev script),
passed with `--config`. `gt4-gs.exe` rejects keys not in its allow-list, so the table is the single
source. A protocol `CONTROL reload_config` lets a later in-game menu apply changes live.

### D.3 Texture replacement and dumping

Layout (PCSX2 contract, [C] `GSTextureReplacements.cpp:41-42,271,416`, verified in TASK-050 / `docs/GS-TEXTURES.md`):
`%LOCALAPPDATA%\GT4Recomp\textures\SCUS-97328\...` (flattened directly under serial in gt4-gs 1.2.1+; older gt4-gs
read `...\replacements\...`), and `...\dumps\...`. **Correction:** `.zip` packs are **not** supported natively by
PCSX2 v2.8.2 / gt4-gs; the launcher extracts packs to disk. Names are 64-bit hashes of GS texture data and optional
CLUT; `-mipN` files are ignored by the base scanner. Duplicates across folders resolve by enumeration order (first
seen wins). PNG must be 8-bit RGB/RGBA; DDS BC7 with mips is recommended for large packs.

### D.4 GT4 defaults and GameDB

`SCUS-97328` (`GameIndex.yaml`, identical at `aa7ab43` and `685637d`) [C]:
- `gsHWFixes: *gran-turismo-4-gs-fixes` = GT3 base (`halfPixelOffset: 5`, `autoFlush: 1`,
  `getSkipCount: GSC_PolyphonyDigitalGames`) + prologue (`recommendedBlendingLevel: 3`) + override
  `halfPixelOffset: 4` (Native). All are GS-state decisions on the stream: **they apply** to our stream [HC].
  `GSC_PolyphonyDigitalGames` (`GSHwHack.cpp:644-730`) replaces the RGB and alpha channel-shuffle sequences
  by one draw.
- `clampModes: vuClampMode: 2`: VU float clamping in PCSX2's VU emulation; **does not apply** (our VU1/VU0
  run as our AOT code). `memcardFilters`: does not apply.
- Correction to GSDX-STUDY section 4: SCUS-97328 has **no** `roundModes` entry in either commit [C].
- Applied automatically when `ManualUserHacks=false` (`GameDatabase.cpp:708-713`) and the serial is known:
  the handshake supplies `SCUS-97328`. No GameDB fix for GT4 reads EE state [HC]; the review is part of
  TASK-034.

---

## E. Legal and compliance (not legal advice)

### E.1 GPL obligations for the fork releases

- Public fork with full history; every release tag builds from that tag only (CI workflow in the fork).
- Each release asset ships `COPYING.GPLv3`, a NOTICE (PCSX2 copyright, "based on PCSX2", our changes and
  dates, GPLv3 section 5), third-party licenses of bundled DLLs (SDL3, shaderc, zstd, lz4, libpng, freetype,
  plutosvg, rapidyaml, etc.), and a link to the exact source tag plus a source archive attached to the same
  release (GPLv3 6(d): source from the same place).
- Build instructions in the fork (`gt4gs/BUILD.md`) reproduce the CI recipe; deps pinned by script hash;
  `/Brepro` and fixed paths aim at bit-identical rebuilds [H]; SHA-256 and a build provenance attestation
  published per release. "Installation Information" (GPLv3 6) concerns consumer devices; not applicable to
  a PC download [HC].
- Name: not "PCSX2"; credit PCSX2 prominently. No game code, data or keys in the fork.

### E.2 Arm's-length position and honest risk

Position: (1) the game executable contains no GPL code and loads no GPL DLL; (2) `gt4-gs.exe` contains no
game code; (3) they are distributed separately (installer vs fork releases); (4) they communicate only a
GS hardware register and GIF packet stream, the same data any GS emulator consumes (PCSX2 `.gs` dumps,
our `.gscap`), through a public MIT protocol; (5) a MIT consumer (stream recorder/null sink) and the
offline replay prove the protocol is not PCSX2-specific. Risk: the FSF view is that programs exchanging
complex internal data through shared memory, designed to work only together, can form one combined work;
a court could weigh the GT4-only purpose and tight coupling. Our counter is that the data are
hardware-defined and the interface generic. Residual risk remains; the owner accepts it as with 0067.
Keep GT4-specific logic out of the protocol (generic flags only), and never copy PCSX2 code into MIT files.

### E.3 Clean-room isolation for CoreGS 2.0 (proposed CLAUDE.md wording)

> **Clean room (TASK-027).** Agents that read PCSX2 sources, the `gt4recomp-gs` fork,
> `private/tooling/pcsx2-*` or `private/slice189-pcsx2-ref` are *PCSX2-tainted*. Tainted agents never work on
> TASK-027, never edit `native/hw_gs/` or `task/027-*` branches, and never answer TASK-027 questions except
> through the TASK-026 specification team in prose. TASK-027 agents never open those sources or the fork; they
> may use `gt4-gs.exe` only as a black box (final frame images), never its debug dumps or logs. The task brief
> states the agent's status.

### E.4 Mobile (phases 2-3), options only

- iOS cannot spawn executables. Options: (a) CoreGS 2.0 (MIT) as the mobile renderer; (b) personal builds
  on the owner's Mac (TASK-024 already builds with Xcode) that link a GPL GS in process, never distributed;
  (c) a distributed GPL app (requires the game code to be GPL-compatible: not possible under 0067).
- Android: (a) CoreGS 2.0; (b) a separate GPL "renderer app" APK from the fork, connected via Binder +
  shared memory (`AHardwareBuffer`/ashmem) and `SurfaceControlViewHost` to draw into our view (Android 11+);
  (c) a separate `android:process` service inside our APK (one package containing both: weaker
  arm's-length position); (d) ARMSX2 pipeline in process (GPL; same conflict as iOS (c)).

---

## F. Phased tasks

Details, acceptance and branches are in `TASKS-rows.md`. Summary:

| Task | Goal | Repo / branch | Model | Sessions | Depends |
|---|---|---|---|---|---|
| 032 | Docs: decision 0070, CLAUDE.md clean-room text, TASKS, renderer docs | main `docs/decision-0070-gs-process` | Haiku | 1 | owner answers |
| 033 | Fork + reproducible build + CI releases (stock GSRunner first) | fork `gt4gs/main` | Haiku -> Opus for deps | 2-3 | 032 |
| 034 | PoC: `.gscap` -> `.gs` converter, replay with GSRunner, image compare | fork `gt4gs/gscap2gs` + main `task/034-gs-replay-compare` | Haiku -> Opus | 2-3 | local build |
| 035 | MIT stream protocol, IPC ring, renderer seam on main, recorder/null consumer | main `task/035-gs-stream` | Opus | 4-6 | 032 |
| 036 | Live MVP `gt4-gs.exe` (stream replayer, child window) | fork `gt4gs/live` + main `task/036-gs-live` | Opus | 4-6 | 033, 035 |
| 037 | Readbacks and night race | both | Opus | 2-3 | 036 |
| 038 | Window, fullscreen, focus, input, crash handling | both | Opus | 3-4 | 036 |
| 039 | Settings table + `gt4-gs.ini` generator/validator | main + fork | Haiku | 2-3 | 036 |
| 040 | Texture dump/replace | fork + main docs | Haiku | 1-2 | 039 |
| 041 | Performance: F8 `[gs-ipc]`, tuning, owner A/B | both | Haiku -> Opus | 3-4 | 037 |
| 042 | No-JIT hardening (`GT4GS_ONLY`) | fork | Opus | 3-5 | 036 |
| 043 | Launcher integration (download, pin, update) | main `task/019-launcher` | Haiku | 2-3 | 039, launcher resumes |
| 044 | Packaging, release notes, RELEASE-1.0.0 update | both | Haiku | 1-2 | all |
| 045 | High frame rate on the PCSX2 GS (TASK-031 redone) | both | Opus | 10-14 | after 1.0.0 |

**Go/no-go gate (coordinator adjustment, owner approved, 2026-10-15).** After TASK-034 (the proof of
concept) the owner decides, from the side-by-side images and the timings, whether to invest in TASK-035
and TASK-036. No work on TASK-035 or TASK-036 starts before that decision. Gate criteria: 1,101 frames
replayed; frames 100..1100 recognisably the same as the donor (report from the MIT compare script); owner
accepts the images; GS-thread ms/frame is not a blocker at 3x. A "no-go" stops the track at TASK-034 and
keeps CoreGS 1.1 as the release renderer. TASK-032 and TASK-033 can proceed regardless.

Parallelism (one writer per repository and branch at a time): fork track 033 -> 034 runs first; main track
032 runs first. TASK-035 (main) waits for the gate, so the game-side track 035 -> 036 starts after it.
037/038/039 after 036 can run in parallel on separate branches; 043 waits for the launcher; 045 starts
after 1.0.0 except its zero-risk D1 step. (The earlier draft ran 035 in parallel with 034; the gate
replaces that.)
Total to release: **30-45 agent sessions** [H], plus owner tests at 034 (image review), 036, 037, 038,
041 and 044.

Release 1.0.0 consequences (TASK-044 edits RELEASE-1.0.0-PLAN): renderer table becomes **PCSX2 GS**
(download, default when present) / **CoreGS 1.1** (built in, fallback and offline default) / CoreGS 2.0
greyed; paraLLEl-GS removed; W2 (CoreGS 1.1) keeps the upscaler and deinterlace but its FXAA/FSR items
become optional; new workstream W8 = TASK-032..044; the owner gate adds "renderer download, verify and
fallback".

---

## G. Owner answers (2026-10-15)


1. **Fork name:** `moesuito/gt4recomp-gs`, exe `gt4-gs.exe`. An agent may create the public fork in TASK-033
   (not before).
2. **paraLLEl-GS:** dropped from the release; branch `task/012-parallel-gs` archived. Only its MIT renderer
   seam and capture code are salvaged (TASK-035).
3. **CoreGS 1.1:** stays built in as the fallback and offline default.
4. **Upstream base:** PCSX2 stable `v2.8.x` for the release; the proof of concept may use the 2.9.x nightly
   at `685637d`.
5. **JIT-free `gt4-gs.exe`:** required (TASK-042) before 1.0.0.
6. **Window:** the child-window model (option 1) is approved; the GS-owned window (option 2) is the fallback.
7. **Readbacks:** accurate by default; "Unsynchronized" is offered as a speed option.
8. **Internal resolution:** default 3x.
9. **TASK-027 black-box use:** TASK-027 agents may use `gt4-gs.exe` final frame images as a black-box
   reference, final frame images only (never debug dumps or logs). The CLAUDE.md wording is in E.3.
10. **Setup default:** the GS renderer download is on by default during setup (one click).

Coordinator adjustment (owner approved): go/no-go gate after TASK-034, see section F.

Note on references: `docs/GSDX-STUDY.md` (task/012 branch) and `docs/launcher/*` (task/019 branch) are not on
`main`; read them on those branches.
