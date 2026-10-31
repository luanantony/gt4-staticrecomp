# External controller contribution integration

> Historical integration evidence. TASK-131 restores main's program source to v1.2.4 at the
> owner's request. PR #100/#106 and #101/#109/#111 remain merged in Git history, but their
> program changes are withdrawn from the active baseline. A future 1.2.5 should reintroduce
> only the reviewed controller improvements, without the current wheel bridge. Previous test
> results below describe those historical candidates, not a new 1.2.5 acceptance result.

Owner request: 2026-10-25. Accept the contributions one at a time, resolving conflicts and validating
before each merge. This work does not authorize or create another release.

## Baseline and sequence

| Step | Contribution | Integration base | Scope and state |
| --- | --- | --- | --- |
| 1 | PR #100 by luanantony, head `a36486d333c8fa23b89e1bddaa2404979b53dd87` | main `90af154`, stable 1.2.4 plus publication records | TASK-120 merged through PR #106 at main `8b73fa0`; original PR #100 merged; all recorded automated checks passed |
| 2 | PR #101 by JulioHeroZ, head `921ee810ce453036ee47ae5fcce9fd4190ae6344` at inventory | main `8b73fa0` after PR #106 | TASK-121, dedicated `task/121-wheel-integration`; three conflicts resolved, independent reviews pending |

The original contributor commits and authorship must remain in the merge ancestry. Use a normal merge
without history rewriting. If the external branch cannot be updated, use a separate integration PR in
our repository and explicitly link its original contribution. Record the exact heads checked before merge.

## Step 1: trigger normalization and curves

The automatic merge conflicts only in TASKS.md. Keep the current TASK-119 release record and the
contribution scope, but rename its incoming TASK-104 entry to TASK-120: TASK-104 already means save
import on the 1.3.0 line. Planned controller tuning TASK-110 remains broader future work.

Review the complete input path, not only the new formulas:

1. SDL3 standardized Gamepad trigger range versus raw joystick/HID ranges; behavior for DualSense,
   standard pads and XInput; identification, explicit override and reconnect semantics.
2. Native pressure conversion, logical binding versus physical trigger selection, deadzones and curves.
3. Rust controls parser/writer, native parser and UI model round-trip compatibility, including old files.
4. Existing RetroAchievements account revision guards, cancelable HTTP and page-response guards remain.

Run focused native mapping/controls fixtures, complete CTest and the Python suite with the exact new
build, then the Rust workspace and real Controls development UI checks. Record failures before fixes
and do not weaken checks. Fresh native outputs and input pins are required when building game candidates.

## Step 2: racing wheels

Start only after step 1 is completed. Re-fetch PR #101 and compare its wheel delta against the newly
merged main. Its history includes the original RetroAchievements contribution; that is already in 1.2.4
with follow-up fixes. Resolve overlap by retaining those fixes rather than restoring old versions.

Expected overlap with step 1 includes SDL input enumeration, platform pad handling, controller service,
controls page/model and native host input. Review combined settings, device ownership and hotplug rules
explicitly. Keep the second integration on its own branch and worktree, with exactly one writer.

Review multi-device axis/button ownership, controller-versus-wheel routing, resting and pressed pedal
polarities, saved per-device identifiers, calibration bounds, disconnected devices and malformed files.
Review force-feedback initialization, strength bounds, reconnect, stop and shutdown behavior. Register
and run native wheel/mapping controls plus the Rust configuration and launcher contract fixtures, using
SDL virtual devices where available. Check that standard gamepads and keyboard mappings still work.

## Evidence limits and protected state

Automated tests can verify formulas, configuration, virtual-device routing and shutdown contracts.
They cannot establish actual DualSense USB/Bluetooth trigger travel or wheel/pedal/force-feedback feel.
Record those as unvalidated hardware checks, never as a passing gameplay result. The owner controls
in-game testing; do not start another game while an active session exists.

Do not change published 1.2.4 or 1.3.0-rc5 assets, version labels or release notes for this integration.
Do not touch installed applications, owner settings, cards, saves or other active worktrees. No private
junctions or symlinks. PCSX2 sources and `native/hw_gs/` remain outside this work.

## Completion record

For each contribution, record the incoming and integration heads, resolved conflict files, substantive
review fixes, exact build/pin locations, test totals and remaining hardware limits in TASKS.md, STATUS
and the daily journal before merge. ai-memory tools were unavailable at preparation; repository records
are the handoff until that service is available again.

## Review corrections prepared for step 1

Confirmed API contract: [SDL_GetGamepadAxis](https://wiki.libsdl.org/SDL3/SDL_GetGamepadAxis) reports
Gamepad triggers as 0 released through 32767 pressed, including DualSense. Raw joystick/HID axes are
not the same API. The incoming automatic signed-range conversion maps SDL zero to pressure 128;
its manual range selector also has no raw input source here. Both are removed, retaining extended
curves and standard Gamepad behavior. No released controls file contained that mode extension.

The production SDL-to-native reading is extracted unchanged into `read_sdl_gamepad`; the host fixture
uses that actual adapter with its own virtual DualSense-identified and Xbox-identified devices. It opens
only those virtual instance IDs and never selects a physical pad or sends haptics. Added rest, quarter,
half, three-quarter and full travel, neutral release and trigger-to-face-button remap assertions.
Rust sampling uses the same byte reduction as native; capture regressions exercise the same standardized
range. New native curve endpoint, monotonicity and exact byte vectors match browser preview checks.
Both parsers reject unsupported `mode` options on triggers and sticks consistently.

The Slint legacy dropdown remains compatible with the new curve enum and its line preview has samples
for each formula. That separate legacy crate is outside the current Cargo workspace and is not claimed
built by workspace validation. Current Tauri production and development paths are the acceptance focus.
Review corrections are prepared but no build or test was executed by this documentation/implementation
writer; the orchestrator must run and record them before merge. No active 1.3.0 worktree is edited or
ported by either integration step.

Step 1 observed checks: CTest 43/43, Python 641 (42 skipped), Rust 381 passed (4 ignored), UI focus 114/114 and navigation 154/154. New immutable slice1200 records the rebuilt game hash; no published asset was changed.

## Step 2 initial integration record

PR #100 and its follow-up integration are merged; step 2 now starts from their reviewed result. PR #101
head is `921ee810ce453036ee47ae5fcce9fd4190ae6344`. The source imports conflict in `commands.rs` is
resolved by keeping `wheel::{self, WheelConfig}` and `LAUNCHER_DISPLAY_VERSION`. Conflicts in
`ui/dev/index.html` and `ui/dev/focus-check.html` are resolved by retaining both Wheel and Achievements
page scripts. No source behavior was otherwise changed during this conflict-resolution pass.

The merged account-revision, cancelable HTTP and stale-response protections remain; all seven curves
and the standardized SDL Gamepad adapter remain. Reviewers audit native routing/haptics and Rust/UI
configuration independently before any further fixes. Fresh slice1210 output and a new manifest derived
from slice1200 are reserved for after code review and a successful native build; no old pin is edited.

This contribution is linked to TASK-018 but does not establish actual Logitech USB identity, physical
wheel/pedal behavior or real force-feedback operation. No physical haptic test or owner application,
settings, card/save, active session or 1.3.0 worktree mutation is performed. No release is created.

## Validated update and release destination (2026-10-26)

The owner assigned these options to 1.2.5 on the stable code line, then a port to 1.3.0-rc7.
PR #101 advanced to6da047cf. PR #109 (99f8dc48, luanantony) contributes the same original conflict
resolution, based on the earlier921ee810 wheel head. Include both histories without restoring
old behavior. Newly reviewed vendor/axis-count guesses are narrowed to actual wheel identity.
Native ownership precedes gamepad polling; snapshots retain instance IDs; focus/disconnect/stop
clear all force outputs. Capture is GUID-qualified, hotplug-safe, and neutral for missing/ambiguous
devices. The real UI/backend key and telemetry contracts are exercised rather than trusting a
permissive mock. No guest physics, native USB wheel identity, direct gear or clutch feature is added.

Final checks: CTest44/44, Python641 (42 skipped), Rust391 passed (4 ignored), actual UI focus114/114,
navigation169/169 and recorded setting-command replay. New slice1210 runtime/pin preserves old
inputs and manifests. Physical hardware/force feedback remain owner acceptance. Both static reviews
approved the result. No release asset, modern 1.3.0 checkout or render-skip checkout is changed.

Future rc7 acceptance must preserve EE/IOP fixed timing and the frame-skip decision, test controller
and wheel routing with both game targets, keep RA off for Spec II, and integrate en-US/pt-BR UI
strings into the modern launcher. It is a separate source port after this integration, not a reuse
of the old launcher or a replacement of published rc6 assets.

Completion: step 1 merged as #106/main8b73fa0, original #100 merged. Step 2 merged as #111/maincbfd8b8, original #101 and #109 merged with both latest heads preserved. Stable1.2.5 packaging and the modern rc7 source port remain subsequent work; published1.2.4 and rc6 assets are unchanged.
