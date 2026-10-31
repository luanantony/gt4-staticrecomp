# TASK-133: integrate RetroAchievements and controller curves into 1.3.0-rc7

## Superseding execution boundary (owner, 2026-10-26)

Authorized output is the scoped integration, a LOCAL test build/kit and the worker's own
checks with actual evidence, followed by a stop for Codex review and owner gameplay testing.
No PR creation/merge, release upload (including drafts), public publication or next task is
authorized. This overrides the draft-upload delivery wording below. The orchestrator updated
this main copy only; the active worker's worktree was not edited or messaged.

Every result must identify the exact commit, build path, command/log and artifact hash where
applicable. Unknown/unmeasured values stay explicit; do not invent results, repeat old counts
as new runs, or claim hardware validation from mocks. The orchestrator independently reviews
the diff/provenance and repeats the relevant tests before accepting completion.

## Owner decision and scope

Owner, 2026-10-26: the new rc7 is published rc6 plus the RetroAchievements upgrade from stable
1.2.4 and controller-only improvements from stable 1.2.5. Frame skipping is deferred for later
CoreGS 2.0 work and is excluded from this candidate. No wheel bridge or native DFP work enters rc7.
This task is mapped and prepared, not implemented or dispatched. Codex reviews; the owner runs
Gemini/DeepSeek for implementation. Do not spawn agents or launch another coding CLI.

## Frozen inputs and preserved work

| Purpose | Immutable reference | Use |
| --- | --- | --- |
| Published rc6 | v1.3.0-rc6, 58322229c0bd63a0652b0550c5ad2154bd907039 | Exact program baseline |
| RetroAchievements stable | v1.2.4, ad46df65c03894ed204beafbd02819a11e4037bc | Feature plus reviewed account/HTTP/UI fixes |
| Reviewed controller integration | 8b73fa0 (PR #100 via #106) | Semantic controller patch, before wheels |
| Published controller-only stable | v1.2.5, 5d7a669bdecf5e60c0782ca449f48aacac3c7a32 | Final feature reference, not a version/build baseline |
| Deferred frame-skip checkpoint | task/082-render-skip, 15b7bf3971f4046faabc2d0cbd08768ae4883374 | Preserve separately; never merge into this task |

PR #113 is OPEN/DRAFT and unmerged; report docs/TASK-082-RESUMPTION-REPORT.md is committed on
its branch. Implementation checkpoint8da7bff and both owner videos remain referenced there.
Do not rename/delete/rebase that branch or change the CoreGS worktree.

Prepared execution worktree: C:/Antigravity/gt4-wt-133-rc7-ra-controls.
Branch: task/133-rc7-ra-controls, created directly from the published rc6 tag.
The current task/116-117-flare-perf head5f0d3d6 adds only TASKS/journal publication records to
the tag; it must not silently become a moving code baseline. task/101 is older. Do not merge
main wholesale: its legacy launcher and stable release metadata would overwrite modern behavior.

## Read-first and ownership

Read AGENTS.md, current TASKS/STATUS, the rc6 release notes, this brief and ai-memory current
state/project rules/release plan. Resolve task-number collisions explicitly: main's TASK-119
RetroAchievements, modern controller-selection119 and rc6 CPU-parity119 are different records.
TASK-133 is reserved on main for this integration. Preserve all historical tasks when branches meet.

The single writer owns only this new worktree. Other checkouts, owner saves/cards/settings and
installed/portable copies are not writable. Do not link private/. Use the main assets root read-only
for pinned build inputs, and fresh private output/slice directories. No GPL/PCSX2 source reading
or importing is needed. Never inspect/edit native/hw_gs/ or TASK-027/090.

## Integration map

### A. Establish baseline and source provenance

Record exact HEAD, clean/dirty state and selected pins. Run baseline checks against rc6 with the
same toolchain/inputs planned for the candidate. Capture actual CTest/Python/Rust failures and
skips. Historical frame-skip Python failures were not independently proven baseline failures;
do not repeat that claim or weaken tests. Stop for review if the baseline requires unrelated fixes.
Compare original PR #96/#105 and reviewed #100/#106 history, but port from released fixed code.

### B. RetroAchievements native library and runtime

Reuse native/achievements.*, achievements_http.*, achievements_overlay.*, their fixtures,
native/third_party/rcheevos/, THIRD_PARTY and the CMake wiring from1.2.4. Retain reviewed
cancelable WinHTTP shutdown and lifecycle behavior. The original vendored rcheevos provenance
is upstreambbc79b58a5a2a0b98dfbf2be6ca9e42c764661ce; verify unchanged vendor files by Git blobs.

Adapt native/window.cpp hooks into the rc6 lifecycle rather than replacing the file: initialize,
per-VBlank evaluation, overlay movement/resize, cleanup. Preserve rc6 VU1/MTVU journal ordering,
readback/performance-mode behavior, input, audio and pacing. Review RAM access/thread ordering.

Enforce the target restriction inside the executable as well as the launcher: only original
GT4 v2.00 may initialize/evaluate/send achievements. Spec II must stay inactive even with
inherited/forged GT4_RA_* settings. Never use the original-game hash for Spec II. Keep the original
game's identity and memory map explicit; do not infer support from common titles or executables.

### C. Account/configuration and modern launch adaptation

Port gt4l-core/src/retroachievements.rs and the necessary path/model/state methods. Account
configuration is shared user configuration, resolved through the modern UserDirs/BuildKind APIs;
cache location must be explicitly resolved through the modern data/game layout. Preserve installed
and portable isolation, existing1.2.4/1.2.5 credentials and controls, and per-game saves/runtime paths.

Adapt commands.rs prepare_play/PlayInputs, state.rs, commands.list, main.rs and capabilities as
one contract. Inject enabled RA configuration only for games::GT4, after environment scrubbing.
For games::SPEC2 explicitly disable RA and prevent inherited credentials/hash from activating it.
Use temporary fixtures; never read the owner's credential files or print/store passwords/tokens.

Retain account revision guards: refresh/login in flight cannot restore logged-out credentials,
overwrite newer account choices, or paint stale target/account UI. Audit target switching too.
No secrets in logs, exported diagnostic bundles, command-line arguments or test artifacts.

### D. RetroAchievements UI and two languages

Port the achievements page, settings route/tab, app model, dev mocks and focus/navigation
coverage into rc6's modular UI. Do not replace modern settings.js, index.html or app state with
the legacy equivalents. Keep game selection, Games/Saves/Replays pages, version picker and channels.

Translate all added player-facing strings into en-US and pt-BR, including native overlay strings
where applicable. The English-only stable1.2.x exception does not apply to1.3.0. Selected Spec II
must clearly indicate achievements are unsupported/disabled; choosing it must not erase the shared
account or original-game preferences. Match locale keys/placeholders and test target/language changes.

### E. Controller-only upgrade

Port four curves (s_curve, smooth_power, sublinear, logarithmic) into native/controls.*,
gt4controls/src/lib.rs, modern Tauri model.rs and ui/js/controls-model.js, preserving all aliases,
defaults, serialization, deadzones and previews. Native/launcher formulas must agree.

Retain SDL Gamepad triggers' standardized0..32767 range for all mapped gamepads; no signed-axis
or device-name DualSense heuristic. Review read_sdl_gamepad/host_io fixture changes and pad.rs
preview/capture consistency against current rc6 code. Apply shared control behavior to both game
targets, without replacing their distinct EE/IOP programs. Preserve modern player/device behavior.
Slint gt4l-app changes apply only if that executable is still built/used; record the disposition.

Do not import wheel.cpp/wheel.rs/sdl_wheels.rs, wheel commands/UI, wheel.toml, force-feedback
adaptation or PR #101/#109/#111 changes. Published1.2.5 is already controller-only; use its fixed
code, not the superseded wheel draft branch or its installer.

## Delivery sequence and review gates

1. Baseline/provenance report and proposed target/configuration contract. Stop for Codex review
   before implementing if unresolved architectural choices remain.
2. RA integration with native+launcher Spec II negative tests and reviewed cancellation/race tests.
3. Controller semantic port with fixture/preview/persistence controls for both targets.
4. Full regression/language checks; report actual commands, build paths, counts, skips and limits.
5. Fresh builds of both target executables where shared native code changes, fresh pins and
   distribution manifests. Do not ship the1.2.5 native executable as the rc7 executable or reuse
   Spec II's old binary after changing compiled shared native code without evidence.
6. Prepare full rc7 notes and both short notes before launcher builds. Build numeric1.3.0 with
   rc7 labels, installer and portable editions, correct executable names gt4.exe/gt4-spec2.exe.
7. Verify extracted installer and portable assets against source manifests and SHA256SUMS.
   Upload as draft only after automated checks. Owner tests; Codex reviews; merge/publish only
   with explicit owner authorization. No request here authorizes a public rc7 or final1.3.0.

## Acceptance matrix

| Area | Required evidence |
| --- | --- |
| Native correctness | CTest/Python against each actual candidate build; account/logout and HTTP cancellation regressions; source/vendor/pin checks |
| Target isolation | Original game RA on/off; Spec II RA forcibly off at launcher and native boundary, even with inherited environment; target switches |
| Account lifecycle | Delayed refresh/login vs logout/new account; no stale restore; missing/invalid config; no credential leakage in diagnostics |
| Controller behavior | All7 curves round-trip; monotone endpoints/neutral/defaults, trigger rest/full travel, capture and preview/native consistency; keyboard/gamepad regressions |
| Modern launcher | Rust workspace and contracts; installer+portable layout;1.2.5->rc7 and rc6->rc7 update fixtures; channels/version picker; saves/game isolation |
| Languages | en-US/pt-BR locale key+placeholder parity, translated RA/control text, focus/navigation in both languages, matching short notes |
| Renderer/runtime | rc6 performance-mode readbacks, MTVU/VU verification and normal rendering retained; no frame-skipping code/flag/UI/pin from TASK082 |
| Owner gameplay | Both games, original RA login/overlay/unlock/logout, physical controller response, audio, window/fullscreen/API coverage, saves intact |

Real-account unlock and physical-controller feel remain owner acceptance, not claims established
by mocks. No new performance gain is promised. Retain screenshots/logs only in ignored fresh
outputs and cite paths/hashes in the handoff. End with exact commit, test results, unresolved
limits, pins and draft links, then stop. Do not start another task automatically.
