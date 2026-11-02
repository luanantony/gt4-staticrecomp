# Current Status

## TASK-077: Context-aware input mapping (2026-11-01)

Completed TASK-077, implementing native context-aware input mapping separating race controls from menus and garages (e.g. Esc = Start in races, Triangle in menus).
- Host-side VBlank context probe (`native/context_probe.{hpp,cpp}`) inspecting guest owner pointer `0x622F4C`, 10 race class vtables, attract demo detection, and pause objects (`race + 0xE418`).
- 3-frame debounce filtering with cached offset resolution.
- Context transition filter (`native/context_input.{hpp,cpp}`) enforcing the re-arm rule and clean switch frames to prevent stuck keys across scene changes.
- Schema 2 controls (`native/controls.{hpp,cpp}`) supporting per-context override tables (`[bindings.<context>]`) with full fallback to `[bindings]`.
- Verified with `context_probe_fixture` (all passed), `controls_fixture` (all passed), `test_controls_probe.py` (7 passed), and `gt4controls` (21 passed).
- Documentation: `docs/TASK-077-CONTEXT-INPUT-MAPPING.md`, `docs/CONTROLS.md`.

## 1.3.0-rc14: closing as a pre-release (2026-11-04, afternoon)

On top of the slice407 test build: the full trigger range on by default with a fixed-width meter (TASK-161), the shader
compile guard and `shaders.log` in the log bundle (TASK-170), and Spec II images accepted by their code (TASK-173). Games slice408 (PGO and ThinLTO from `build/cam360/*-units-rc14b`;
pins `docs/inputs/slice408-*.json`, exe `a4e1c742...` v2.00, `e5267b8f...` Spec II; distribution
`private/slice408-dist-130-rc14`), launchers labelled rc14 (`private/slice408-launchers-rc14`). Gates against rc13 identical
(race clock, simulation state, every compared frame, VU1/VU0 and MTVU 0 differences); CTest 301/301 on both games, Python 790,
launcher 665, focus checks 294/294 and 301/301. Installer `d8ac9593...`, portable zip `961047fc...`
(`private/release-130-rc14-out`; contents checked against the distribution and the launchers). Published as the pre-release
`v1.3.0-rc14` (the owner's word given). Next: TASK-171 (the first license on laptops) for rc15. Recorded: TASK-172 (an earlier
Spec II release needs its own build).

## TASK-095: 2D overlays over the race view at ultrawide ratios, both games (2026-11-04)

Branch `task/095-ultrawide-menu-canvas` (re-does PR #177, Luan Antony co-author); `docs/ULTRAWIDE-OVERLAYS.md`. Pre-race screens,
pause, pit menu and results keep their 16:9 shape, centred, at 21:9 and 32:9; 16:9 identical. Owner test pending.

## 1.3.0-rc14 test build on the owner's desktop and laptop (2026-11-04, morning)

For the owner's test before any pre-release (no PGO until the owner asks). Contents: TASK-095 (PR #200, re-does Luan Antony's
#177: the 2D overlays over the race view keep their 16:9 shape at 21:9 and 32:9), TASK-159b, TASK-160 and TASK-161 (PR #197: the
camera's pedal zeroing only on the right stick Y, FoV zoom on R3 + D-pad on by default, full trigger range off by default with a
launcher switch), TASK-164 (first-start note on Play, issue #160), TASK-166 (Spec II ultrawide: the race font x scale and the
mirror divisor patched as on v2.00, `scripts/spec2_aspect_patch.py`, units `private/slice405-spec2-aspect-producer`) and TASK-167
(the Spec II PT-BR + CPSP image named). Games slice407, plain (pins `docs/inputs/slice407-*.json`, exe `4e87eb83...` v2.00,
`10b7ce52...` Spec II, EE libraries from `build/cam360/*-units-rc14b`; distribution `private/slice407-dist-130-rc14-test`),
launchers labelled rc14 (`private/slice407-launchers-rc14-test`), installer
`private/slice407-installer-130-rc14-test/GT4Recomp-Setup-1.3.0-rc14.exe`. Gates against rc13 (`gates5.sh`, wrong=0): race clock,
simulation state and every compared frame identical on three Spec II laps, the v2.00 night race and Arcade path, Spec II at 6x;
VU1/VU0 and MTVU checks 0 differences. TASK-095 overlay check on Spec II `george-pausa`: 16:9 identical to the build without the
overlay calls (clock, state, 121 of 121 frames); at 21:9 the race identical and only the lobby and pause frames differ, 5 of 5
narrowed by 1/1.3125 around the centre. CTest 300/300 in both builds, Python 785/785 (after the TASK-118 test update), launcher
tests 662, focus checks 293/293 and 300/300, nav-test 217/217, locale keys 695 in both. Desktop updated through the installer's
update mode (backups `private/_program-backups/rc13test-before-rc14test-20261104T120226Z` and
`rc14test-before-rc14b-20261104T134327Z`), the laptop's portable copy in place (backups `avell-rc13test-before-rc14test-...`,
`avell-rc14test-before-rc14b-...`). The slice406 build (the same without TASK-095) was installed first and is superseded.

## 1.3.0-rc13 published as a pre-release (2026-11-04)

Tag `v1.3.0-rc13`. Contents: TASK-149 (Nordschleife fences), TASK-150 (showroom: large writes merged on the GPU, IN-538),
TASK-151 (photo mode flicker), TASK-152 to TASK-158 (contributor pull requests re-done, Luan Antony co-author) and TASK-159 (360
chase camera on the right stick, on by default, PR #194). Games slice404 (PGO and ThinLTO from the camera's hooked units; pins
`docs/inputs/slice404-*.json`, exe `901f997e...` v2.00, `10cb28b5...` Spec II; distribution `private/slice404-dist-130-rc13`);
launchers labelled rc13 (`private/slice404-launchers-rc13`); installer `e96ca3e1...`, portable zip `742ac156...`
(`private/release-130-rc13-out`). Gates against rc12 (`gates4.sh`): race clock and simulation state identical on three Spec II
laps and the v2.00 night race, every compared frame identical, VU1/VU0 and MTVU checks 0 differences. Launcher: locale keys 687 in
both languages, focus checks 278/278 and 285/285, nav-test 209/209. Laptop (RTX 3060, 6x): showroom 60.0 FPS, photo mode
59.4-60.0 FPS. Not in it, off by default after measurement: IN-537 and IN-539 (photo mode frame times, TASK-162).

## 1.3.0-rc13 test build on the owner's desktop and laptop (2026-11-04, night)

For the owner's test suite before any pre-release. Contents: TASK-149 (an upload over a mip level draws the pending batch
first: the Nordschleife's far fences), TASK-151 (the display prefers the circuit's width: photo mode no longer black every
other picture), and the contributor pull requests re-done by hand with Luan Antony as co-author: TASK-152 (#120 PCSX2 cards),
TASK-153 (#116 delete the active save), TASK-154 (#121 ReShade's Home key with GSdx), TASK-155 (#136 portable.ini beside
gt4-gs), TASK-156 (#141 Wine audio profile, `GT4_AUDIO_LATENCY_MS`), TASK-157 (#145 custom textures folder), TASK-158 (#143
GSdx retry and install from a file). Each PR recorded as merged on `main` (tree-neutral) and its issue answered.

Games slice403, plain (pins `docs/inputs/slice403-*.json`, exe `b2f12a10...` v2.00, `d16acc88...` Spec II; distribution
`private/slice403-dist-130-rc13-test`), launchers labelled rc13 (`private/slice403-launchers-rc13-test`), installer
`private/slice403-installer-130-rc13-test/GT4Recomp-Setup-1.3.0-rc13.exe`. Gates against rc12 (`gates4.sh`): race clock,
simulation state and every compared frame identical, VU1/VU0 and MTVU checks 0 differences. Launcher tests 659, focus checks
278/285, nav-test 209, locale keys 687 in both. Desktop updated through the installer's update mode (rc12 test files in
`private/_program-backups/rc12test-before-rc13test-20261104T064835Z`); the laptop's portable copy updated in place (backup
`avell-rc12test-before-rc13test-20261104T064859Z`). Still open for rc13: TASK-150 (showroom frame times) and TASK-151's frame
times: both scenes read GS memory back (the showroom every other frame, photo mode every frame) and wait for the GPU.

## TASK-161: full trigger range for the throttle and the brake, both games, off by default (2026-11-04)

Branch `task/161-trigger-sensitivity`; `docs/research/TRIGGER-SENSITIVITY.md`. Owner test decides the default.

## TASK-160: field of view zoom on R3 + D-pad, both games, on by default, checked headless (2026-11-04)

Branch `task/160-fov-adjustment`; `docs/FOV-ADJUST.md`. Owner test pending.

## TASK-159: 360-degree chase camera on the right stick, both games checked headless, on by default (2026-11-04)

On `task/159-360-chase-camera` (from task/134 at `ad457090`). On by default in v2.00 and Spec II (owner, 2026-11-04; in rc13):
`GT4_CAMERA_360=0` or `camera_360=0` in `game.ini` turns it off.
The right stick X turns the chase camera round the car through the game's own camera spring (a host call that
`scripts/camera360_units.py` adds to the translated yaw call), and the right stick Y stops being the analog throttle and brake
while it is on. Headless v2.00 Arcade race (plain test build): switch off identical to the build without the hook (frames,
race clock, state); switch on: race clock identical at every VBlank, simulation identical once the stick is released (only the
camera's own words differ while it is held), the view turns and comes back; with the stick at rest, identical. Spec II, the
owner's Tokyo R246 Time Trial with the stick moved: the same results (the simulation words that differ after release are the
camera's and the ones drawing writes). Owner test in game pending. `docs/CAMERA-360.md`.

## 1.3.0-rc12 built with PGO and published as a pre-release (2026-11-04, night)

On the owner's word ("a rc12 atual assim que buildar, vc já sobe como pre-release"): TASK-145 to TASK-148 on top of rc11. Games
slice402 (PGO and ThinLTO, `private/task134-tools/final12.sh`; exe `3c8f870c...` v2.00, `9b3c4d38...` Spec II; pins
`docs/inputs/slice402-*.json`; distribution `private/slice402-dist-130-rc12`), launchers labelled rc12 from `e562048f`
(`private/release-130-rc12-launchers`), package `private/release-130-rc12-out` (installer `28b6776b...` 51,870,003 bytes, portable
zip `0932edb7...` 80,813,207 bytes, SHA256SUMS), checked against its sources by extraction. Gates against rc11
(`private/task134-tools/gates3.sh`): race clock and simulation state identical on three Spec II laps and the v2.00 night race,
every compared frame identical, VU1/VU0 and MTVU checks 0 differences. Locale keys 664 in both languages, focus checks 275/282
and nav-test 209 on this launcher's UI. Tag `v1.3.0-rc12` at `e562048f` (the launcher's source; the game sources are those of
the build).

## TASK-147 and TASK-148 for rc12: triggers, sticks and the controller choice (2026-11-03, night)

On `task/134-game-code-optimization-study`, plain test builds on the owner's laptop. Triggers: R2 spans GT4's own 0..45
throttle range and L2 its 0..84 brake range (the game scales the pressure byte and reached 100% at a third of the travel);
the owner approved them. Sticks: a 10% dead zone, and the bytes step over GT4's own 29% stick dead zone, so steering is
proportional from the first step (it was dead up to about 46% of the travel, then too steep). Controller choice: a dropdown
on the Controls page (`controller` setting, `GT4_PAD`); the game reads the chosen pad, else one of its model, else
controller 1. Owner change (2026-11-04): no "Automatic" entry; the dropdown lists only the connected controllers and the
launcher stores the one it reads when the stored one is not connected (checked again at every launcher start). TASK-147 is
done (owner, 2026-11-04: control precision solved); issues #183, #134 and #178 are closed as fixed for rc12.

GitHub issues (2026-11-04, owner's request to close what is fixed): 11 closed with a note each. For rc12: #183, #134, #178.
Fixed in published pre-releases: #191 (GT Auto bubbles, rc10), #185 (Spec II replays, rc10), #139 and #179 (GSdx missing:
Play offers CoreGS 2.0, rc10), #115 (no sound device, rc10), #123 (toggles with a controller, rc8), #132 (portable build,
rc7), #89 (integrated GPU start, rc5; the reporter races on rc6 in #112). Left open: #99 (flashing window then a hang exit,
the TASK-145 fix) until rc12 is public; the others have no confirmed fix or are feature requests. 25 issues stay open.
Later the same day (owner): TASK-148 done too, #99 closed (rc12 goes out today), and #161, #88 and #128 answered; 24 issues
stay open (`docs/ISSUE-TRIAGE.md`).

Test installer (2026-11-04 UTC): the same plain builds pinned as slice 401 (`docs/inputs/slice401-pcsx2.json`, gt4.exe
`eac9918f`; `slice401-spec2.json`, gt4-spec2.exe `2b1f3202`), distribution `private/slice401-dist-130-rc12-test`, launcher
labelled rc12 (`89164318`), `private/slice401-installer-130-rc12-test/GT4Recomp-Setup-1.3.0-rc12.exe` (`63fcdb23`). Installed
on the owner's desktop through the installer's update mode over rc11 (silent, result ok; installed game files, launcher and
SDL3 match their sources); rc11 program files backed up in
`private/_program-backups/rc11-before-rc12test-20261104T023355Z`. Not uploaded; the rc12 release build (PGO) comes later.
Then the launcher without the "Automatic" controller entry (TASK-148 owner change; launcher `4e0a7e50`, same games):
`private/slice401-installer-130-rc12-test2/GT4Recomp-Setup-1.3.0-rc12.exe` (`cf90c041`), installed the same way (result ok,
files match); the first test install is in `private/_program-backups/rc12test-before-rc12test2-20261104T031129Z`.
The owner's laptop got the same launcher as a portable build (`7e46a64a`, copied over `Y:\GT4Recomp.exe`; its games already
were the slice 401 builds); the previous one is in `private/_program-backups/avell-rc12test-before-rc12test2-20261104T041009Z`.

## TASK-146 for the owner's test: 60 FPS lets a race drop frames instead of slowing (2026-11-03, night)

On the owner's word, experimental in the rc12 line: the launcher's Frame rate "60 FPS" (CoreGS 2.0, the default) now passes
`GT4_FRAME_SKIP=1` (`game.ini` `hw_frame_skip_adaptive=1`), TASK-082's frame skipping when behind. With time to spare nothing
is left out; when the PC falls behind in a running race, frames are left out and the game keeps its full speed and 60 Hz
physics. 30 FPS is unchanged; menus never skip. The first laptop test crashed at a race's end inside the NVIDIA driver: the
paced presentation path (frames left out, the 30 FPS mode too) shared the GS thread's command pool, and the frametime
thread's present wait overlapped the swapchain's acquires and presents in every mode. Both fixed (IN-536; the validation
layer's thread checks: 10 overlaps before, 0 after). Second test: no crash, but 99.1% game speed and audio underruns at 6x.
On the owner's rules (60 Hz simulation always; 30 FPS from the race intro): the gate opens in the intro (race object, clock 0),
the 60 FPS policy acts at a frame of delay and may leave out up to 8 in a row, the pacer owes up to 12 frames. Both frame
skip suites pass; 100.00% at 12x on the desktop. Laptop builds write `frameskip.csv`; owner test pending.

## TASK-145 fixed for rc12: the desktop flash every 6 seconds in fullscreen on NVIDIA RTX (2026-11-03, night)

On `task/134-game-code-optimization-study`, not built as an rc yet (the owner has more for rc12). In borderless fullscreen with a
gamepad the game window's message queue stays empty, and a PeekMessage on an empty queue does not count as a read, so Windows
declared the window "not responding" every few seconds; on RTX PCs the shell and `dwm.exe` then flashed another window or a white
Ghost window in front of the game for a frame (CoreGS 2.0 and GSdx alike), and after Alt+Tab the game could come back white. The
game window now keeps a one-second timer (`native/window_heartbeat.hpp`, decision 0076: keep it, and why). Owner test on the
laptop: a full lap with no hang (before: one every 360 VBlanks), Alt+Tab and GSdx fine. CTest `native_window_heartbeat`, Python
`test_window_heartbeat.py`.

## 1.3.0-rc11 published as a pre-release (2026-11-03, night)

On the owner's word after their test ("tá tudo certo"): `v1.3.0-rc11` at `52645c6b` (the build's source), pre-release, not
Latest; v1.2.5 stays Latest; rc10, rc9 and rc8 stay published. Assets from `private/release-130-rc11-out` (installer
`afb176ef...`, zip `fbdd4abb...`, SHA256SUMS), uploaded as a draft, the three GitHub digests compared with the local files, then
made a pre-release. The owner chose not to wait for a 2026-11-04 tag: on the releases page rc11 sorts by tag text among the
2026-11-03 releases (the launcher compares numerically, so Beta players are offered rc11).

## 1.3.0-rc11 built and installed for the owner's test (2026-11-03, night)

On the owner's word: TASK-142 (IN-535, the R34 rear glass) and TASK-143 (GitHub contributors in About, with Kaezzey; README
credits on `main`; Luan's #118, #119, #154, #174 marked merged by a tree-neutral merge). Games slice400 (exe `7a4c51f1...` v2.00,
`10fd1b88...` Spec II; pins `docs/inputs/slice400-*.json`; distribution `private/slice400-dist-130-rc11`), package
`private/release-130-rc11-out` (installer `afb176ef...` 51,842,164 bytes, zip `fbdd4abb...` 80,762,662 bytes), checked against
its sources. Gates against rc9: clock and state identical everywhere, v2.00 frames identical, Spec II frames differ only on car
glass (identical with the fix switched off); VU1 and MTVU checks 0 differences. CTest 293/294 (python_native_contracts, known),
launcher cargo test 630, locale keys 660 in both, focus checks 275/282, nav-test 209. The installer now has a version resource
and the launcher names its publisher (a laptop's Defender flagged the rc10 installer by machine learning; local scans are clean).
Installed through the installer's update mode; rc10's program files in
`private/_program-backups/rc10b-before-rc11-20261103T213036Z`. Not uploaded. When it is, the tag must point at a commit dated
2026-11-04 (UTC) or later: GitHub lists releases of the same day by tag text, so an rc11 from 2026-11-03 would sort below rc9.

## TASK-142 fixed for rc11: the car in front through a Skyline R34's rear glass (2026-11-03)

On `task/134-game-code-optimization-study`; in 1.3.0-rc11 (above). GT4 clears its environment map with two sprites whose depth buffer
lies in the map's own memory; CoreGS 2.0 kept that depth apart, half of the map kept alpha 0, and the R34's glass reflection (scaled
by the alpha a car's reflection left behind the glass) vanished over the car in front, which stayed sharp and dark. A depth fill
now writes the colour target whose memory it covers (IN-535, `hw_core2_depth_alias`). On the owner's recording at 6x the glass is
misted as on GSdx (8 of 13 frames change, 7 closer to GSdx, 1 equal); Tokyo R246 (17 of 43 frames, all closer to GSdx) and El
Capitan (6 of 43, equal) change only on car glass, by at most 133 pixels. The owner saw the frames: fixed. New diagnostics: `GT4_CORE2_PROBE_DRAWS`/`_RECT`, `GT4_CORE2_SKIP_DRAWS`,
`GT4_CORE2_DEPTH_ALIAS_TRACE`, depth range, tests and TEX0 in the frame trace.

TASK-143 (owner): the launcher's About page credits the GitHub contributors with a merged or open pull request (gustavocanepa,
JulioHeroZ, luanantony), en-US and pt-BR; focus checks, nav-test and launcher tests pass. Release launcher built with label rc11,
not installed and not packaged.

## 1.3.0-rc10 published as a pre-release (2026-11-03)

On the owner's word after their tests ("tudo certo nos meus testes"): `v1.3.0-rc10` at `63bd3321` (no code after `079601ac`, the
build's source), pre-release, not Latest; v1.2.5 stays Latest. Assets from `private/release-130-rc10b-out` (installer `b07b7097...`,
zip `2eef869d...`, SHA256SUMS), uploaded as a draft, the three GitHub digests compared with the local files, then made a
pre-release. Known issue added to the notes and to TASKS (TASK-142, rc11): on CoreGS 2.0 close behind another car, a car's
transparent glass shows parts of the car in front (owner, 6x). Launcher language checks as for the first rc10 build (the UI did
not change since).

## 1.3.0-rc10 rebuilt with the CoreGS 2.0 custom texture fixes (TASK-141), installed, not uploaded (2026-11-03, night)

The owner's test of the first rc10 found two glitches with the HD HUD & UI pack on CoreGS 2.0 (a player saw a third): HUD digits
showing the game's texture for a frame on Tokyo R246, and the GT Auto logo and Oil Change title doubled. Fixed (IN-533, IN-534,
commit `079601ac`; TASK-141) and rc10 rebuilt under the same label: game slice399 (exe `0f0a4b3d...` v2.00, `289396db...` Spec II,
pins `docs/inputs/slice399-*.json`, distribution `private/slice399-dist-130-rc10`), package `private/release-130-rc10b-out`
(installer `b07b7097...` 51,818,176 bytes, zip `2eef869d...` 80,744,000 bytes), checked against its sources. Gates against rc9
all equal (the same set as below). GT Auto recording at 4x and 6x with the pack: the logo and title drawn from the pack in every
frame, no doubled copy. Installed on the owner's desktop through the installer's update mode (files match the manifest); the
first rc10's program files are in `private/_program-backups/rc10a-before-rc10b-20261103T190658Z`. Not uploaded (owner).

## 1.3.0-rc10 built for the owner's test, not uploaded (2026-11-03)

TASK-140 (the player fixes the owner chose from the triage of 47 open PRs and the open issues; ai-memory
`notes/pr-issue-triage-2026-11-03`) and one more TASK-134 step, on `task/134-game-code-optimization-study`. **Owner: do not
publish yet, not even a draft.** Package `private/release-130-rc10-out`: installer `GT4Recomp-Setup-1.3.0-rc10.exe` 51,814,328
bytes `7e90395f...`, portable zip 80,742,596 bytes `fea09567...`; game `private/slice398-dist-130-rc10`, pins `slice398-*.json`
(exe `9b71df8b...` v2.00, `531eeccb...` Spec II).

- **Fixes:** Spec II replays kept (its names are `BASCUS-97436R` plus seven letters and digits; the launcher kept only four digits,
  issue #185); no audio device runs silent (#93, #115); the SPU2 exponential release reaches silence, so GT Auto's bubbles stop
  (#104, capsule `private/slice396-spu2-runtime`); card backups limited to the newest 10 card images, 5 before a New save, 10
  active-save copies (owner: 74 images, about 600 MB, in two weeks); plain exit messages (#107, #99); Play offers CoreGS 2.0 when
  GSdx is not installed (#139, #179); the card error names the file (#160); the 480p hold covers a 1080i save. Ideas of PRs #118,
  #119, #154 and #174 by luanantony, re-done with him as co-author; items 3, 5 and 7 by a Sonnet agent on a merged branch.
- **CPU:** VU1 products with VF0.w emitted as copies (table `private/slice397-vu1-gen`, 20,530 lines, every one that rewrite): VU1
  program -3.4%; whole process in a player's session rc9 9.31 -> rc10 9.26 ms per VBlank (-0.6%, inside the spread).
- **Gates against rc9, all equal:** clock, state and 24/35/35 frames on the three recorded laps; v2.00 night 3x 10/10 and state;
  Arcade 14/14; 6x 12/12; VU1 verify 236,448 and 355,388 samples, MTVU 8,032 frames, 0 differences.
- **Tests:** CTest 292 of 293 (`python_native_contracts`, known); Python 766, the 13 known failures; launcher `cargo test` 630
  with `flow_audit::a_1_2_x_install_moves_into_games_gt4_at_the_first_start` failing under heavy disk load (also on unmodified rc9
  sources) and passing 3 of 3 on a quiet machine; locale keys 655 equal; focus checks en-US 275, pt-BR 282, nav-test 209.
- **Installed on the owner's desktop** (owner's word, 2026-11-03): the rc10 installer in its update mode (`/S /UPDATE`, elevated);
  `version.json` reads 1.3.0-rc10, the launcher is the installer build (`e0208c96...`), the 19 game files match `MANIFEST.json`.
  The rc9 program files are backed up in `private/_program-backups/rc9-before-rc10-20261103T174300Z`.
- **Open:** the owner's test (a car wash or oil change in GT Auto; a Spec II replay saved, the game closed, the replay found again;
  a launcher on GSdx without it installed). The notebook still has the first rc9.

## 1.3.0-rc9 published as a pre-release (2026-11-03)

The owner approved rc9 (slice395, installed on their desktop) and asked to publish it as a pre-release at once, because
rc8 has the CoreGS 2.0 picture bugs that TASK-139 fixed. `v1.3.0-rc9` at `39320b99` on `task/134-game-code-optimization-study`
(the branch is not merged into `main`); pre-release, not Latest; `v1.2.5` stays Latest and `v1.3.0-rc8` stays published.

- **Game:** slice395 (`private/slice395-dist-t134r2`, pins `slice395-pcsx2.json` and `slice395-spec2.json`), the files installed
  on the owner's desktop.
- **Assets** (`private/release-130-rc9b-out`, launchers labelled rc9 from the same commit): installer
  `GT4Recomp-Setup-1.3.0-rc9.exe` 52,620,178 bytes `4f2c54bc...`, portable zip 81,746,509 bytes `fdb54662...`, `SHA256SUMS`.
  The GitHub digests of all three equal the local files.
- **Checks:** installer (7-Zip extraction) and zip against the game folder, 0 differences, no game data, no build paths in the game
  programs; both launchers carry the rc9 label and the updated 1.3.0 notes in both languages; locale keys and placeholders equal
  (645); focus checks en-US 258 of 258, pt-BR 265 of 265; nav-test 209 of 209; launcher `cargo test` 0 failures.
- **Notes:** `docs/release-notes/1.3.0-rc9.md` (the GitHub body) rewritten with highlights, the picture fixes first and the
  measured numbers; the short notes in en-US and pt-BR have the same 10 bullets.
- **Not done:** the notebook still has the first rc9 build (slice390) under the same version label, so its launcher does not offer
  this one; it needs a manual update when the notebook is on.

## TASK-134, second round: rc9 with the picture fixes uses about 10% less processor (2026-11-03)

The owner resumed the CPU work after TASK-139 and asked for every optimisation overnight, one at a time, measured, kept even when
small, on the desktop only (the notebook was off). Final build `build/t134-cand/f3-spec2` and `f3-v200` in the worktree (PGO and
ThinLTO again, exe `6ad2e34b...` and `b3989168...`), pins `docs/inputs/slice395-spec2.json` and `slice395-pcsx2.json`, game files
in `private/slice395-dist-t134r2`. **Installed on the owner's desktop** (owner's word, 2026-11-03, elevated copy into
`C:\Program Files\GT4Recomp\game`; the 19 files match `MANIFEST.json` by hash). The slice392 files it replaced are backed up in
`private/_program-backups/t139-before-t134r2-20261103T072312Z`; the notebook keeps rc9. Not uploaded; rc9 is still not published.
- **El Capitan and Tokyo R246** (same session kind, 2 rounds, slice392 -> this build): process CPU 7.76 -> 7.10 ms (-8.5%) and
  7.43 -> 6.94 ms (-6.6%); wall -10% on both.

- **Result, player's session (warm shader cache), recorded George V Paris lap, Spec II, 1x, headless, 3 interleaved rounds, Ryzen
  7 5700X and RX 9060 XT, process CPU per VBlank:** rc8 12.96 ms, rc9 10.48, rc9 with the TASK-139 fixes (slice392) 10.59, this
  build **9.57 (-9.7% against slice392, -26% against rc8)**. Wall without pacing 5.65 -> 5.15 ms (-9%).
- **Bench fix first:** the headless runs had no shader cache and compiled CoreGS 2.0's pipelines during the lap; the bench tools
  now warm one per executable (ai-memory `gotchas/headless-bench-shader-cache`). rc9's 2026-11-02 numbers were taken without it.
- **Kept, each measured against the step before:** CoreGS 2.0 submits every 64 batches on a discrete GPU (GS -5% at 1x); the VU1
  CLIP flags over the lanes (-2.6% of the VU1 program); the VU1 outer product as one vector operation (-8.5%); the VU0 programs over
  four lanes with fp4 (-24% of VU0); GPU vertices inserted per primitive (GS -3.1%); partial VU loads and stores as whole qwords
  (VU1 -2.8%). The VU1 and VU0 tables are regenerated by the changed emitter (`private/slice394-*`). Journal 2026-11-03.
- **Dropped with a measurement:** the VIF wrap mask and a per-format generic VIF unpack (exact, no gain); uber pipelines only (2%
  less CPU, 5% slower frame).
- **Same game (gates against slice392, `private/task134-tools/gates2.sh`):** race clock and simulation state identical on the
  George V Paris, El Capitan and Tokyo R246 recordings, with 24, 35 and 35 frames identical; v2.00 night race at 3x, state and
  10 of 10 frames; Arcade path 14 of 14; Spec II lap at 6x 12 of 12; VU1 and VU0 verify against the exact interpreter on both
  games and the MTVU check over 8,032 frames, 0 differences.
- **Not verified:** the notebook (off); a window session; the owner's own test.
- **The dark block on a track sign** (VBlank 3750, 2 of 12 runs on rc8 and rc9): not seen in 12 runs of this build, probably fixed with IN-528.
- **Left:** a CoreGS 2.0 submission thread (presentation submits on the same queue, so every submission needs one ordered queue);
  a deferred flare readback on the GS thread; the SPU2 envelope; P8. Study section 8, "Second round".

## TASK-139: CoreGS 2.0 picture bugs, fixed on rc9 and merged into the CPU branch (2026-11-03)

**Done:** the owner tested slice392 in the game ("everything fine so far") and approved the merge into `task/134-game-code-optimization-study`, where the TASK-134 agent continues. Not uploaded.

Branch `task/139-coregs2-picture-bugs`, from `task/134-game-code-optimization-study` at `56f90c96`. The owner saw graphic bugs and records drives for us to replay on both renderers.

- **Tools:** `scripts/coregs2_bench/record.cmd <name>` records on the rc9 package; `replay-run.sh` replays on CoreGS 2.0 and the new `replay-gsdx.sh` on the reference renderer, final frames only.
- **Checked:** a short recording through `record.cmd` ended with a complete file. The owner's Ford GT lap replayed headless on the rc9 package on both renderers at 3x reaches the same race, with the same lap of 1'23.394. The reference's dump numbering is one frame ahead of ours (0.016 s on the race clock).
- **Not verified:** that Tokyo R246 is listed in Arcade on the bench's card; replay in sync on the reference renderer with AI cars (IN-506 saw a different rival list between the two), so a Time Trial is the safe recording.
- **Bugs registered (TASK-139 in `TASKS.md`):** the time trial ghost drawn solid (Confirmed: the reference and a real PS2 draw it in alternating rows); the Tokyo R246 asphalt texture (owner and a player); a glitch when a race returns to the menus at the 480p to 480i switch (owner, random, not on GSdx).
- **The owner's recordings:** Tokyo R246 and El Capitan (Time Trial, Spec II), replayed headless at 3x on both renderers. The reference stays in sync with the recordings.
- **Fixed, all four found on those replays (IMPLEMENTATION-NOTES IN-528 to IN-531):**
  - IN-528, black blocks and broken textures on Tokyo R246: a CPU sprite left its blocks' identities stale, so the upload memo skipped restoring them. Upload check to VBlank 8000: 4,276 mismatches -> 0.
  - IN-529, the wrong frame as a race opens and in the fade out (signs missing, white trees), and El Capitan's grey road at VBlank 2700: GT4 clears its depth through FRAME and ZBUF at once; drawn now as a depth write.
  - IN-530, the solid time trial ghost: SCANMSK was not drawn at all; now on output rows, like GSdx.
  - IN-531, green screens and junk when a race returns to the menus: the display showed GS memory no buffer was written to.
  - IN-532, El Capitan's exit: the language menu of the game's start shown for 13 VBlanks: memory or a target that the race's draws had replaced.
- **Checks:** Tokyo VBlanks 10750-10780 against GSdx: median difference 0.00 (was green); the fade frames and frame 8000 look like GSdx; George V Paris lap: the car's rear glass now shows the interior, as on GSdx. New tests `hw_core2_depth_clear`, `hw_core2_scanmsk`, `hw_core2_display_memory`, and a CPU sprite case in `hw_core2_upload_memo`; each fails with its A/B switch.
- **v2.00:** the night race against rc9: 3 of 10 frames identical, the rest differ only on the wheels, which now spin with their blur as on GSdx (rc9 showed still spokes).
- **Installed for the owner's test (2026-11-03):** game files only, `private/slice392-dist-t139` (pins `docs/inputs/slice392-pcsx2.json` exe `27cbf602...`, `slice392-spec2.json` exe `72eea060...`; slice391 was the same without IN-532), into `C:\Program Files\GT4Recomp\game`; every file checked by hash. The rc9 game files are kept in `private/_program-backups/rc9-before-t139-20261103T025446Z`. Built by `scripts/coregs2_bench/game/use-build.sh` with slice390's EE libraries and profiles (no new training); its George V Paris frames are equal to the debug build's, 8 of 8, and the debug build's to the one before IN-532. Launcher, saves and settings untouched. The first Play compiles shaders again.
- **CTest on the debug build:** 291 of 292, `python_native_contracts` the known failure (its GS settings and VU options cases).
- **With the owner's texture pack** (`data\Textures`, an HD HUD and UI pack, 15,423 files): Tokyo VBlanks 7700 and 8000 show the black blocks on rc9 and none on the installed build. The player's own pack was not available.
- **Not verified:** a window run and the owner's own test; the notebook (off). Not uploaded.

## TASK-134: 1.3.0-rc9, the CPU optimisation, installed for the owner's test (2026-11-02)

Branch `task/134-game-code-optimization-study`, on top of rc8. **Not published and not uploaded, on the owner's word:** the owner has CoreGS 2.0 picture bugs to fix first. Details: `docs/GAME-CODE-OPTIMIZATION-STUDY.md` section 8, `docs/release-notes/1.3.0-rc9.md`, `docs/BUILD.md`.

- **Result, process CPU per VBlank in a player's session, recorded lap on George V Paris, Spec II, 1x, headless, interleaved:** i5-7200U 33.87 -> 27.64 ms (-18%); Ryzen 7 5700X 12.59 -> 10.14 ms (-19%). Wall 15.77 -> 14.48 and 5.88 -> 5.43.
- **What did it:** PGO and ThinLTO of the whole game with the EE library rebuilt; Spec II's VU0 programs compiled; the VU0 macro operations over four lanes; no measuring without a report.
- **Same game:** race clock and simulation state byte-identical to rc8 on the lap and on the v2.00 night race; 36 of 36 frames at 3x and 6x; VU1, VU0 and MTVU verify at 0 differences.
- **Baseline from now on (owner):** the recorded lap, `scripts/coregs2_bench/game/ab.sh`.
- **Without the performance report,** the stage times in `[core2]` read 0; F7/F8 needs that report on, as before.
- **Open:** the owner's own test on both machines; everything above is headless. A dark block on a track sign at VBlank 3750 in about 1 run in 6, on rc8 too, cause Unknown. The GS worker is now the largest thread.
- **Installed:** the owner's desktop and the notebook's portable copy (backup `_program-backups/rc8-before-rc9-20261102T235701Z`). Pins slice390; output `private/release-130-rc9-out`.
- **On hold (owner, 2026-11-02):** the CPU work stops here for now and rc9 is not finished. The owner goes back to CoreGS 2.0 for the picture: textures that break and graphic glitches. Those fixes build on this branch, and the slice390 package will need rebuilding with them.

## 1.3.0-rc7 and 1.3.0-rc8 published as pre-releases; the laptop test passed by the owner (2026-11-02)

**The owner's test on the i5-7200U / HD 620 closes the beta gate:** "2x 720p locked at 30 FPS was a really nice experience". A player on this card chooses 1x at 60 FPS or 2x at 30 FPS. Full speed at 2x and 60 FPS is no longer the gate. Sessions on the notebook, Spec II, candidate 382, table in `docs/release-notes/1.3.0-rc8.md`; every interval is the owner's own F7/F8, so the stretches differ.

- **At 2x and 60 FPS the limit in this report is the game's own frame (Confirmed), and the cause is a Hypothesis:** critical path 20.28 ms, with the VU1 program at 12.42 ms per frame against 7.07-8.03 ms at 1x on the same kind of work, GS waits for the GPU 0.91 ms. The processor seems slower when the graphics part is busy, since the two share power and memory on this chip. Not measured apart.
- **Known issue, left as it is (owner):** at 2x on the HD 620, the showroom runs at about 30 FPS with the card at 100%, because menus stay at 60 and the 30 FPS choice applies to races only.
- **The notes:** the "about half the processor of GSdx" claim is removed, since nothing measured it for the whole process. Discord Rich Presence, on by default since rc7, is now named in the rc7 notes. The owner tested it with the real Discord client: it works (2026-11-02).
- **Later (owner), no work now:** an experimental unlocked frame rate up to 60 that skips frames when it falls (`docs/TASK-082-UNLOCKED-FPS-STATE.md`).
- **Published on the owner's word:**
  - `v1.3.0-rc7` at `c21e934c`, launcher rebuilt from that commit with the game of `private/slice337-dist-130-rc7` (pins 335 and 336), output `private/release-130-rc7-out`: installer `94cffac0...`, zip `b1f8ca47...`.
  - `v1.3.0-rc8` at `31753907`, candidate 382 with the final notes, output `private/release-130-rc8w-out`: installer `189ab428...`, zip `73deb5e9...`. It replaces the old draft of candidate 354. The owner's desktop and notebook have this launcher's previous build, which differs only in the notes.
  - `v1.2.5` stays Latest.
- **Checks before each:** installer and zip against their sources by hash; locale keys and placeholders equal; `dev/focus-check.html` in both languages with no failure (rc7 215 and 222 passed; rc8 258 and 265); launcher `cargo test`. On rc7, a clean checkout fails `gs_golden` and `launch_contract` because `tests/fixtures/gs` is stored with LF and the tests compare CRLF; with those fixtures in CRLF both pass, and the file is not in the program. This branch's tests normalise both sides.
- **Not verified:** rc7 and rc8 installed from the published assets; rc7's launcher in a real session (its game files are the ones of the earlier kit); GSdx in an accented folder.
- **Merged on the owner's word:** PR moesuito/gt4-staticrecomp#180 into `task/134-game-code-optimization-study` (`d1f9e765`).
- **Next (owner):** rc9 is the CPU optimisation (TASK-134), on this branch.

## TASK-138: a portable folder carries its image, accented paths, the GSdx fallback; candidate 382 (2026-11-02)

Branch `task/138-relative-image-link`, on top of TASK-137. Decision 0075. Not merged, not uploaded. Pins `docs/inputs/slice382-pcsx2.json`, `slice382-spec2.json` (capsule of 376); game `private/slice382-rc8-game`, distribution `private/slice382-dist-130-rc8`, package `private/release-130-rc8u-out`.

- **Relative link:** an image inside the launcher's folder is saved relative to it, one outside by its absolute path. The folder can move with the image. A time within 2 s is not a change (exFAT and FAT round it). The `game` folder is refused for the image, because updates replace it.
- **Bugs found by testing it for real:**
  - A direct start of a release was always refused: the game knew only the build's names (`gt4vulkan.exe`), not `gt4.exe` and `gt4-spec2.exe`.
  - CoreGS 2.0 closed at once in an accented folder: `u8path` on the shader cache and textures folders, which are in the ANSI code page. Now `_wgetenv`; `test_paths_from_environment.py` guards it.
  - The GSdx path and config, and the achievements cache folder, were decoded as UTF-8 too; the message box drew bytes as characters.
- **Fallback:** when GSdx fails, the game offers CoreGS 2.0 (not CoreGS 1.1). Yes stores it, when the card has Vulkan 1.2. A direct start says where to choose it.
- **Checks:** ctest 173 passed; launcher `cargo test` passed; `dev/focus-check.html` in both languages; frames 30 of 30 equal to 381; real setup, then the folder renamed and moved, then a direct start: plain name, and `Usu\u00e1rios\\Jo\u00e3o\\Pendrive` with the shader cache and textures, on CoreGS 2.0.
- **Not verified, open:** GSdx in a portable folder started directly exits with code 2 during the handshake, with a plain name and with an accent, and the game of rc7 does the same there, so this work did not cause it. A TASK-027 agent may not read `gt4-gs` logs or the fork. So GSdx in an accented folder is **not confirmed**, and needs the owner's test through the launcher, or a PCSX2-tainted agent.
- **Installed (owner's word, 2026-11-02):** the owner's desktop through the installer's update mode and the notebook's portable copy, both candidate 382. On the desktop the owner's onboarding had set up Spec II on the earlier candidate 381: no copy of the image and no GSdx download, with CoreGS 2.0 as the renderer, and its absolute link stays valid. The notebook keeps its copies of the images and its setup.
- **Settings > Games:** names the disc image's size and location; a launcher-only change on top of candidate 382, same game executables (package `private/release-130-rc8v-out`). Installed on the owner's desktop and the notebook's portable copy (owner's word, 2026-11-02).

## TASK-137: the launcher links the disc image, candidate 381 (2026-11-02)

Branch `task/137-link-disc-image`, on top of TASK-136. Decision 0075. Not merged, not uploaded. Pins `docs/inputs/slice381-pcsx2.json`, `slice381-spec2.json` (capsule of 376); game `private/slice381-rc8-game`, distribution `private/slice381-dist-130-rc8`, package `private/release-130-rc8t-out`.

- **What changes:** setup no longer copies the image; the receipt records where it is. An image that moved or changed asks to be chosen again. The game's direct start reads the same receipt.
- **Safety:** an install with a copy keeps working with it; removing a game never touches a linked image.
- **Checks:** ctest and launcher `cargo test` passed; `dev/focus-check.html` in both languages; setup on the real 5.3 GB image took 1 min 59 s, wrote 8.1 MB and left the image untouched; frames 30 of 30 equal to 378; a session with the game, card and logs under `Usuários\\João`, with the image in an accented folder and in a Japanese one (by its short name).
- **Found by that test:** a path outside the ANSI code page reached the game as `???`, and it went on with no disc. It now goes by its short name, or setup refuses it.
- **Not verified:** the owner's onboarding with the real page, an emulator holding the image open, another code page.

**The owner's desktop was reset for the onboarding (2026-11-02), nothing deleted:** `%APPDATA%\\GT4Recomp`, `%LOCALAPPDATA%\\GT4Recomp`, `%LOCALAPPDATA%\\io.github.moesuito.gt4recomp` and `<install>\\data` were set aside as `*.before-onboarding-20261102` (`data.before-onboarding-20261102`; the saves are there). `data\\Textures` stayed in place.

## TASK-136: simple onboarding, GSdx optional (2026-11-02)

Branch `task/136-simple-onboarding`, on top of TASK-135. Launcher only, game exes unchanged (candidate 378). Not merged, not uploaded, not built into a package yet. Decision 0074.

- **New install:** setup checks the card (Vulkan 1.2, then the game starts CoreGS 2.0 with `--prepare-shaders`). It passes: CoreGS 2.0, nothing downloaded. It fails: GSdx is downloaded and the page says why.
- **Settings:** choosing GSdx when it is missing asks in a dialog, which shows the download and closes by itself; declining or a failure goes back to CoreGS 2.0.
- **No change** for a player who chose a renderer or already has GSdx. The launch-time repair no longer downloads GSdx under CoreGS 2.0.
- **Checks:** launcher `cargo test` passed, with two new flow tests and the rule's test; two ignored tests with the real check on this PC passed, CoreGS 2.0 with `gt4.exe`, GSdx with the program missing; `--prepare-shaders` from an empty cache: 15 s here, 3 min 40 s on the notebook, exit 0 on both; `dev/focus-check.html` in both languages.
- **Not verified:** a whole setup with a real disc through the page, a real card refused by the renderer (the fallback was exercised with the program missing), a PC without Vulkan.

## TASK-135: performance HUD on CoreGS 2.0, candidate 378 (2026-11-02)

Branch `task/135-coregs2-performance-hud`, on top of `task/082-coregs2-frame-skip`. Not merged, not uploaded. Candidate 378 is 377 with the launcher's icon on both game executables (owner): 10 of 10 icon images in each, 30 of 30 frames equal to 377. Pins `docs/inputs/slice378-pcsx2.json`, `slice378-spec2.json` (capsule of 376); game `private/slice378-rc8-game`, distribution `private/slice378-dist-130-rc8`, portable zip in `private/release-130-rc8r-out` (`bb4538cc...`). No installer was built.

**Where it is:** the owner's notebook (portable copy, 25 program files replaced over SMB, each checked by hash; 639 files of `user` and `data` unchanged). It was not started there. The owner's desktop is still candidate 376, without the option selector and without the HUD.

- **The option:** Settings > Graphics > Advanced > Performance HUD, under CoreGS 2.0 only, off by default. A panel in the top right corner of the window: graphics card, output size, render scale, refresh rate, FPS, frame time, CPU and GPU of the game, memory, video memory, and a live frame time graph.
- **Cost:** none measurable on the graphics thread; 26.6 us per presentation by its own counter (the ledger has the table).
- **Checks:** ctest 168 passed, one skipped, with `hw_hud`; launcher `cargo test` passed; `dev/focus-check.html` in both languages; frames 30 of 30 equal to 376 and equal with it on; validation layer on the own-queue, one-queue and 30 FPS paths adds no message over a run with it off; a window resize works.
- **Not verified:** the owner's session, fullscreen at the owner's resolution, the laptop, other graphics cards (the GPU and video memory lines read counters of Windows, and show dashes where those are missing).
- **Known:** the panel covers what is behind it at 63% opacity (the owner did not ask to change it). The GSdx overlay is unchanged: changing it is fork work, for a PCSX2-tainted agent.

## TASK-082: the 30 FPS mode, candidate 376; unlocked FPS paused (2026-11-02)

**The owner's decision:** any framerate needs interpolation to be smooth, so for now the option is a fixed **30 FPS mode**. The study of unlocked FPS paused and is saved in `docs/TASK-082-UNLOCKED-FPS-STATE.md`. The owner goes on with CPU optimisation with another agent. This replaces the Skip frames when slow of candidate 375, which was never released.

Candidate 376 (pins `docs/inputs/slice376-pcsx2.json`, `slice376-spec2.json`, capsule `slice376-frame-render-license-guard-runtime`), installed on the owner's desktop through the installer's update mode (output `private/release-130-rc8q-out`, installer `fc920e62...`, portable zip `76593a0b...`). **Not uploaded.** Branch `task/082-coregs2-frame-skip`, on top of `task/027-coregs2`, not merged.

- **The option:** Settings > Graphics > Frame rate (experimental), 60 or 30 FPS, under CoreGS 2.0 only, 60 by default. In races only; menus stay at 60, because both ways of doing it in menus failed (`docs/TASK-082-UNLOCKED-FPS-STATE.md`).
- **Numbers (the ledger has the table):** 12x: 100% speed at 30 frames per second, intervals 33.0 to 33.7 ms, against 77% speed at 46 without it.
- **Checks:** ctest 167 passed, one skipped; `test_frame_sheet.py` passed; launcher `cargo test` passed; locale keys equal. Frames with it off: 77 of 77 equal to 375.
- **Not verified:** the owner's session with the mode, the laptop, a one-queue device, split screen, a real 60 Hz monitor.

## TASK-082 frame skipping on CoreGS 2.0, candidate 375 (2026-11-02)

The owner asked for frame skipping after the optimisations, with a frame by frame sheet, because the first attempt (Gemini, `task/082-render-skip`, PR moesuito/gt4-staticrecomp#113) showed old frames between new ones. Branch `task/082-coregs2-frame-skip`, on top of `task/027-coregs2`. Candidate 375 (pins `docs/inputs/slice375-pcsx2.json`, `slice375-spec2.json`, capsule `slice375-frame-skip-license-guard-runtime`), installed on the owner's desktop, **not uploaded**.

- **New option:** Settings > Graphics > Skip frames when slow, under CoreGS 2.0 only, **off by default**.
- **The stale image is explained and gone (IN-524):** Confirmed by the sheet: on both games, the frame shown is always the last one drawn. The old report's hypothesis had the right flip and the wrong timing: a drawing list runs a frame after it is built. Its proposed fix (undo the toggle on skipped frames) would have shown each frame a frame later: the first version here did that, and the sheet showed it (age 1 and 2 where 0 and 1 are possible).
- **Physics:** race clock equal at every VBlank, state after a whole lap equal except for 22 wheel floats on Spec II, 0 words on v2.00.
- **Numbers (the ledger has the table):** George V Paris at 12x on the RX 9060 XT: 100.02% speed at 38.5 frames per second, against 76.85% at 46.1 without it.
- **Spec II works too:** the first attempt was v2.00 only.
- **Pause and leaving a race:** checked frame by frame, 383 of 383 and 483 of 483 frames ok; frames are left out again a second after the race goes on.
- **Off, nothing changes:** 108 of 108 frames equal to candidate 374.
- **Decisions of mine, the owner can change:** off by default; floor of 20 frames per second; it engages at 33 ms behind.
- **Tests:** ctest 166 passed, one skipped, with `native_frame_skip`; `test_frame_sheet.py` 8 passed; launcher `cargo test` passed; locale keys equal.
- **Known:** the speed number on the race display reads up to about 3 mph differently while skipping (smoothed per frame drawn). 480p only, which the launcher always uses.
- **Not verified:** the owner's own session, the laptop (the reason for this feature), split screen, a long race with pit stops, a 60 Hz display.
- **An optimisation note for later:** 30.7% of a race's area at 12x is constant-colour blended layers (IN-523).

## TASK-027 CoreGS 2.0 candidate 374: input lag and the EE's wait (2026-11-02)

Candidate 374 (pins `docs/inputs/slice374-pcsx2.json`, `36044156...`, and `docs/inputs/slice374-spec2.json`, `647d7585...`), installed on the owner's desktop through the installer's update mode. Output `private/release-130-rc8l-out`: installer `14d4cf5c...`, portable zip `f20efb45...`. **Not uploaded.**

- **Input lag (IN-521):** presentation to display on the 60 Hz virtual display, whole lap of the owner's recording at 6x: 53.0 ms on 373's behaviour, 31 to 39.5 ms now, depending on the run (the phase of the presentation against the flip). The owner considers 30-35 ms the normal range for 60 Hz with vsync and finds this great against the 50 ms before. Input to presentation is not in these figures.
- **The waits (IN-522):** the EE no longer waits for the flare table: 9.5 ms per VBlank to 0.00 at 6x. The flares stay exact: this replaces the late flares of IN-514, and the owner's decision on them is no longer needed.
- **A bug of mine in candidate 373, fixed:** from IN-514 on, with a window, Performance Mode went through the read with a limit before its own answer at once. So on 373 it waited for the read and took the real table. Confirmed by reading the code; the effect was not measured apart, and the owner's Performance Mode reports on 373 include it. It may be why the GS looked slow in that mode.
- **D1 (IN-523): measured, not built.** The attachment path that exists is worse on this GPU at 12x (GPU busy 19.73 to 21.37 ms). 43% of a race's area has no depth test, 26% tests with alpha test and a depth write, where an early write is wrong, and 18% could reject early, where the shader already rejects before texturing. High confidence the gain is below 10% of GPU time, not worth the rewrite now. The owner can overrule. At 12x normal mode does not hold 60 FPS on George V Paris (presented mean 21.3 ms, GPU busy 19.73): the larger target is the 30.7% of area in constant-colour blended layers.
- **Frames:** against 373, headless: 122 of 122 identical on the PGO build before the last change (night 1x and 3x and 12x, Arcade 3x, George V Paris 6x in both modes, Spec II night 3x), and 47 of 47 after it (Arcade 3x, George V Paris 6x). The lap at 3x, EE waiting against not waiting: 18 of 20 identical, with the same 2 frames differing between two runs of the same mode; EE cycles equal.
- **Checks:** `ctest -R "^hw_|native_ipu_fast|standalone|native_pacing|native_input|native_mtvu"` 162 passed, one skipped; after the last change `hw_present_latency` and `native_pacing` again; launcher `cargo test --workspace` passed. Locale files unchanged since 373. Launcher notes changed in both languages with the same bullets.
- **Not verified:**
  - The flip edge step (`note_flip_miss`) has only its fixture: it was written after one lap of the final build showed 12 pairs of a late frame and a frame right behind it, and the three laps after it never landed there.
  - The owner's own session, a real 60 Hz monitor, the laptop.

**To reproduce:** `scripts/coregs2_bench/lap-run.sh <run> [ENV=VAL ...]`. A/B switches: `GT4_HW_LATENCY_CONTROL=0`, `GT4_HW_FLARE_DEFER=0`.

## TASK-027 CoreGS 2.0: owner's direction after candidate 373, open items and the input recorder (2026-11-01)

**The owner's view and decisions:**
- CoreGS 2.0 is already better than GSdx in the owner's opinion: GSdx without Performance Mode loses frame rate on George V Paris at 6x, and CoreGS 2.0 does not.
- **The darker road in Performance Mode stays:** it is not a problem.
- **The next two optimisations, both inside CoreGS 2.0:** hardware depth (D1), and reducing the waits. Waits are what to reduce from now on. Other optimisations stay for later.
- First, the input recorder below, to make the tests easier.

**Issue, improved in candidate 374 above (owner, 2026-11-01): the input lag.** The owner finds it very bad on candidate 373, about 50 ms by their estimate over a remote link that alone adds about 10 ms, and says it was not like this before. Measured, from `vkQueuePresentKHR` to the display taking the frame, at 60 Hz on the virtual display: 3 images and the second queue (373) 51-55 ms, and 42 ms in the replay of the owner's drive; 3 images, old path 49-51 ms; 2 images, old path 35-39 ms; 2 images, second queue 44 ms. At 144 Hz 13-20 ms. Input to presentation, at least one more frame, is not in these figures. High confidence that IN-511 (three images, candidate 372) added about a refresh and IN-516 2-6 ms; nothing was measured on 371, which predates the log. The same build gave 42 and 51-55 ms depending on what came before it (fast menus or not), which points to how full the display queue stays. A fix must not bring back the stall IN-516 removed.

**Open items, none started:**
- **Before publishing rc8:** the owner's own session on 373 at 60 Hz (showroom, a car ahead, a night race); a real monitor at 60 Hz; the laptop (still on 367, the beta gate is the HD 620 at 2x); the note's "about half the processor of GSdx", which needs a GSdx F8 to compare; en-US focus pages varying with unchanged code; the installer against its sources; replacing the old rc8 draft (candidate 354). Publishing and the PR into `task/134` need the owner's word, and TASK-134's P0/P1 wait for the owner's "CoreGS 2.0 build done".
- **Decided (owner, 2026-11-02):** the colour filter stays in Performance Mode too. IN-505's cut is not built: -4.4% of GPU work on the HD 620 at 2x by day, against a change to the game's whole look.
- **For 2.0.0:** fractional scales (requirement, large refactor); GPU use and power measured beside GSdx.
- **Optimisation, later unless named above:** the race start's camera cut (about 30 ms, once per race).
- **Unknown:** a few pixels (1-30, at most 88 of 255, at the right edge of the picture) differ at VBlanks 21300 and 21400 between two runs of the same exe and the same `--press` list at 3x on George V Paris. The 6x gates did not show it.
- **Out of scope for now:** GSdx stutter (a PCSX2-tainted task).

**IN-520, the input recorder:** `GT4_INPUT_RECORD=<file>` writes the pad state the game receives, one line per change, keyed by guest VBlank, so it does not depend on the key map or the frame rate. F9 ends it and closes the game; Alt+F4 also leaves a complete file. `GT4_INPUT_REPLAY=<file>` gives the game those states, with or without a window. `GT4_PACING_FROM_VBLANK=<n>` does not wait before that VBlank. `scripts/coregs2_bench/record-input.sh` and `replay-run.sh`. New CTest `native_input_recording`.
- **Verified:** the old George V Paris `--press` list converted to a recording, 3x, headless: 18 of 20 frames identical to the `--press` run, and the other 2 differ as two `--press` runs differ between them. F9: file complete, game closed, exit 0. Window replay with fast menus: 1 min 27 s against about 6 min.
- **A bug found in testing:** with the fast menus the pacer's grid ran ahead of the clock, and the first paced frame slept for about 4 minutes, which Windows showed as "not responding" on the owner's screen. Fixed: the grid is re-anchored while not waiting. It never reached an installed build.
- **The owner's recording `george-volta2`:** a time trial on George V Paris, 8,032 VBlanks. Replayed headless at 3x in 65 s: a whole lap of 1'23.394, on the track, the ghost ahead on lap 2. Two headless runs: 13 of 14 frames identical; frame 4500 differs in 876 pixels (at most 176 of 255) at the right edge of the picture, the Unknown noise above. Not yet compared with the lap time the owner saw.
- **`scripts/coregs2_bench/lap-run.sh <run> [ENV=VAL ...]`:** menus without waiting, the drive in real time, graph and figures: 1 min 50 s. First run, normal mode, 6x, 16:9, 60 Hz virtual display, 5,991 VBlanks: sent p99 17.2 ms, longest 21.7 ms, 4 over 20 ms; shown p99 18.1, longest 18.9 ms; late flares 108 in the session.

## TASK-027 CoreGS 2.0 candidate 373: frame pacing, from the owner's five F8 reports on 372 (2026-11-01)

Candidate 373 (pins `docs/inputs/slice373-pcsx2.json`, `35a11ca7...`, and `docs/inputs/slice373-spec2.json`, `abe87b24...`), installed on the owner's desktop through the installer's update mode. Output `private/release-130-rc8k-out` in the main checkout: installer `c21f0f85...`, portable zip `bbbaf160...`. Installed game folder (20 files) and launcher equal the build. Of 16,446 files of settings, saves, replays, disc copies and textures one changed: `config/game.ini`, which the launcher reopened by the installer rewrites with the active monitor (`DISPLAY1`, no stream connected). Not uploaded.

- **The owner's report on 372:** showroom frametime strange only in the first opening, in Performance Mode; normal mode 100% there. A one-off drop to about 50 FPS at the end of George V Paris's main straight, only with a car ahead (about 58 with a time trial ghost); no drop without one. Small spikes in the graph that cannot be felt.
- **Findings:**
  - The owner plays on a 1920 x 1080, 60 Hz virtual display (a game stream's). It exists only while a client is connected; when the owner left, the runs moved to the real 3440 x 1440, 144 Hz monitor at 21.5:9.
  - **The refresh rate, not the virtual display (High confidence):** the same virtual display at 144 Hz, 16:9, old path, 8 slots: showroom GPU waits 0.79 ms, against 10.96 ms at 60 Hz. A real monitor at 60 Hz was not tested. **At 60 Hz (the owner's logs and ours):** with vsync the GS thread waits about 11 ms per VBlank in the showroom, and one or two whole refreshes (17.7, 33 ms) in races. A real monitor and a plain window show none of it, and neither does no vsync.
  - **The drop:** without Performance Mode the EE waits for the GS at the flare table every frame. Frames of GS 17 ms make wall 20-22 ms; Performance Mode does not wait and held 60 FPS.
- **What it adds:** IN-514 the flare read with a limit (normal mode, window only); IN-516 the presentation on its own queue; IN-517 16 submission slots; IN-518 no block walks for targets that need no readback; IN-519 no bursts when the pacer catches up; a fresh PGO profile; IN-515 measured and left off; `frametime.csv` in every CoreGS 2.0 session and `scripts/coregs2_bench/frametime_plot.py`.
- **Measured, window, Spec II George V Paris at 6x, 16:9, real monitor, 372 against 373, one run each, 3,700 race VBlanks:** normal mode frames over 17.5 ms 40 to 5, longest 19.6 to 18.8 ms; Performance Mode longest 17.5 to 17.6 ms. **GS work per VBlank, all stages with the flare read:** normal 11.00 to 10.73 ms, of which CPU (without GPU waits) 7.94 to 7.31 ms, -8%; Performance 8.70 to 7.72 ms, CPU 7.78 to 7.24 ms, -7%. The `gs` figure of the spike lines, 9.42 to 6.79 ms, leaves out the flare read: with 16 slots the wait moved from inside the stream to that read (1.43 to 3.80 ms), and the EE waits the same (8.55 to 8.58 ms). **So the flatter frametime in normal mode comes from IN-514, not from a faster GS.** Measured before IN-519, which does not touch these frames' pacing unless the game is behind.
- **The cost of IN-514:** late flare answers on about 8% of the frames of that race (291 of 3,700; 8% to 32% across runs, 1,681 at 21.5:9). In those frames a flare is one or two frames late. **The owner decides whether normal mode keeps it;** `GT4_HW_FLARE_DEADLINE_MS=0` removes it.
- **Frames against 372, headless:** 154 of 154 identical: 1x 37/37, 3x 10/10, 12x 31/31, Arcade 27/27, Spec II George V Paris at 6x 20/20 in each mode, C.S. Route 5 4/4, Costa di Amalfi 5/5. Taken before IN-519 and the teardown fix, which change no pixel path. Headless runs always wait for the flare table, so they do not exercise a late answer.
- **Checks:** `ctest -R "^hw_|native_ipu_fast|standalone|native_pacing"` 154 passed, one skipped, with the new `hw_flare_table`. `hw_core2_gpu_clut`, last night's unexplained single failure, is explained and fixed: the fixture makes dozens of renderers on one device and each kept its rings and descriptor pools, so memory ran out; they now go with the renderer. Launcher `cargo test --workspace` passed. Window resize and minimise: 8 swapchains in one run, clean exit.
- **Measured at 60 Hz on the virtual display, candidate 373's build (the ledger has the table):** the old path with 8 slots reproduces the owner's stall (showroom waits 11.42 ms, the jagged RivaTuner graph); yet the frames reached the display evenly, so that graph was not visible on screen, as the owner said. 16 slots remove most of it (0.47 to 1.61 ms), with 2 to 4 presentation spikes of 25-33 ms per run. The second queue removes the rest: waits 0.01 ms and no spike in two runs, longest 18.2 ms, at 2-6 ms more latency and a few shown intervals of 18-20 ms. **It stays the default.** Late flares here: 0.5-1.4% of the race. A frame takes about 50 ms to reach this display, against 13-20 ms at 144 Hz; the two-image and latest-ready options did not solve it (Unknown how much of it is the display's own path).
- **Not verified:** a real monitor at 60 Hz, the owner's own session on 373, the car ahead on the owner's save. The car ahead on the owner's save (the bench's heaviest stretches, up to 48 submissions per VBlank, stand in for it). The laptop (off). Focus pages (see 372).
- **Left:** the v2.00 Arcade race start has about 1 s where the GPU needs 15-18 ms at its low clock and the GS thread waits for it at the flare read (window: presented p99 19.9 ms, longest 32.5 ms at the camera cut). Next step: the GS thread does not wait at the flare read (the reduction goes through `ReduceLater`, and the EE waits on its fence with the same limit).

## TASK-027 CoreGS 2.0 candidate 372: final 1.3.0-rc8 of the night, race start, vsync and showroom (2026-11-01)

Candidate 372 (pins `docs/inputs/slice372-pcsx2.json`, `ec4d3bd8...`, and `docs/inputs/slice372-spec2.json`, `7ccf3a68...`), installed on the owner's desktop through the installer's update mode (`private/release-130-rc8j-out`, installer `5115b8bd...`, zip `45ab8ba9...`). **Not uploaded: the owner said no GitHub yet.** **The owner's F8 on this build is pending.**

- **The owner's F8 on 371:** "sensational", normal mode nearly at full speed on George V Paris, with a pointed drop as the cars roll to the line, in both modes and not always, and an unstable frametime in the showroom only, worse on some cars.
- **What it adds, none of it cuts an effect, all in normal mode:** IN-510 GLM writes left out when nothing changed (the race start drop); IN-511 three swapchain images under vsync; IN-512 small writes over drawn pages merged on the GPU (the showroom wave); IN-513 decodes over targets reused inside a VBlank.
- **Measured in a window against 371:** race on George V Paris without Performance Mode, VBlanks over 20 ms 29 to 1 of 1000 and mean 18.13 to 16.79 ms; showroom GS thread 6.1 / 9.9 to 6.0 / 6.0 ms. Table in `docs/hw-renderer-spec/CORE2-PERF-LEDGER.md`.
- **Frames against 371:** 1x 37 of 37 identical; 3x 10 of 10, Arcade 27 of 27; Spec II George V Paris at 6x 20 of 20 twice, C.S. Route 5 4 of 4, Costa di Amalfi 5 of 5. **At 12x 30 of 31: frame 11000 differs in 2 pixels of 33 million, by at most 8 of 255.** It is deterministic and comes from IN-512, which keeps scaled pixels the whole-row patch used to replace at 1x. Named relaxation of the 12x gate; the 1x gate holds.
- **Checks:** `ctest -R "^hw_|native_ipu_fast|standalone"` passed, one skipped, with the new `hw_core2_glm_memo`, `hw_core2_patch_bits` and `hw_core2_over_target_memo`; the first two and the earlier page and grace tests also pass without the interlock. `hw_core2_gpu_clut` failed once under `-j 4` with an intermediate build and passed alone and in every later full run: not explained. Launcher `cargo test --workspace` 599 passed. Locale files: 626 keys each, same placeholders; launcher notes have the same bullets in both languages. Installed game folder equal to the distribution; 16,441 files of settings, saves, replays, disc copies and textures unchanged.
- **Focus pages, a pending check before any upload:** pt-BR 218 of 230, the same 12 known failures. en-US gave 210 of 223, 203 of 222 and 196 of 222 in three runs in a row with the same files, against 211 of 223 on candidate 369. The launcher's code and locales are unchanged since 369, so this build did not cause it, but the cause is Unknown.
- **Not fixed:** the race start's camera cut (27 to 33 readbacks, about 42 ms, once). The owner's heaviest showroom cars and the per-VBlank store they showed at the rolling start (`upload=14`, `store 1`) were not reproduced in the bench, so only the F8 can say they are gone. Three images can add up to one frame of latency under vsync. The PGO profile was not regenerated.

## TASK-027 CoreGS 2.0 candidate 371: the frametime wave on George V Paris removed (2026-11-01)

Candidate 371 (pins `docs/inputs/slice371-pcsx2.json`, `0c509aa3...`, and `docs/inputs/slice371-spec2.json`, `011ecb9f...`), installed on the owner's desktop through the installer's update mode (`private/release-130-rc8i-out`, installer `8ce07302...`, zip `ace931e9...`). **Not uploaded until the owner says so.** **The owner's F8 on George V Paris at 6x, with and without Performance Mode, is pending.**

- **The owner's report on candidate 370:** much faster (59.20 and 59.45 VBlanks/s, from 49.60 and 57.73) but a bad frame pacing in both modes: at 60 FPS the frametime looks like an audio waveform, 10-12 ms beside more than 20 ms. The owner says it was there before 370.
- **Cause (Confirmed, reproduced headless):** IN-509: GT4 draws a 256 x 256 Z target at BP 0 every second VBlank. One VBlank of grace for a target of another width dropped it each time and kept its memory: six readbacks every second VBlank, four of 640 x 448 targets. In the owner's windowed session that readback waited 7 to 11 ms for the GPU.
- **What it adds:** two VBlanks of grace. It cuts no effect, so it is in normal mode. `GT4_CORE2_WIDTH_GRACE_LONG=0` is the A/B switch.
- **Measured (headless race, three interleaved rounds):** change from one VBlank to the next 4.38 to 0.44 ms without Performance Mode and 3.87 to 0.40 with it; wall p95 21.87 to 15.13 and 17.35 to 12.75. Table in `docs/hw-renderer-spec/CORE2-PERF-LEDGER.md`.
- **Frames against candidate 370:** 117 of 117 identical: v2.00 10/10 at 3x, 27/27 Arcade, 31/31 at 12x; Spec II George V Paris at 6x 20/20 and 20/20, C.S. Route 5 4/4 and Costa di Amalfi 5/5 at 3x.
- **Checks:** `ctest -R "^hw_|native_ipu_fast|standalone"` 149 of 149 passed (one skipped), with the new `hw_core2_width_grace`. Installed game folder equal to the distribution; 16,427 files of settings, saves, replays, disc copies and textures unchanged.
- **Not fixed, seen in the owner's sessions:** the race start's camera cut (27 readbacks, about 42 ms, once; IN-429 reduced it before), and rare GPU waits of 26 to 30 ms inside a draw with no readback. In the window, GPU waits outside readbacks run at 0.5 to 7 ms per VBlank against about 0.03 to 0.5 headless: cause Unknown; Hypothesis: the swapchain's vsync. A headless run cannot show it, so the owner's F8 decides.
- **Not run on this build:** a window, the launcher's `cargo test` and focus pages (its code and locales are unchanged since 369; only its notes changed), the laptop (off), the full installer comparison required before any upload.

## TASK-027 CoreGS 2.0 candidate 370: the readback before a write on George V Paris removed (2026-11-01)

Candidate 370 (pins `docs/inputs/slice370-pcsx2.json`, `361d3310...`, and `docs/inputs/slice370-spec2.json`, `a2262c2e...`), installed on the owner's desktop through the installer's update mode (`private/release-130-rc8h-out`, installer `74c8a0a6...`, zip `66221cde...`). **Not uploaded until the owner says so**; the GitHub draft still holds candidate 354. **The owner's F8 on George V Paris at 6x, with and without Performance Mode, is pending.**

- **What it adds:** IN-508: a write that shares no page with a target's drawn rectangle neither stores nor patches that rectangle. It cuts no effect, so it is in normal mode. `GT4_CORE2_DRAWN_PAGE_TEST=0` is the A/B switch.
- **Frames against candidate 369:** 78 of 78 identical: v2.00 10/10 at 3x, 14/14 Arcade, 31/31 at 12x; Spec II George V Paris at 6x 7/7 without and 7/7 with Performance Mode, C.S. Route 5 4/4 and Costa di Amalfi 5/5 at 3x.
- **Measured (headless race, three interleaved rounds):** GS thread 17.41 to 14.04 ms per VBlank without Performance Mode (-19%), 14.95 to 12.25 with it (-18%); GPU waits 4.86 to 2.08 and 2.44 to 0.39; frame median 20.03 to 16.54 and 16.15 to 12.97. Table in `docs/hw-renderer-spec/CORE2-PERF-LEDGER.md`.
- **Checks:** `ctest -R "^hw_|native_ipu_fast|standalone"` 148 of 148 passed (one skipped), with the new `hw_core2_drawn_page_test` and new cases in `hw_core2_upload_swizzle`. With the dirty side turned off on purpose the new test fails, 9 failures. Installed game folder equal to the distribution; 16,424 files of settings, saves, replays, disc copies and textures unchanged.
- **Not run on this build:** a window, the launcher's `cargo test` and focus pages (the launcher's code and locales are unchanged since 369; only its notes changed), the laptop (off), the full installer comparison required before any upload.

## TASK-027 CoreGS 2.0 candidate 369: shaders compiled before Play (2026-11-01)

Candidate 369 (pins `docs/inputs/slice369-pcsx2.json`, `1aa79cf0...`, and `docs/inputs/slice369-spec2.json`, `d3380f68...`), installed on the owner's desktop through the installer's update mode (`private/release-130-rc8g-out`, installer `59dd9a42...`, zip `372f141d...`). **Not uploaded until the owner says so.**

- **What it adds:** IN-507: under CoreGS 2.0 Play checks the shader cache and shows "Compiling shaders" only when it must compile. GSdx is unchanged.
- **Checks:** v2.00 frames against candidate 367 10/10 and 14/14; `ctest` 147 of 147 passed (one skipped); launcher `cargo test --workspace` 599; focus pages en-US 211 of 223 and pt-BR 218 of 230, the same 12 known failures, the 6 new checks pass; a run with the prepared cache had no uber batches.
- **Real compile:** the owner's first Play on 369 compiled 300 variants through the launcher (`shaders: compiled`, 00:59); the next session found them. **Not checked:** the HD 620 (laptop off).
- **Against GSdx at 6x on George V Paris:** GSdx 57.50 VBlanks/s, 60.00 in Performance Mode (no late frame in 5871), against 49.60 and 57.73 here; p99 19.64 ms against 27.74 ms in Performance Mode. Table in the journal of 2026-11-01. GSdx wins this scene.
- **Owner's F8, George V Paris at 6x on Spec II, RX 9060 XT (candidate 368):** without Performance Mode 49.6 VBlanks/s, median 19.9 ms, p99 28.9 ms; with it 57.7 VBlanks/s, median 16.8 ms, p99 27.7 ms. GS thread 17.1 and 16.0 ms per VBlank, GPU busy 12.2 and 10.4 ms: the GS thread is the limit, not the GPU. Readbacks are the largest cost, 7.7 and 7.2 ms, of which 5.5 and 5.0 ms wait for the GPU; "before write" is the largest reason (3.0 and 3.6 ms). The worst spike was 40.2 ms, 24.7 of them waiting for the GPU. Shaders did not limit these intervals.

## TASK-027 CoreGS 2.0 candidate 368: Spec II too, installed on the owner's desktop (2026-11-01)

Candidate 368 (pins `docs/inputs/slice368-pcsx2.json`, `4ac5b8a8...`, and `docs/inputs/slice368-spec2.json`, `bfa80aa4...`), the current 1.3.0-rc8, is installed in the owner's `C:\Program Files\GT4Recomp` through the installer's update mode (`private/release-130-rc8f-out`, installer `22013da9...`, zip `d197f05a...`). Saves, replays and renderer data are unchanged; only `setup.log` changed. **Not uploaded until the owner says so**; the GitHub draft still holds candidate 354.

- **What changed:** CoreGS 2.0 is no longer limited to GT4 (IN-506). v2.00's program differs from candidate 367 only in `standalone.cpp`.
- **Checks:** Spec II headless at 3x, C.S. Route 5 and Costa di Amalfi in both Performance Mode settings, two night runs 22/22 identical, 2x and 12x ran, beside GSdx at 3x by eye. `standalone_fixture`, `spec2_hooks`, `game_target`, `spec2_iop`, launcher `launch_contract` 22 and `multi_game` 5 pass; locale keys equal.
- **Not run (the owner cancelled the tests and tests by hand):** George V Paris at 4x and 6x with and without Performance Mode (it ran at 2x and 3x); candidate 368's v2.00 frame gates; focus pages; the full launcher suite; a window; GT Mode on Spec II.

## TASK-027 CoreGS 2.0 candidate 367: GPU work for weak graphics cards (2026-10-31)

Candidate 367 (pin `docs/inputs/slice367-pcsx2.json`), the current game of 1.3.0-rc8, is on the owner's laptop only, **not uploaded until the owner says so**. Against candidate 362 on the HD 620 at 2x, no window, shaders warm, GPU busy per VBlank: day 25.84 to 18.85 ms in normal mode (-27%), 25.74 to 16.75 in Performance Mode (-35%); night 32.9 to 25.4 (-23%), 33.0 to 22.7 (-31%). RX 9060 XT, 12x night: 20.7 to 19.1 ms (-8%). No owner test of this build yet.

- **What it is:** IN-485 to IN-503: palette and upload memos, shader cache between sessions, video decoder rewrite, whole clears, depth and alpha fail before the interlock, no interlock for large disjoint batches. Results and dropped steps: `docs/hw-renderer-spec/CORE2-GPU-PLAN.md`.
- **Performance Mode now also cuts:** the road's sheen and the alpha fills that serve it (IN-501, IN-502), and small effect targets stay at 1x (IN-497). The road is darker.
- **Checks:** frames 10/10 at 3x, 14/14 Arcade, 31/31 at 12x, 28/28 video, and 10/10 and 14/14 on the integrated GPU path on the desktop; 146 renderer tests (one skipped); launcher `cargo test --workspace` 598 passed. Language keys and placeholders match in both locales. Focus pages in the browser pane: en-US 205 of 217, pt-BR 212 of 224, the same 12 failures in both, the 11 tab-highlight timing checks and the focus check known from rc8 in this pane. The release game ran with a window on the desktop, with the launcher's variables and the integrated GPU path forced, in both modes at 2x: exit 0, 16.67 ms per VBlank, the sheen cut and the early variant firing. Not tested: a window on the laptop and the launcher's own window: the owner's test decides.
- **Open:** fractional scales (G3), a launcher screen that prepares shaders with a shipped first list, frame skipping, D1 (real depth for opaque draws, the largest cost left: 7.7 of 19 ms at 2x by day).

## TASK-027 CoreGS 2.0 in the launcher as a beta, 1.3.0-rc8 (2026-10-30, evening)

Candidate 362 (pin `docs/inputs/slice362-pcsx2.json`), the current game of 1.3.0-rc8, on the owner's laptop only; the owner measured 59.3 VBlanks/s at 1x on a day track there: **not uploaded until the owner says so** (their word, 2026-10-30). The GitHub **draft** v1.3.0-rc8 still holds candidate 354, which fails to start on GPUs without mailbox when vsync is Fast (IN-475). Renderer choice CoreGS 2.0 (beta), GT4 only, GSdx stays the default; the renderer defines the graphics options (IN-474). Texture replacement (IN-470), GPUs without pixel interlock (IN-471: GPU 49.05 -> 6.85 ms at 3x), PGO in the release game (IN-473: GS worker -13%). Hardware raster path measured and left out, pending the laptop test (IN-472). Frames identical to the references (10/10, 14/14, 31/31). CTest 269/270 (the known GS settings contract failure from the rc7 merge), `cargo test --workspace` clean. Waiting for: the owner's test on the laptop; the PR into the game-code branch comes after their approval.

## TASK-027 CoreGS 2.0 candidate 347: CPU work of this side finished, handed over (2026-10-30, afternoon)

`build\hwr-play-scale.ps1` points at `build\candidates\coregs2-candidate-347` (commit 68af02f, PGO and ThinLTO of the GS libraries): 337 + IN-464 (GLM hazards by page: full barriers 579 -> 276 per race VBlank), IN-465 (statistics query opt-in), IN-466 (disjoint primitives skip the per-primitive wait), IN-467 (strip runs), IN-468 (PGO, ThinLTO), IN-469 (VU1 output scan). Against 313, 3x, interleaved: process CPU 22.0 -> 13.8 ms per VBlank (-37%), GS worker -35%, VU1 worker -23%, wall -26%, frame p50 10.9 -> 7.75 ms (-29%), p95 13.7 -> 10.6 ms. Against 332: process CPU -5%, GS worker -13%, frame p50 -7.5%. Frames identical to 337 at 3x (10/10) and 12x (31/31), and in Arcade to 345; the only difference to 332 is IN-463 (3 pixels at Arcade VBlank 9100, where the race is replaced by the copy). CTest 267/268 (the known `python_native_contracts` GS settings failure), 137/137 `hw_` tests without the interlock. Not play tested yet. Not wired into the release build: `build-release.ps1` does not pass the PGO options. Measured for later: without the pixel interlock (most mobile GPUs) the 3x race needs about 44 ms of GPU against 3.9 ms; the owner set mobile aside for now. Handover for the game-code agent: `docs/hw-renderer-spec/CORE2-HANDOVER-TASK134.md`; bench tools: `scripts/coregs2_bench/`.

## TASK-027 CoreGS 2.0 candidate 337: 12x determinism fixed, direct intake by default (2026-10-30, afternoon)

`build\hwr-play-scale.ps1` points at `build\candidates\coregs2-candidate-337`: 332 + IN-462 (direct GS intake on by default after the owner's play test of 332; `-NoDirect` sets `GT4_MTVU_DIRECT=0`; frame dumps a row at a time) + IN-463 (spec 8.8: a batch whose texture is the target it draws into samples a copy of the sampled rectangle taken before it). About 35 such batches per race VBlank (glow buffers, the in-place luminance reduction, a full-screen 1:1 pass in strips) read texels other fragments of the same batch were writing, so 12x frames differed from run to run (17-21 of 31 frames, up to 15,500 pixels; full barriers did not help, every renderer counter was equal). With the copy: two 12x runs identical (31 of 31), two 3x long-race runs identical at all 10 checks (frames 6600, 7200, 9000 and 12000 included, the pixels the ledger listed as varying in every build; without the copy the same pair differs at 6600, 9000, 10200, 11400 and 12000), 3x frames equal to 332. Cost: +1.3 ms GPU at 12x (+6%, mostly the full-screen copy), one sixteenth at 3x. New CTest `hw_core2_self_texture`; CORE2 29/29, MTVU and stream sink 13/13, front end 80/80. The branch now also carries `task/134-game-code-optimization-study` (documents only).

## TASK-027 CoreGS 2.0 candidate 332: transport and CLUT fronts, direct GS intake (2026-10-30, owner asleep)

`build\hwr-play-scale.ps1` points at `build\candidates\coregs2-candidate-332` with `GT4_MTVU_DIRECT=1` (`-NoDirect`
turns it off): 326 + IN-456 (block of a pixel from the address tables), IN-457 (CSM1 CLUT loads by table, the
renderer's CLUT halfwords as ranges), IN-458 (redundant draw binds left out), IN-459 (the VU1 worker wakes the
sleeping EE only past 64 KiB of output, when idle or stopping, or with a full ring: EE wake-ups 650 -> 16 per VBlank,
process CPU -12%), IN-461 (direct GS intake: the GS worker reads the VU1 output ring itself; off by default in the
code, on in the play script: process CPU -9%, wall -2.7%). Frames identical to candidate 310 at every check (night
race; Arcade menus, showroom and race start with direct intake on and off); CTest 28/28 CORE2, 13/13 MTVU and stream
sink, front end 79/79. Measured and dropped: IN-460 (GLM patch kernel with shared words, no change at 12x) and
expanding indexed textures instead of palette rows (worse on GPU and CPU). The 12x draw passes are bound by frame and
depth traffic; big GPU gains there need CoreGS 3.0. New instruments: `[core2] batch flushes/vblank` (flushes by
reason: CLUT loads flush about 275 of the 480 batches), `notifies` in `[mtvu] doorbells`, `mtvu_*` in `[hw] transport`.
Against 313 (start of the CPU work; headless night race, interleaved): 3x process CPU 20.2 -> 14.5 ms per VBlank
(-28%), GS worker -21%, wall 10.3 -> 8.7 ms (-15%), frame p50 10.1 -> 8.4 ms, p95 12.3 -> 11.0 ms; 12x process CPU
-23%, GPU busy 20.8 -> 20.3 ms. Found: two 12x runs of the same executable differ in a 1-pixel column of one frame
(non-determinism, to be traced in its own task; every 3x check is identical).

## TASK-027 CoreGS 2.0 candidate 326: night of CPU and GPU fronts (2026-10-30, owner asleep)

`build\hwr-play-scale.ps1` points at `build\candidates\coregs2-candidate-326`: 313 + IN-446 (GS stream through a
lock-free ring), IN-447 (draw path without allocations), IN-455 (vertex queue ring), IN-450 (unchanged uploads keep page versions), IN-451
(speculative depth rejection before texturing, exact), IN-452 (CT32 uploads by whole columns, SSE2), IN-453 (texture
cache by block identity). Frames identical to candidate 310 at every check; CTest 266/267 in both interlock modes
(the known GS settings contract failure). Final comparison with 313 (headless night race, interleaved): 3x GS worker
7.19 -> 6.41 ms per VBlank (-11%), process CPU 20.2 -> 18.9 ms, EE critical path 9.69 -> 8.96 ms, frame p50 9.7 ->
8.9 ms and p95 11.9 -> 10.9 ms, GPU busy 4.16 -> 3.90 ms; 12x GPU busy 20.77 -> 20.25 ms, frame p50 24.0 -> 22.4 ms. Measured and dropped tonight:
MTVU journal slot rings (no gain), longer submission intervals (worse), textureGather (+2.6% GPU at 12x). Details in
the ledger and the journal.

## TASK-027 CoreGS 2.0 candidate 313: CPU survey done, SSE4.1 adopted (2026-10-30)

`build\hwr-play-scale.ps1` points at `build\candidates\coregs2-candidate-313-sse41`: 309 + the CPU survey
instruments (IN-445: `GT4_CPU_SAMPLER` process sampler, `[hw] transport` line, exclusive stage times) built with
SSE4.1 (`GT4_SSE41`, now ON by default; owner rule). No visual change: frames identical to SSE2 and AVX2 builds. 3x
headless night race, medians of 3: SSE2 -> SSE4.1 10.00 -> 9.62 ms per VBlank (-3.8%, the VU1 worker -16%); AVX2 no
further gain, so no AVX2 executable. The survey (`docs/hw-renderer-spec/CORE2-CPU-SURVEY.md`) lists the CPU fronts
of the GS pipeline and transport with measured costs and a proposed task order (replay bench first, zero-copy MTVU
intake last); they wait for the owner's go-ahead. CTest 266/267 in both interlock modes (the known GS settings
contract failure from the rc7 merge).

## TASK-027 CoreGS 2.0 candidate 309: constant fills as compute (2026-10-30, fine-tuning)

`build\hwr-play-scale.ps1` points at `build\candidates\coregs2-candidate-309`: 308 + IN-442, GT4's full-screen colour,
depth and alpha clears drawn by a compute shader with the uber shader's per-pixel functions (exact; storage writes from
compute run about twice as fast as from the fragment stage). GPU busy per VBlank, night race: 12x 21.6 -> 20.6 ms, 6x
8.0 -> 7.9, 4x 5.4 -> 5.2, 3x 4.3 -> 4.1; frames identical. CTest 264/265 in both interlock modes.

## TASK-027 CoreGS 2.0 candidate 308: on the 1.3.0-rc7 base, flares hidden in performance mode (2026-10-30)

Candidate 308: 1.3.0-rc7 (`task/133-rc7-ra-controls`
c21e934) is merged into the branch as its base (Spec II, rc7 launcher; CoreGS 2.0 stays out of 1.3.0). Performance
mode (`-Performance`, `GT4_CORE2_PERFORMANCE=1`) now also hides the light flares and the sun glare as rc7's
Performance mode does for the PCSX2 GS (IN-441): Clubman Stage Route 5 night at 10x 18.1 -> 16.9 ms per VBlank.
CTest 263/264 in both interlock modes. Next: the owner's fine-tuning of CoreGS 2.0, then a PR to the EE optimisation
branch.

## TASK-027 CoreGS 2.0 candidate 307: performance mode starts (2026-10-30)

`build\hwr-play-scale.ps1` points at `build\candidates\coregs2-candidate-307` in this worktree (everything moved off
the external SSD). 307 (gt4vulkan.exe sha256 a7884470...) = 306 + IN-438 (frame/depth reads only when needed, exact) + IN-439, the first performance
mode item (decision 0073, `-Performance` / `GT4_CORE2_PERFORMANCE=1`): GT4's flare-test downloads answered one VBlank
late without a GPU wait. Owner test of 306 at 10x: 43-52 FPS, GPU-bound (128-136 W). Next levers measured at 12x:
fixed-cost upkeep batching (exact, up to ~2 ms), hardware texture filtering in performance mode (up to ~1.8 ms),
full-screen fills as clears (~1 ms).

## TASK-027 CoreGS 2.0 candidate 306: specialised pipelines (2026-10-29, waiting for the owner test)

`build\hwr-play-scale.ps1` now points at `O:\gt4recomp-scratch\coregs2-candidate-306` (gt4vulkan.exe sha256
213c29b7...): candidate 305 plus IN-437. The 12x cost was the uber pixel shader re-deciding its draw-state branches in
every wave (IN-436); pipelines specialised per draw state cut the night race GPU time per VBlank by 25-36% (12x
33.3 -> 21.4 ms, 10x 23.8 -> 15.5, 8x 15.9 -> 11.2, 3x 5.6 -> 4.2) with byte-identical frames.
`GT4_CORE2_SPECIALIZE=0` is the A/B switch. Next GPU levers, measured at 12x: the remaining fragment cost against
the 0.41 ms per full-screen pass of a fully folded shader, then D1 (ROP path, 0.21 ms per pass).

## TASK-027 CoreGS 2.0 beta candidate 305 (2026-10-29, waiting for the owner test)

Branch `task/027-coregs2`, `build\hwr-play-scale.ps1` points at `O:\gt4recomp-scratch\coregs2-candidate-305`
(gt4vulkan.exe sha256 b10cefd8...). Done overnight on the owner's beta list (`decisions/coregs2-beta-scope`):
- 1x fixed (IN-427): the 3D scene shows at 1x (was HUD only).
- Mipmapping and LOD (IN-428): GS mip levels and trilinear; distant asphalt less noisy. `GT4_CORE2_MIPMAP=0` A/B.
- Pre-race camera-cut spike reduced (IN-429 batched readbacks, IN-430 merged dirty rectangles): 46 -> 40 ms once per
  race start; in the race the only readback left is the game's own download.
- Image memory budget (IN-431): texture cache 192 MiB on integrated GPUs (768 discrete); at 192 MiB the night race
  needs 507 MB of images instead of 621 MB.
- Presentation resampling to the exact window rectangle (IN-432): Lanczos-2 when shrinking (3x+ into 1080p, sharper
  than the old blit), FSR1 EASU when growing (1x, 2x); FSR1 RCAS sharpening optional (`-Sharpen 0.2`).
- Forced bilinear `GT4_HW_FILTER=forced|forced_no_sprite` (IN-433) and anisotropic filtering of mipmapped textures
  `GT4_CORE2_ANISO=2..16` (IN-434), both off by default (`-Filter`, `-Aniso`).
- 12x checked on the night race (1.6 GB of images, 27 ms of GPU per VBlank on the RX 9060 XT); frame dumps above the
  64 MB readback buffer read in bands (IN-435).
- Not in this beta (decided, see CORE2-ROADMAP "Beta closing plan"): fractional scales (integer scale everywhere, a
  large refactor; the window resampling already gives an exact output size), texture replacement (needs the
  GSdx-compatible hash and naming from the specification team), D1 hardware raster (only if the laptop's GPU is the
  limit), frame skipping (owner).
- Checks: CTest 252/253 in both interlock modes (python_native_contracts, the pin); night race 3x frames equal 301's
  except a few wheel and road-marking pixels that vary run to run (a run with both new switches off varies the same);
  title identical to 287; 1x renders; windowed run equals the forced-size headless frame.

## TASK-027 CoreGS 2.0: CORE2 rewrite and the shared scale defect (IN-401..IN-404, candidate 285)

Branch `task/027-coregs2`. Owner baseline from 2026-10-28: a 1920 x 1080 window at 3x, 16:9, no ultrawide
(`build\hwr-play-scale.ps1`, `GT4_HW_CORE=2` by default). The core was rewritten on the reference model (CORE2,
`docs/hw-renderer-spec/CORE2-SPEC.md` and `CORE2-DESIGN.md`, `native/hw_gs/core2/`): one scaled image per target, GS
memory only through readbacks, EE uploads as dirty rectangles patched on the GPU, textures decoded on the GPU from a GPU
copy of GS memory, GT4's channel-shuffle tone as one pass (IN-401). Performance pass (IN-402): double-buffered command
recording, cached descriptor sets, fast host-to-local uploads; headless race 26.5 -> 18.8 ms per VBlank at 3x; the
owner measured stable 60 FPS in menus. The owner then saw the same defects as with the old core (ring notches in menus,
horizon stripes, random shadows), so the cause was in shared code: the front end dropped every primitive whose box holds
no pixel centre, which is right only at 1x (IN-403, found by black-box comparison with gt4-gs.exe final frames and
synthetic GS streams in `scripts/hw_blackbox/`). Fixed for both cores: the 6x menu ring now equals gt4-gs's, the grey
line across the road near the horizon is gone, race480p memory at 6x identical. Target heights round to page rows
(IN-404, no more menu-field overlap with the texture memory). Open: 480i deinterlace in CORE2, the car shadow,
frametime spikes at race start, CORE2 at 1x (HUD only), the remaining speed gap. Owner test 12 (candidate 285): passed
for picture faults ("zero bugs visuais"); open: race-start stutter and mid-race drops (the GS thread costs 20 ms per
VBlank at 3x, the GPU waits), no anti-aliasing, 480i deinterlace. Next steps in
[CORE2-ROADMAP](hw-renderer-spec/CORE2-ROADMAP.md). Roadmap phase A done in candidate 286 (IN-405): 480i deinterlace
and FXAA in core 2, with an adaptive rule that keeps GT4's fast-scrolling ticker clean as in gt4-gs. Owner test 13:
deinterlace clean like GSdx, but the race picture softer than GSdx's in the same 1080p window. Cause: the present
halved the 3x frame to 960 x 672 and stretched it back (IN-390 rule); fixed in candidate 287 (IN-406, both cores), one
bilinear reduction like the reference. Owner test 13b: sharpness solved. Phase B (deep performance pass, owner goal:
16.7 ms is the ceiling, beta gate a 2x i5-7200U/HD 620 laptop) started on a headless night B-Spec scene: palettes and
textures over drawn targets now read on the GPU, indexed textures keep their indices, submissions every 96 batches
(IN-407..IN-410, candidate 288): night race at 3x 20.8-26.4 -> 17.0-20.6 ms per VBlank, readbacks 6.9-8.7 -> 0.8-1.6 ms,
frames byte-identical. Owner test of 288: lighter at night, but the title backdrop broke after a GT Mode visit (GPU
reads kept a target alive past its ageing); fixed in candidate 289 (IN-411), night 17.8-22.2 ms per VBlank headless. Owner test of 289: title clean; night
race 57.0 VBlanks/s over 192 s (frame time p95 20.6 ms, p99 23.4 ms). Measured next: the GLM page copies cost only
0.3-0.5 ms per night VBlank, the CPU swizzle of EE uploads 2.3-2.8 ms; page-row runs cut it to 0.9 ms (IN-414,
candidate 290), and the new upload fixture found and fixed wrong T8H/T4HL/T4HH uploads. Most IN-411 store waits
were for nothing (GT4's streamed textures replace whole pages under its night render): skipped in candidate 291
(IN-415), GPU waits 2.3-3.4 -> 1.1-2.0 ms and GS thread 16-22 -> 14-19 ms per night VBlank. Owner test of 291:
night race 59.2 VBlanks/s over 190 s (p95 18.0, p99 19.6 ms), 60 FPS on the hood camera, no defects. Dirty
rectangles are now exact page by page and block by block for writes in the same layout (IN-416, candidate 292): the
remaining store waits went to about 0 ms on a long night race. CLUT loads over several or depth targets are read on
the GPU too (IN-417, candidate 293): GS thread 7.2-11.1 ms per VBlank on the long night race (292: 8.3-13.9).
Owner test of 293: 59.3 VBlanks/s, EE critical path 14.0 ms per frame. No allocation per batch and images reused by
size (IN-418, candidate 294): GS thread 5.5-8.9 ms per VBlank; the GPU side now shows up as the next limit. Owner
test of 294: 59.1 VBlanks/s, GPU-limited in the window (the GPU at 45 W of 180 W). GPU timestamps show a fixed cost
per command (1x 4.3-7.3 ms against 3x 6.6-9.1 ms); barriers only where a command depends on another (IN-419,
candidate 295) halved them, GPU time 3-13% less. Draw batches deferred into one render pass (IN-420, candidate
296): 4-8 render passes per VBlank instead of 106-312, GPU time 5-13% less; timestamps split the GPU time into
drawing 3.8-4.5 ms at 3x (scales with pixels, D1's target) and a fixed 1.4-3.8 ms of compute and transfer work.
D1 measured before building it (IN-421): the whole fragment stage is only 0.4-0.7 ms of GPU time at 3x, and the
draw-pass timestamps include overlapping compute; the GPU time is the chain of small dependent commands, the next
target. Textures over no target are now decoded from a CPU page window (IN-422, candidate 297): barriers down again,
GPU time 3-10% less. Every GPU-side step gives far less than its share; a low GPU clock at this partial load is the
next thing to check. All steps are tabled in `hw-renderer-spec/CORE2-PERF-LEDGER.md`. The owner's overlay confirmed
the GPU idles at a low clock (as with GSdx), so the frame is the EE critical path. The race-start stutters were found
with a per-VBlank spike log (IN-423): 40-160 image allocations per VBlank. Image memory now comes from 64 MiB blocks
(IN-424, candidate 298): race-start VBlanks over 20 ms 60 -> 2. Owner test of 298: race smooth at 60 FPS, frametime
still jittery in menus and slightly in the race. A GPU cost map (IN-425) showed the GPU has the power but is full of
gaps; with only two submissions in flight the CPU waited for it. Up to 8 submissions in flight and submissions every
32 batches (IN-426, candidate 299): Arcade B-Spec race VBlanks over 17 ms 59 -> 15, GPU waits cut to about a third.
Owner test of 299: 60 FPS, frametime nearly flat, fidelity perfect; phase B closed. Beta closing plan in CORE2-ROADMAP
(1x shows only the HUD, mipmapping, spec step 3 audit, the remaining spikes, a memory budget, CAS, present resampling,
the HD 620 laptop gate). After the beta, the 1.3.0-rc7 branch comes in and the PR goes to the code optimisation branch.
Not merged; no pins.

## TASK-027 CoreGS 2.0: scaled images sized to their slot (IN-384, parity plan Part A)

Branch `task/027-coregs2`. First half of item 1 of `docs/hw-renderer-spec/PARITY-PLAN.md`: every scaled presentation
image now holds only its slot's real extent (FBW * 64 wide, the loaded or drawn rows, rounded to a page), created
lazily and grown on demand with the old content copied. A draw, load or texture copy that reaches past the extent grows
the image first, so no pixel is clipped (`scaled_draws_clipped=0`). A texture taken from a resident slot copies only
the batch's sampled footprint (IN-380), so the 640 x 448 race frame no longer grows to its 1024 x 512 texture view.
Scaled image device memory on the 480p race capture: 64.1 MB at 2x, 243.1 MB at 4x, 541.4 MB at 6x, against 184.5 /
738.2 / 1,660.9 MB before (65-67% less; one 640 x 448 target is 41 MB instead of 151 MB at 6x). Gates unchanged:
CTest 245/245 in both interlock modes (new `hw_scale_target_extent`), the five memory captures byte-identical (menu
1,101 frames, race 121, car 121, night 121 with `fifo_hash=3a2fe617a174e7e7`, race480p 121), scaled memory equal to
1x and 0 low-detail presented frames at 2x/4x/6x, and the 06.3/IN-377/IN-379 fixtures unchanged. GPU busy on the
480p race replay (`--gpu-timing`, this build against candidate 273): 5.25/6.42/9.58/14.77 ms at 1/2/4/6x against
5.35/5.99/8.68/16.92; the 6x gain (about 13%, scaled pass 9.18 -> 8.04 ms) is the only one outside the run-to-run
spread, so the item is reported mainly as a memory win. Reported as `scaled_image_mb` on the `[hw] cost` line and on
the replay's `scale=` line with `scaled_image_grows` and `scaled_draws_clipped`. Next: the fast scaled raster itself
(an early-Z second pass with real attachments, tolerance only on the presented image). Not merged; no pins.

## TASK-027 CoreGS 2.0: tone-sequence strip executor (IN-383, parity plan item 2)

Branch `task/027-coregs2`. Owner requirement (2026-10-23): CoreGS 2.0 must reach GSdx's performance at the same
scale; item 2 of `docs/hw-renderer-spec/PARITY-PLAN.md` (tone-sequence HLE, memory-exact) landed first.
`hw_tone_strips.comp` writes the eight 8 x 32 decal T8 strips of one 64 x 32 frame-buffer page in one dispatch from
the GLM (index texel -> 07.4 CSM1 palette entry of the strip's quadrant -> CT24 RGB, alpha kept), replacing 135,399
strip batches with 16,940 page dispatches on the 480p race capture (140 per frame) and 118,580 palette loads held
(GPU CLUT loads 200,467 -> 98,827). All five memory captures are
byte-identical (menu 1,101 frames, race 121, car 121, night 121 with `fifo_hash=3a2fe617a174e7e7`, race480p 121),
`hw_tone_hle_exact` compares the dispatch against the batches (0 of 2 frames differ in 4 MiB, 0 pixels off the CPU
form of the rule, 0 alpha bytes changed) and CTest passes in both interlock modes. Replay CPU per frame: GS stream
9.95 -> 8.14 ms, batch 5.54 -> 4.28, translate 1.83 -> 0.97, execute 3.52 -> 2.89, CLUT 1.12 -> 0.75; GLM swizzles
33,938 -> 18,272. In the game (headless 480p race, same build with the HLE off and on): GS thread 13.4-15.6 ->
10.6-11.6 ms, render passes 341-347 -> 198-204, barriers 1,591-1,645 -> 1,437-1,474; GPU busy 7.19 -> 5.35 ms at 1x
and 19.01 -> 16.92 ms at 6x, which leaves the scaled pass (9.2 ms at 6x) as the next target (item 1 of the plan).
Switches `GT4_HW_TONE_HLE=0` / `--no-tone-hle` and `GT4_HW_VERIFY_SHUFFLE=1` / `--tone-verify`. Not merged; no pins.

## TASK-134: game-code optimization study for the i5-7200U laptop (2026-10-28)

2026-10-29: the branch is synced with 1.3.0-rc7 (`task/133-rc7-ra-controls` c21e934; program files identical);
the only per-frame change since rc6, the RetroAchievements hook on the EE thread, is in study section 3.8.

Research and plan only (no code, build, benchmark or game run): [GAME-CODE-OPTIMIZATION-STUDY](GAME-CODE-OPTIMIZATION-STUDY.md),
branch task/134-game-code-optimization-study. From the owner's laptop logs (rc6, Spec II, 1x, D3D11) the four busy
threads use about 35 ms of CPU per 16.7 ms frame (EE 11.7, VU1 worker 9.3, gt4-gs CPU thread 3.4, gt4-gs GS thread
~11); half or more of the EE critical path (12.2 ms) is emulation plumbing; syscalls and guest idle are negligible and
no large spin-wait exists. Ordered plan P0 (measure) to P8 (IOP AOT); the GSdx bridge is left to the CoreGS 2.0
intake, which must be zero-copy. Owner decisions recorded the same day (study section 7): cheapest steps first,
SSE4.1 only, translator last, no GSdx bridge work, game-side gains ship with CoreGS 2.0. P0 baselines (headless Arcade
night race and its replay, WPR profile) start when the owner says the CoreGS 2.0 build has freed the desktop.

## Local rc7 owner test candidate (2026-10-27)

Codex built an unpublished installer including the current Discord changes and verified extracted game and launcher hashes. Automated Rust tests passed; real Discord and gameplay remain untested. Important lifecycle and transport edge-case validation remains open; see docs/tasks/TASK-115-CODEX-LOCAL-KIT.md. This supersedes any earlier readiness claim.

## TASK-115 rc7 Discord Rich Presence review corrections (2026-10-27)

Superseded by the second review below; the earlier "corrected" wording was premature. The feature remains unreviewed by the owner and the Discord client is not yet validated.

## TASK-115 rc7 Discord Rich Presence second review (2026-10-27)

Second review corrections on branch `task/133-rc7-ra-controls`: `FILE_FLAG_OVERLAPPED`, safe completion waiting, protocol ack/error handling, generation fencing, non-blocking stop, and real Windows named-pipe tests (`windows_pipe_roundtrip`, `windows_pipe_read_timeout`) plus 6 manager tests, flow_audit and UI checks. Live Discord still pending owner authorization. Game Bar/GPU recognition remains future scope.
