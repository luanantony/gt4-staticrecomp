# Launcher UI (Tauri 2 frontend)

Plain HTML, CSS and JavaScript. No framework, no Node build, no remote content.
Strict CSP (see `tauri.conf.json`): no inline scripts, no inline styles, no `eval`.
Scripts are classic (not modules) so the pages also open from `file://`.

## Files

| Path | Purpose |
|---|---|
| `index.html` | App shell (Tauri `frontendDist` is `ui/`) |
| `css/app.css` | Tokens from `docs/launcher/LAUNCHER-PLAN.md` section 12.2 |
| `js/spatial.js` | Pure spatial focus picker (`GT4.spatial.pickNext`) |
| `js/input-rules.js` | Pure input rules: swipe step, right-stick scroll, slider value from a touch, face-button routing (`GT4.input`) |
| `js/backend.js` | Command bridge: `window.__TAURI__.core.invoke`, or the dev mock |
| `js/ui.js` | Element builder, hint bar, toast, dialogs, date format |
| `js/settings-model.js` | Pure settings row model: visibility, the row after a change, the Saves/Replays entry, the newest-read gate (`GT4.settingsModel`) |
| `js/nav.js` | Keyboard, the pad's `pad-nav` and `pad-live` events (SDL3 in Rust), focus ring |
| `js/controller-svg.js` | GT4Recomp's own DualShock 2 drawing (no Sony artwork): buttons, chips, live state, nearest-button pick |
| `js/controls-model.js` | Pure Controls rules: live state from the pad-live shape and keys, the fixed keyboard layout, trigger output, key names, conflicts, chip labels (`GT4.controlsModel`). The Gamepad-object helpers (`padLive`, `padSummary`) are for the dev mock only |
| `js/config.js` | Feature flags (`mods` off for release 1.0.0) |
| `js/app.js` | Router (`#/setup`, `#/home`, `#/settings/<tab>`) and boot |
| `js/pages/*.js` | Setup, Home, Settings shell and rows, Saves and Replays, Controls |
| `assets/` | Fonts (Roboto, Roboto Condensed), `cover-front.png`, `logo-gt4.png` |
| `dev/index.html` | Dev entry: same app with `dev/mock-data.js` (plain browser) |
| `dev/mock-data.js` | Mock command layer; sample values only, not game data |
| `dev/nav-test.html` | Unit tests: spatial picker, input rules, focus levels, settings row model (open in any browser) |
| `dev/focus-check.html` | DOM-level focus and value checks on the mock (headless Edge; see below) |
| `dev/MANUAL-CHECKLIST.md` | Manual checks for a controller and the Tauri build |

`ui/dev/` is for development. Exclude it from release bundles if the build does not need it.

## Commands the UI calls

Arguments are the payload object shown. Errors are strings or Error objects; the UI shows them as toasts.

| Command | Payload | Returns |
|---|---|---|
| `app_state` | none | `{setup: {state, step, progress, bytes_done, bytes_total, renderer_step, disc_name, problem, note}, renderer_installed: bool, renderer_version: string | null, game_running: bool, game_files}`; see "Setup states" below |
| `setup_choose_disc` | none | `{path, serial, version, name}` when the file is accepted, `null` when the picker was cancelled; a refused file is an error string (the reason), and `app_state` then says `wrong_disc` |
| `setup_start` | none | none; called by the page right after `setup_choose_disc` accepted a file (and by "Try again") |
| `setup_cancel` | none | none; the copy and the staging folder are removed, `app_state` says `failed` with "Setup was cancelled" |
| `play` | none | none |
| `stop_game` | none | none |
| `get_settings_table` | none | `{max_upscale: int, rows: [row]}` |
| `set_setting` | `{id, value}` | none (`Result<(), String>`); after a success the UI sets the row's value and reads the table again, since other rows can depend on it (`visible_when`, `disabled`) |
| `reset_gs_settings_to_defaults` | none | `{max_upscale, rows}` |
| `get_controls` | none | `{controls: [{id, name, binding, sources, xinput, key, is_default}], triggers: [row], locked}`; `id` is the input of the controls file (`cross`, `dpad_up`, `l2`...), `sources` the raw sources (`xinput:A`, `key:Z`), `xinput`/`key` the first of each family or null; `triggers` holds the analog rows (below); `locked` is true when the file comes from a newer launcher and is kept read-only |
| `capture_binding` | `{control}` | `{control, source}`: the next newly pressed control of the active pad, read through SDL3 (a control held at the start does not count; the source is in the controls-file names, SOUTH is `xinput:A`). It binds nothing; the page checks for a conflict and calls `assign_binding`. `cancel_capture` ends it with an error |
| `pad_status` | none | the last `pad-status` payload: `{available, connected, name, kind, player_index, count, error}` (`kind` is SDL's gamepad type, e.g. `xbox360`, `ps5`; `count` is the number of pads; the named pad is "controller 1 of N") |
| `cancel_capture` | none | none |
| `assign_binding` | `{control, source, swap}` | the `get_controls` view plus `swapped_with` (the other button or null) and `unchanged`. `source` replaces the button's source of the same family (`xinput:` or `key:`). When another button has it, `swap: true` gives that button the previous source (or removes it there when there was none) and `swap: false` is refused. The file is written at once |
| `reset_binding` | `{control}` | none; one button back to its built-in sources |
| `reset_all_bindings` | none | the `get_controls` view; every button, trigger and stick setting back to the built-in mapping |
| `saves_list` | none | `{items: [{id, name, modified, active}]}` |
| `saves_import` | none | `{items}` or `null` when the picker is cancelled |
| `saves_activate` | `{id}` | `{items}` |
| `saves_delete` | `{id}` | `{items}` (the UI confirms first) |
| `replays_list` | none | `{items: [{id, name, modified}]}` |
| `replays_import` | none | `{items}` or `null` |
| `replays_delete` | `{id}` | `{items}` (the UI confirms first) |
| `about_info` | none | `{version, gt4recomp, gt4gs, disc_path, game_files, data_folder}`; `gt4gs` is `not installed` until the renderer is there; `game_files` is the folder of the disc image copy |
| `open_log_folder` | none | none |
| `open_game_files_folder` | none | none; opens `game_files` in Explorer |
| `check_updates` | none | `{offers: [offer], errors: [string], upToDate: bool, launcher, renderer}`; offer = `{stream, installed, latest, notes, package, update}` |
| `apply_update` | `{stream}` (`launcher` or `renderer`) | `{ok, message}`; the renderer installs in place (not while the game runs); the launcher starts its setup and exits |
| `install_renderer_archive` | none | `{cancelled: bool, version: string, message: string}`; opens file picker for the player to select `gt4-gs-*.zip` and installs it |
| `skip_update` | `{stream, version}` | none; no offer again for exactly that version |
| `confirm_close` | none | none; the player chose Close launcher in the close confirmation |

`modified` is an ISO 8601 string; the UI formats it.

### Setup states (`app_state().setup.state`)

| state | When | Page |
|---|---|---|
| `needed` | never set up | "Choose disc image" |
| `wrong_disc` | the chosen file was refused (`problem` is the reason, for example "This disc is not supported yet. GT4Recomp supports Gran Turismo 4 (USA, SCUS-97328). ...") | "Not supported" row, the reason, "Choose another file" |
| `checking` | setup runs; `step` is `copying` (bytes), `checking`, `preparing` or `renderer` (`renderer_step`: `listing`, `downloading` with bytes, `installing`) | rows with a bar, Cancel (not during the renderer step) |
| `failed` | a run stopped: cancelled, not enough free space, a write error | the reason, "Try again", "Choose another file" |
| `files_missing` | set up once, but the copy of the disc image or the prepared files are gone or changed | "Game files missing", "Choose disc image" |
| `done` | ready (the receipt's size and modified time of the copy match; nothing is hashed) | "Ready", Play; `note` says the game cannot start yet because gt4-gs could not be downloaded (TASK-096: no CoreGS 1.1 fallback) |

The game reads only the copy in `%LOCALAPPDATA%\GT4Recomp\disc\SCUS-97328.iso`. The file the player picked may be
moved or deleted after setup.

Settings row (`rows[]` in the table):

```
{ id, label, help, page: "basic"|"advanced"|"mods"|"hidden",
  group?: "graphics"|"display",          // splits page=basic into the Display and Graphics sections of the Graphics tab
  control: "slider"|"toggle"|"dropdown",
  values, default, current, disabled?,
  visible_when?: {id, equals} }          // e.g. FSR sharpness only when scaling_filter == "fsr"
```

- slider values: `{min, max, step, unit?, limit?: "max_upscale", scale?, labels?}`; the effective max is `min(max, table[limit])`. `labels` maps a value to its readout (TASK-080 Render scale: `{"100": "100% · 4x · 2560x1792"}`); a slider with labels gets a wider control.
- TASK-080 addendum rows: `monitor` (dropdown of the connected monitors, values `<GDI device>@<W>x<H>`, `primary` when none is known), `resolution` (dropdown of the chosen monitor's modes, `WxH`, `desktop` when none is known; set_setting `monitor` resets it to `desktop`), `render_scale` (slider 50..200 step 25 with `labels`; GSdx is the only renderer, so no `visible_when`). There is no `aspect_ratio`, `aspect_custom` or `upscale` row.
  With `scale` (TASK-080 Custom ratio: 100) the stored whole number is shown divided by it with two decimals
  (`233` reads "2.33:1").
- dropdown values: `[{value, label, disabled?, note?}]`; disabled entries are greyed and not selectable.
- toggle values: `null`; `current` is a bool.
- The Controls analog rows use the same row shape (`group: "controls"`). Their ids are `l2_deadzone`, `l2_curve`, `r2_deadzone`, `r2_curve`, `lstick_deadzone` and `rstick_deadzone` (percent of travel, 0-95), and they are written with `set_setting`. A trigger row tunes the trigger source of that button (`deadzone=`/`curve=` options); a stick row writes `deadzone=` on both axis sources of the stick.

Pad events (from the Rust pad service, `src/pad_service.rs`, SDL3; read in `js/nav.js` and `js/pages/controls.js`):

| Event | Payload | Meaning |
|---|---|---|
| `pad-status` | `{available, connected, name, kind, player_index, count, error}` | sent when the pad state changes; `available` is false when SDL3 did not start (the reason is in `error`) |
| `pad-live` | `{sources, sticks}` | about 60 times a second while a pad is connected and the window is shown. `sources` maps each controls-file source (`xinput:A`, `xinput:LT`...) to its pressure 0..1; `sticks.left` and `sticks.right` are `{x, y}` in -1..1 with y positive down |
| `pad-nav` | `{name}` | a press for navigation: `a b x y lb rb start` once, and `up down left right` with repeat (first press at once, after 420 ms, then every 110 ms while held; the left stick past half deflection counts as a direction) |

The pad service does no SDL work while the window is hidden for a game (the tray hides it on Play).

Other Tauri events from Rust (listened to in `js/app.js`, popup code in `js/updates.js`):

| Event | Payload | Meaning |
|---|---|---|
| `update-available` | `{offers: [offer]}` | Sent at launch when a stream has a newer stable version that is not skipped. Shows the update popup. |
| `setup-progress` | `{step, percent, bytesDone, bytesTotal}` | Setup progress (also in `app_state`, which the page polls). |
| `setup-finished` | `{ok, message}` | The end of a setup run. |
| `renderer-progress` | `{step}`: `listing`, `downloading` (with `percent`, `bytesDone`, `bytesTotal` during setup), `installing`, `downloading-launcher`, `done`, `failed` | Progress of a gt4-gs install (setup, the retry at launch, or an update). The setup page draws it; elsewhere `done` and `failed` are toasts. |
| `update-finished` | `{stream, ok, message}` | The end of an `apply_update`. |
| `game-state` | `{running, message}` | The game ended (`running: false`); a non-empty message is shown and Home is opened. |
| `close-confirm` | `{message}` | The window close button was pressed while the game runs. The page asks "Close the launcher anyway?". |

The update popup: Update (runs `apply_update`), Not now (closes it; B does the same), Skip this version (runs `skip_update`). The release notes are shown as text (`#` headings, bullets and `**bold**` only).

The Rust side registers exactly these names (`src/main.rs`). `src/contract.rs` checks both directions: every command a
page calls is registered, and every registered command is called by a page.

## Navigation map

Focus moves spatially (nearest element in the pressed direction; sideways offset costs twice the distance).

| Page | A | B / Esc | X | Y | Start | LB/RB or Q/E | Left/right |
|---|---|---|---|---|---|---|---|
| Setup | primary button (Choose disc image / Choose another file / Play) | Cancel while copying or checking; Home when done | none | none | none | none | none |
| Home | Play | none | none | Settings | Play | none | none |
| Settings: Graphics (with its Advanced section) | activate (dropdown opens its list) | Back to Home (closes a tip or a dialog first) | none | Reset to defaults (confirm) | none | previous / next tab | adjust slider or dropdown |
| Settings: Display | activate (dropdown opens its list) | Back to Home (closes a tip or a dialog first) | none | none | none | previous / next tab | adjust slider or dropdown |
| Settings: About | activate (log folder) | Back to Home | none | none | none | tabs | none |
| Saves | select / make active | Back | Import | Make active | none | tabs | none |
| Replays | none | Back | Import | Delete (confirm) | none | tabs | none |
| Controls (on the drawing) | rebind the focused DS2 button | Back to the tab list | Reset all (asks first) | Reset this button | none | tabs | D-pad and stick move between DS2 buttons by position |
| Rebind overlay | none | cancel (Esc too); every other pad button or key is the new binding | none | none | none | none | none (navigation is suspended) |
| Conflict dialog | Swap (or Move) / Cancel | cancel | none | none | none | none | move between the two buttons |
| Dialog (list, confirm) | activate | close | none | none | none | none | none |

Right stick: scrolls the nearest scroll area of the focused element (or the page main area). Deadzone 0.2.
Footer hints (A Select, B Back, X Import or Rebind where the tab has them, Y where defined, LB RB Tabs) are tappable by touch and mouse. Every settings change applies at once: there is no Save action.

Routing of the face and shoulder buttons lives in `GT4.input.padRoute` (`js/input-rules.js`), tested in `dev/nav-test.html`.

## Touch

- Every target is at least 48 x 48 CSS px (the visuals stay small; the hit area is padded). `pointer: coarse` adds a little spacing.
- Nothing depends on hover. A help "i" opens on tap and closes on a second tap or a tap outside. Focus shows it for keys and pad only.
- Sliders drag with touch (pointer events with capture; `touch-action: pan-y`). Dropdowns open as a list.
- Long lists (settings, saves, replays) scroll with a finger and do not chain the scroll to the page.
- A horizontal swipe over 60 px on the Settings page (finger, mostly horizontal) switches tabs: left goes to the next tab, right to the previous one. Swipes that start on a slider or a dropdown list do not switch tabs.

## Play button

Idle: dark translucent glass with a thin white border and a slow light sweep (about every 3.4 s). Focused (pad or keys) and hovered: solid white, dark label, scale 1.03, soft glow. Pressed: a quick press-down. Under `prefers-reduced-motion` the sweep is removed and the idle state stays static. Other primary buttons (`.btn.primary`) use the same idle and focused language, without the sweep.

Tab order for LB/RB and Q/E: Graphics, Controls, Saves, Replays, About. Display and Advanced are not tabs (owner,
2026-10-21): the Graphics tab holds three sections, in order **Display** (Monitor, Resolution, Fullscreen, VSync: the
`group = display` rows), **Graphics** (Renderer, Render scale, FXAA, CAS: the other `page = basic` rows) and **Advanced**
(the `page = advanced` rows, then "Reset to defaults"; Y resets). Each section has a heading styled like "ADVANCED"
(`.section-head`, `section[data-section]`). The D-pad moves down through all three, B returns to the tab list from any of
them, and `#/settings/display` and `#/settings/advanced` fall back to Graphics. Screenshots:
`docs/launcher/shots/display-monitor-resolution.png` (`dev/index.html?focus=content#/settings/graphics`: the Display
section at the top), `docs/launcher/shots/graphics-sections.png` (`dev/focus-check.html?scenario=render`) and
`docs/launcher/shots/graphics-advanced-section.png` (`dev/focus-check.html?scenario=advanced`). The Mods tab and its rows are behind `GT4.features.mods` in `js/config.js` (off in release 1.1.0 builds; the mod manager ships in 1.1.0).

Keyboard: arrows move, Enter/Space activate (native), Esc = B, Q/E = LB/RB, Tab/Shift+Tab move in DOM order.

Settings focus: moving the highlight over the tab list (up and down) only previews the tab and never enters it; only A or D-pad right (or Enter) enters. Rendering a tab never moves focus. Saves and Replays enter on the first row (Import only when the list is empty); Controls enters on the DS2 button selected last (Cross at first).

## Controls page

- **Drawing.** `js/controller-svg.js` builds GT4Recomp's own DS2 drawing (mirrored cubic halves, plain geometry, stroke symbols; no Sony artwork, photos, logos or marks). Classes and SVG attributes only (CSP). Each DS2 button is a focusable `<g class="pad-ctl" data-control="cross">`; a chip beside it, with a short leader line, shows the XInput control (Controller view) or the key (Keyboard view). Chips of changed buttons have an amber edge. When the drawing is narrower than 560 px the chips grow 1.35x.
- **Live input.** The page listens to `pad-live` (about 60 Hz, from SDL3 in Rust) and keeps the latest reading; a frame draws it with a 100 ms timer when the window produces no frames. `GT4.controlsModel.liveState` maps the controls-file sources (`xinput:A`...) through the current bindings to DS2 buttons, so a swapped button lights where the game would see it. L2/R2 fill with the raw trigger value; the panel meter shows the pressure the game computes after the deadzone and curve (`triggerOutput`, the formula of `native/controls.cpp`). Sticks move in their wells; the panel scopes show the per-axis deadzone square. The status pill comes from `pad-status` (SDL's pad name, "controller 1 of N" when several are connected, the one the game reads). In the Keyboard view the chips show the fixed modern layout (`GT4.controlsModel.KEYBOARD_LAYOUT`, read only, several keys per button); the nitro, gear, look-back and camera keys have no button in the table yet.
- **Selecting and rebinding.** Pad and keys: focus is the selection, A (Enter, Space) rebinds. Mouse and touch: a click or tap selects the nearest button or chip (large targets at any size); a second click, or Rebind in the panel, starts the rebind. The overlay counts down 5 s; the pad press comes from `capture_binding` (SDL3). Keyboard rebinding is off: in the overlay a key only cancels (Esc), and in the Keyboard view Rebind and Reset are disabled. B cancels, so B itself is never captured: B moves only through a swap.
- **Conflicts.** `GT4.controlsModel.conflict` finds the button that already has the source; the dialog offers Swap (the other button gets the previous source) or Move (when there is none), and Cancel. Rust applies the same rule (`gt4controls::Controls::assign`) and refuses a taken source without `swap`.
- **Footer.** Content level: A Rebind, Y Reset button, X Reset all, B Back, LB RB Tabs on the drawing; A Select instead of A Rebind on the panel. Rebind: B Cancel. Dialogs: A Choose, B Cancel. The tab list keeps the shell's footer (`host.hints(list, {replace: true})`).
- **Saving.** Every change writes `controls.toml` at once (no Save). A refused change or a failed write keeps the old file and shows the reason. The info "i" says that this game build still uses the built-in mapping; the bindings apply from the next game build.
Mouse use hides the focus ring. A pad or key press brings it back.

## Mock data and browser preview

Serve `ui/` with any static server and open `dev/index.html`, for example
`python -m http.server 8765` in `launcher/crates/gt4l-tauri/ui`, then `http://127.0.0.1:8765/dev/index.html#/home`.

Query options: `?setup=done|needed|wrong|copying|preparing|renderer|failed|missing|offline` (default `done`), `&pick=spec` (the mock picker returns a refused Spec II image), `&running=1`, `&focus=play` (home only: focus the Play button, for screenshots), `&focus=content` (Settings: enter the tab).

Controls (dev only): `&pad=A,LB,DpadRight` (pressed XInput names of a still dev pad; the mock sends them as `pad-live` events, the same shape as the Rust service), `&lt=0.6&rt=0.2` (trigger values), `&lx=0.7&ly=-0.45&rx=&ry=` (stick axes, y down), `&pads=2` (a second pad), `&nopad=1` (no controller), `&select=circle`, `&mode=key` (Keyboard chips), `&remap=1` (a few changed buttons), `&locked=1` (read-only file), `&capture=hold` (the rebind overlay stays open, still countdown) or `&capture=conflict` (the capture returns LB, so the conflict dialog opens). Example, the live-input screenshot: `dev/index.html?focus=content&pad=A,LB,DpadRight,Start&lt=0.6&rt=0.15&lx=0.75&ly=-0.5&rx=-0.45&ry=0.7#/settings/controls`. Screenshots: headless Edge with `--force-prefers-reduced-motion` (`docs/launcher/shots/controls-v2-*.png`).

`dev/nav-test.html` checks the spatial picker, the swipe, stick and slider rules, the face-button routing, the two focus levels and the settings row model in a browser. It also checks the Controls model (live state from the pad-live shape, swaps, keys and the fixed keyboard layout, trigger output, key names, conflicts, chip labels, pad summary) and spatial moves between DS2 buttons on the drawing's own layout. It should report every case passed (150 of 150 at the time of writing).

`dev/focus-check.html` runs the DOM-level checks on the same mock (headless Edge, `--virtual-time-budget=60000`): the sidebar highlight over every tab with arrow keys, entering and leaving each tab, the Saves and Replays entry, no Save hint, that no aspect or internal-resolution row is shown, that the Render scale readout follows a step and the chosen Resolution, that a changed resolution (Display section), toggle or slider shows its new value, the three sections of Graphics in order (no Display or Advanced tab, both routes fall back to Graphics, Monitor first, the D-pad walk from Display through Graphics to Reset to defaults), and the Controls page: entry on Cross, D-pad moves between DS2 buttons with the selection following, rebind and cancel (Esc), Enter not taken as the new key, the conflict dialog from a pad button (the mock answers RB; cancel, then Swap with the chips updated), live input from a still dev pad (Cross lit, L2 at 60%, the stick at its position, the pad name, no focus move), Y reset and X reset all. `body[data-focus-check]` is `pass` when all 100 checks pass. `&scenario=4k` only sets the state for the screenshot (Render scale at 200%; `docs/launcher/shots/graphics-4k-updated.png`); `docs/launcher/shots/display-monitor-resolution.png` is `dev/index.html?focus=content#/settings/graphics` (the Display section at the top of Graphics).
