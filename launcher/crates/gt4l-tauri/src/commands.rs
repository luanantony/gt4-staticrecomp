//! The launcher's commands other than setup: the snapshot, Settings, Play, Controls, Saves and
//! Replays, and About. Each command is a thin wrapper over a crate; the rules live in `model`,
//! `saves` and the `gt4l-*` crates.

use std::fs;
use std::path::{Path, PathBuf};

use gt4controls::{self as controls_file, default_bindings};
use gt4l_core::display::{self as display_rules, DisplayTarget};
use gt4l_core::exit::{self, Outcome};
use gt4l_core::gs_release;
use gt4l_core::launch;
use gt4l_core::paths::{self, DataDirs, GAME_EXECUTABLE};
use gt4l_core::settings;
use gt4l_core::{Settings, LAUNCHER_DISPLAY_VERSION};
use gt4l_disc::setup::{self as disc_setup, Installed};
use gt4l_platform::game;
use gt4saves::library::Report;
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;

use crate::model;
use crate::pad_service;
use crate::setup;
use crate::saves::{self, Item};
use crate::state::{self, lock, AppState, SetupOutcome, Shared};
use crate::webview_guard;

/// `app_state()` (ui/dev/README.md):
/// `{setup: {state, step, progress, bytes_done, bytes_total, renderer_step, disc_name, problem, note}, game_running,
/// game_files}`.
///
/// `state` is `needed` (never set up), `wrong_disc` (the chosen file was refused), `checking` (setup runs),
/// `failed` (a run stopped: cancel, no space, write error), `files_missing` (set up once, but the copied image or
/// the prepared files are gone or changed) or `done`. No hash runs here: the copied image is trusted by its receipt.
#[tauri::command]
pub fn app_state(shared: State<'_, Shared>) -> Result<Value, String> {
    Ok(app_state_of(&lock(shared.inner())))
}

pub fn app_state_of(state: &AppState) -> Value {
    let installed = state.installed();
    let chosen_name = state
        .disc_path()
        .and_then(|path| path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let run = &state.setup_run;
    let (screen, problem, disc_name) = if visual_ready() {
        ("done", String::new(), setup::disc_display_name("2.00"))
    } else if state.setup_busy {
        ("checking", String::new(), chosen_name)
    } else if let Installed::Ready { version, .. } = &installed {
        ("done", String::new(), setup::disc_display_name(version))
    } else {
        match &state.setup_outcome {
            SetupOutcome::Refused(message) => ("wrong_disc", message.clone(), chosen_name),
            SetupOutcome::Failed(message) => ("failed", message.clone(), chosen_name),
            SetupOutcome::Done { .. } => (
                "files_missing",
                "Game files missing. Choose your disc image again.".to_string(),
                chosen_name,
            ),
            SetupOutcome::None => match &installed {
                Installed::Missing { reason } => (
                    "files_missing",
                    format!("Game files missing ({reason}). Choose your disc image again."),
                    chosen_name,
                ),
                _ => ("needed", String::new(), chosen_name),
            },
        }
    };
    let note = match &state.setup_outcome {
        SetupOutcome::Done { renderer_note } => renderer_note.clone(),
        _ => String::new(),
    };
    json!({
        "setup": {
            "state": screen,
            "step": if run.step.is_empty() { "copying" } else { run.step.as_str() },
            "progress": run.fraction,
            "bytes_done": run.bytes_done,
            "bytes_total": run.bytes_total,
            "renderer_step": run.renderer_step,
            "disc_name": disc_name,
            "problem": problem,
            "note": note,
        },
        "game_running": state.game_child.is_some(),
        "game_files": state.dirs.disc_folder().display().to_string(),
        "migration": {
            "running": state.migration.running,
            "item": state.migration.item,
            "bytes_done": state.migration.bytes_done,
            "bytes_total": state.migration.bytes_total,
            "notice": state.migration.notice,
            "problem": state.migration.problem,
        },
        "visual_page": visual_page(),
    })
}

#[cfg(feature = "visual-check")]
fn visual_page() -> Option<String> {
    std::env::var("GT4RECOMP_VISUAL_PAGE").ok()
}

#[cfg(not(feature = "visual-check"))]
fn visual_page() -> Option<String> {
    None
}

#[cfg(feature = "visual-check")]
fn visual_ready() -> bool {
    std::env::var("GT4RECOMP_VISUAL_READY").is_ok_and(|value| value == "1")
}

#[cfg(not(feature = "visual-check"))]
fn visual_ready() -> bool {
    false
}

// Settings ------------------------------------------------------------------------------------

/// `get_settings_table()`: `{max_upscale, rows}`, the rows of ui/dev/README.md (see `model::settings_table`).
/// TASK-080 addendum: the Monitor and Resolution rows list the connected monitors and their modes, read each time.
#[tauri::command]
pub fn get_settings_table(shared: State<'_, Shared>) -> Result<Value, String> {
    get_settings_table_with_monitors(shared.inner(), &gt4l_platform::display::monitors())
}

/// The table without monitors (tests and the flow audit): Monitor shows "Primary monitor", Resolution the desktop.
pub fn get_settings_table_of(shared: &Shared) -> Result<Value, String> {
    get_settings_table_with_monitors(shared, &[])
}

pub fn get_settings_table_with_monitors(shared: &Shared, monitors: &[display_rules::Monitor]) -> Result<Value, String> {
    let state = lock(shared);
    let mut table = model::settings_table_with_monitors(&state.config.settings, &state.controls, monitors);
    // TASK-091: the launcher's own row (Graphics > Advanced), not a game setting.
    let safe_mode = webview_guard::safe_mode_on(state.user_dirs.root());
    if let Some(rows) = table.get_mut("rows").and_then(Value::as_array_mut) {
        rows.push(webview_guard::gpu_row(safe_mode));
    }
    Ok(table)
}

/// `set_setting({id, value})`: a launcher row, a PCSX2 GS table row, a trigger row (`l2_deadzone`, `r2_curve`, ...) or
/// a stick row (`lstick_deadzone`, `rstick_deadzone`).
/// A refused value leaves the setting unchanged and returns the reason.
#[tauri::command]
pub fn set_setting(app: AppHandle, id: String, value: Value, shared: State<'_, Shared>) -> Result<(), String> {
    set_setting_of(id.clone(), value, shared.inner())?;
    // TASK-089 extension (1.2.0): choosing GSdx installs it at once when it is missing (renderer_switch.rs).
    if id == "renderer" {
        crate::renderer_switch::start(app, shared.inner().clone());
    }
    Ok(())
}

pub fn set_setting_of(id: String, value: Value, shared: &Shared) -> Result<(), String> {
    let mut state = lock(shared);
    if id == webview_guard::GPU_ROW_ID {
        // TASK-091: the launcher's own toggle; takes effect at the next start.
        let on = value.as_bool().ok_or_else(|| format!("'{id}' needs on or off"))?;
        let now = webview_guard::unix_seconds();
        return webview_guard::set_gpu_acceleration(state.user_dirs.root(), on, now);
    }
    if let Some(definition) = settings::definition(&id) {
        let text = model::value_text(&value, definition.kind == settings::Kind::Toggle)
            .ok_or_else(|| format!("'{id}' needs a plain value"))?;
        state.config.settings.set_by_user(&id, &text).map_err(|error| error.to_string())?;
        if id == "monitor" {
            // Another monitor has other modes: the Resolution goes back to that monitor's desktop mode.
            state.config.settings.set("resolution", display_rules::DESKTOP).map_err(|error| error.to_string())?;
        }
        if id == "resolution" {
            // TASK-088 (1.2.0): a size equal to the monitor's desktop is stored as "desktop" (it then follows the desktop
            // at every start). Any other size is stored with the monitor's identifier, which records the desktop size at
            // the time of the choice (`display::desktop_at_save`).
            let monitors = gt4l_platform::display::monitors();
            if let Some(monitor) = display_rules::resolve_monitor(&monitors, state.config.settings.get("monitor")) {
                let desktop_choice = text == display_rules::DESKTOP || display_rules::Size::parse(&text) == Some(monitor.desktop);
                if desktop_choice {
                    state.config.settings.set("resolution", display_rules::DESKTOP).map_err(|error| error.to_string())?;
                } else {
                    let identifier = display_rules::monitor_id(monitor);
                    state.config.settings.set("monitor", &identifier).map_err(|error| error.to_string())?;
                }
            }
        }
    } else if let Some((trigger, field)) = id.split_once('_').filter(|(name, _)| ["l2", "r2"].contains(name)) {
        let current = state.controls.trigger_tuning(trigger);
        let (deadzone, curve, scale) = match field {
            "deadzone" => (
                value.as_f64().ok_or_else(|| format!("'{id}' needs a number"))? as f32,
                current.curve.as_str().to_string(),
                current.scale * 100.0,
            ),
            "curve" => (
                current.deadzone * 100.0,
                value.as_str().ok_or_else(|| format!("'{id}' needs a curve name"))?.to_string(),
                current.scale * 100.0,
            ),
            "scale" | "sensitivity" => (
                current.deadzone * 100.0,
                current.curve.as_str().to_string(),
                value.as_f64().ok_or_else(|| format!("'{id}' needs a number"))? as f32,
            ),
            _ => return Err(format!("unknown setting '{id}'")),
        };
        let tuning = model::trigger_tuning(deadzone, &curve, scale)?;
        return change_controls(&mut state, |controls| {
            controls.set_trigger_tuning(trigger, tuning).map_err(|error| error.to_string())
        });
    } else if let Some(stick) = model::stick_setting(&id) {
        let percent = value.as_f64().ok_or_else(|| format!("'{id}' needs a number"))?;
        if !(0.0..=100.0).contains(&percent) {
            return Err("the deadzone must be between 0 and 100 percent".into());
        }
        return change_controls(&mut state, |controls| {
            controls.set_stick_deadzone(stick, percent as f32 / 100.0).map_err(|error| error.to_string())
        });
    } else if let Some(row) = model::pcsx2_table_rows().into_iter().find(|row| row.id == id) {
        let text = model::gs_value_text(row, &value).ok_or_else(|| format!("'{id}' needs a plain value"))?;
        state.config.settings.set_gs(&id, &text).map_err(|error| error.to_string())?;
    } else {
        return Err(format!("unknown setting '{id}'"));
    }
    state.save_config();
    Ok(())
}

/// `reset_gs_settings_to_defaults()`: every PCSX2 GS table row back to its default. Returns the new table.
#[tauri::command]
pub fn reset_gs_settings_to_defaults(shared: State<'_, Shared>) -> Result<Value, String> {
    reset_gs_settings_to_defaults_of(shared.inner())
}

pub fn reset_gs_settings_to_defaults_of(shared: &Shared) -> Result<Value, String> {
    let mut state = lock(shared);
    state.config.settings.clear_gs();
    state.save_config();
    Ok(model::settings_table(&state.config.settings, &state.controls))
}

// Play ------------------------------------------------------------------------------------------

/// Everything Play needs, checked: the copied disc image, the prepared ELF, the game program, the settings, the
/// renderer and the extra environment. The flow test builds the launch plan from this without starting anything.
pub struct PlayInputs {
    pub dirs: DataDirs,
    pub disc: PathBuf,
    pub elf: PathBuf,
    pub executable: PathBuf,
    pub settings: Settings,
    /// The installed GSdx (the only renderer, TASK-096); Play does not start without it.
    pub pcsx2: gs_release::Installed,
    /// The resolved Monitor and Resolution (TASK-080 addendum); `None` when no monitor could be read.
    pub display: Option<DisplayTarget>,
    pub extra: Vec<(String, String)>,
}

/// The checks of Play before anything starts. `program_dir` is the folder of `GT4Recomp.exe`.
pub fn prepare_play(shared: &Shared, program_dir: &Path) -> Result<PlayInputs, String> {
    let state = lock(shared);
    if state.game_child.is_some() {
        return Err("The game is already running.".into());
    }
    if state.migration.running {
        return Err("GT4Recomp is moving its game files to the install folder. Wait until it has finished.".into());
    }
    let disc = match state.installed() {
        Installed::Ready { image, .. } => image,
        Installed::Missing { reason } => {
            return Err(format!("Game files missing ({reason}). Choose your disc image again."));
        }
        Installed::NotSetUp => return Err("Set up the game first.".into()),
    };
    let dirs = state.dirs.clone();
    let game_dir = paths::game_directory(program_dir, state.config.dev_game_dir.as_deref());
    let executable = game_dir.join(GAME_EXECUTABLE);
    if !executable.exists() {
        return Err("The game program is missing. Reinstall GT4Recomp.".into());
    }
    let mut extra: Vec<(String, String)> = Vec::new();
    let controls_path = state.user_dirs.controls_file();
    if controls_path.exists() {
        extra.push(("GT4_CONTROLS_FILE".into(), controls_path.display().to_string()));
    }
    let ra_cfg = gt4l_core::retroachievements::RaConfig::load(&state.user_dirs.retroachievements_file());
    for (key, value) in ra_cfg.launch_environment(&dirs.retroachievements_cache()) {
        extra.push((key, value.to_string_lossy().into_owned()));
    }
    // Only GSdx runs the game (decision 0070, TASK-096): without a verified GSdx, Play stops here with the error.
    let settings = state.config.settings.clone();
    let installed = gs_release::installed(&dirs.renderer_root());
    let pcsx2 = launch::choose_renderer(installed.as_ref(), &dirs.logs())?.clone();
    // TASK-080 addendum: the stored Monitor and Resolution against the monitors connected now.
    let monitors = gt4l_platform::display::monitors();
    let display = display_rules::resolve_target(&monitors, settings.get("monitor"), settings.get("resolution"));
    // TASK-088 (1.2.0): a stored size the monitor no longer has plays at its desktop mode, and the log says so.
    if let Some(monitor) = display_rules::resolve_monitor(&monitors, settings.get("monitor")) {
        if let Some(desktop) = display_rules::resolution_fallback(monitor, settings.get("resolution")) {
            state::log(
                &dirs,
                &format!(
                    "display: resolution {} is not a mode of {} now; playing at its desktop {}",
                    settings.get("resolution"),
                    monitor.device,
                    desktop.text()
                ),
            );
        }
    }
    let plan = launch::session_plan(&settings, display.as_ref());
    state::log(
        &dirs,
        &format!(
            "display: {} output {} -> {}x internal {}x{} (k {}, render scale {}%, aspect {:.4}{})",
            display.as_ref().map(|target| target.device.as_str()).unwrap_or("unknown monitor"),
            display.as_ref().map(|target| target.output.text()).unwrap_or_else(|| "unknown".into()),
            plan.multiplier,
            plan.internal_width,
            plan.internal_height,
            plan.base_multiplier,
            plan.render_scale,
            plan.aspect,
            if plan.ultrawide_on { ", aspect path on" } else { "" }
        ),
    );
    Ok(PlayInputs {
        display,
        elf: dirs.prepared().join(disc_setup::ELF_FILE),
        dirs,
        disc,
        executable,
        settings,
        pcsx2,
        extra,
    })
}

/// Starts the game with the chosen settings. Returns once the process is running; the page hears
/// about the end through the `game-state` event.
#[tauri::command]
pub fn play(app: AppHandle, shared: State<'_, Shared>) -> Result<(), String> {
    let shared: Shared = shared.inner().clone();
    // TASK-081: the processor and Vulkan checks before the game starts (`gt4l_core::system_check`).
    if let Some(problem) = gt4l_platform::system::requirements_problem() {
        let mut state = lock(&shared);
        state::log(&state.dirs, &format!("system requirement not met: {problem}"));
        state.home_message = problem.clone();
        return Err(problem);
    }
    let inputs = prepare_play(&shared, &program_directory()).inspect_err(|message| {
        lock(&shared).home_message = message.clone();
    })?;
    let PlayInputs { dirs, disc, elf, executable, settings, pcsx2, display, extra } = inputs;
    let card = dirs.runtime_card();
    saves::prepare_card(&lock(&shared).library, &card)?;
    let textures = lock(&shared).dirs.textures();
    paths::ensure_texture_folders(&textures).map_err(|error| format!("texture folder: {error}"))?;
    let running = game::start(&dirs, &executable, &elf, &disc, &settings, &pcsx2, display.as_ref(), &textures, &extra)?;
    {
        let mut state = lock(&shared);
        state.game_child = Some(running.child.clone());
        state.home_message.clear();
    }
    state::log(&dirs, &format!("game started, session {}", running.session_dir.display()));
    let _ = app.emit("game-state", json!({ "running": true, "message": "" }));
    crate::tray::handle(&app, crate::tray::Event::GameStarted);

    std::thread::spawn(move || {
        let outcome = game::wait_for_exit(&running);
        // TASK-096: a GSdx restart request ends the session with the GSdx error. CoreGS 1.1 is never started.
        let log_text = fs::read_to_string(&running.log_path).unwrap_or_default();
        let gsdx_error = launch::restart_requested(&log_text)
            .then(|| launch::gsdx_unavailable_message(launch::GSDX_RESTART_REQUESTED, &dirs.logs()));
        match saves::harvest_card(&lock(&shared).library, &card) {
            Ok(report) => state::log(&dirs, &format!("card harvested: {report:?}")),
            Err(error) => state::log(&dirs, &format!("card not harvested: {error}")),
        }
        let log_path = running.log_path.clone();
        drop(running);
        lock(&shared).game_child = None;
        state::log(&dirs, &format!("game ended: {outcome:?} (log {})", log_path.display()));
        let message = match (gsdx_error, &outcome) {
            (Some(error), _) => {
                state::log(&dirs, "GSdx asked for a restart; the launcher does not start CoreGS 1.1");
                error
            }
            (None, Outcome::NormalClose) => String::new(),
            (None, Outcome::Crashed { .. }) => exit::user_message(&outcome),
        };
        lock(&shared).home_message = message.clone();
        crate::tray::handle(&app, crate::tray::Event::GameEnded { message });
    });
    Ok(())
}

/// Stops the game process. The card is harvested by the watcher thread after the exit.
#[tauri::command]
pub fn stop_game(shared: State<'_, Shared>) -> Result<(), String> {
    let child = lock(shared.inner()).game_child.clone();
    if let Some(child) = child {
        let mut guard = match child.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(process) = guard.as_mut() {
            process.kill().map_err(|error| format!("could not stop the game: {error}"))?;
        }
    }
    Ok(())
}

fn program_directory() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

// Controls ---------------------------------------------------------------------------------------

/// Shown when the controls file was written by a newer launcher: it is kept as it is.
const CONTROLS_LOCKED: &str =
    "The controls file was written by a newer GT4Recomp and is kept as it is. Update the launcher to change controls.";

/// Applies `change` to a copy of the controls, writes the file, and only then keeps the copy. A refused change or a
/// failed write leaves the controls and the file as they were, and returns the reason.
fn change_controls<T>(
    state: &mut AppState,
    change: impl FnOnce(&mut controls_file::Controls) -> Result<T, String>,
) -> Result<T, String> {
    if state.controls_locked {
        return Err(CONTROLS_LOCKED.to_string());
    }
    let mut next = state.controls.clone();
    let result = change(&mut next)?;
    controls_file::save(&state.controls_path(), &next)
        .map_err(|error| format!("The controls file could not be saved: {error}"))?;
    state.controls = next;
    Ok(result)
}

fn controls_view(state: &AppState) -> Value {
    model::controls_view(&state.config.settings, &state.controls, state.controls_locked)
}

/// `get_controls()`: `{controls: [{id, name, binding, sources, xinput, key, is_default}], triggers: [row], locked}`.
/// `triggers` holds the analog rows (trigger deadzone and curve, stick deadzones) in the settings row shape.
#[tauri::command]
pub fn get_controls(shared: State<'_, Shared>) -> Result<Value, String> {
    get_controls_of(shared.inner())
}

pub fn get_controls_of(shared: &Shared) -> Result<Value, String> {
    Ok(controls_view(&lock(shared)))
}

/// `capture_binding({control})`: waits for the next newly pressed control of the active pad (SDL3, the pad the game
/// reads) and resolves with `{control, source}` (`xinput:A`, `xinput:LT`, in the controls-file names). It binds
/// nothing: the page checks for a conflict and then calls `assign_binding`. A control held when the wait starts does
/// not count. `cancel_capture` ends the wait with an error.
#[tauri::command]
pub async fn capture_binding(
    control: String,
    shared: State<'_, Shared>,
    pads: State<'_, pad_service::Handle>,
) -> Result<Value, String> {
    if !model::bindable(&control) {
        return Err(format!("'{control}' cannot be rebound"));
    }
    let shared: Shared = shared.inner().clone();
    lock(&shared).capturing = Some(control.clone());
    let answer = pads.inner().start_capture();
    let outcome = tauri::async_runtime::spawn_blocking(move || answer.recv()).await;
    {
        let mut state = lock(&shared);
        if state.capturing.as_deref() == Some(control.as_str()) {
            state.capturing = None;
        }
    }
    match outcome {
        Ok(Ok(Ok(source))) => Ok(json!({ "control": control, "source": source })),
        Ok(Ok(Err(message))) => Err(message),
        Ok(Err(_)) => Err(pad_service::CAPTURE_CANCELLED.to_string()),
        Err(error) => Err(error.to_string()),
    }
}

/// `cancel_capture()`: ends a running capture without binding anything.
#[tauri::command]
pub fn cancel_capture(shared: State<'_, Shared>, pads: State<'_, pad_service::Handle>) -> Result<(), String> {
    pads.inner().cancel_capture();
    let mut state = lock(shared.inner());
    state.capturing = None;
    state.controls_status.clear();
    Ok(())
}

/// `pad_status()`: the last `pad-status` payload (`available`, `connected`, `name`, `kind`, `player_index`, `count`,
/// `error`), for a page that opens after the event was sent.
#[tauri::command]
pub fn pad_status(pads: State<'_, pad_service::Handle>) -> Result<Value, String> {
    Ok(pads.inner().status())
}

/// `assign_binding({control, source, swap})`: binds `source` (`xinput:RB`, `key:Q`) to the button `control`, in place
/// of its source of the same family. When another button has the source, `swap: true` gives that button the previous
/// source; `swap: false` refuses. Writes the file at once and returns the `get_controls` view plus `swapped_with`
/// (the other button, or null) and `unchanged`.
#[tauri::command]
pub fn assign_binding(control: String, source: String, swap: bool, shared: State<'_, Shared>) -> Result<Value, String> {
    assign_binding_of(control, source, swap, shared.inner())
}

pub fn assign_binding_of(control: String, source: String, swap: bool, shared: &Shared) -> Result<Value, String> {
    let mut state = lock(shared);
    let assignment = change_controls(&mut state, |controls| {
        controls.assign(&control, &source, swap).map_err(|error| error.to_string())
    })?;
    state.controls_status = format!("{} is now bound to {}.", model::ps2_name(&control), model::describe_source(&source));
    let mut view = controls_view(&state);
    view["swapped_with"] = json!(assignment.swapped_with);
    view["unchanged"] = json!(assignment.unchanged);
    Ok(view)
}

/// `reset_binding({control})`: restores the built-in sources of one input.
#[tauri::command]
pub fn reset_binding(control: String, shared: State<'_, Shared>) -> Result<(), String> {
    reset_binding_of(control, shared.inner())
}

pub fn reset_binding_of(control: String, shared: &Shared) -> Result<(), String> {
    let mut state = lock(shared);
    let sources = default_bindings().get(&control).cloned().ok_or_else(|| format!("unknown input '{control}'"))?;
    change_controls(&mut state, |controls| controls.set_sources(&control, sources).map_err(|error| error.to_string()))?;
    state.controls_status = format!("{} is back to its built-in binding.", model::ps2_name(&control));
    Ok(())
}

/// `reset_all_bindings()`: every button, trigger and stick setting back to the built-in mapping. Returns the
/// `get_controls` view.
#[tauri::command]
pub fn reset_all_bindings(shared: State<'_, Shared>) -> Result<Value, String> {
    reset_all_bindings_of(shared.inner())
}

pub fn reset_all_bindings_of(shared: &Shared) -> Result<Value, String> {
    let mut state = lock(shared);
    change_controls(&mut state, |controls| {
        controls.reset_all();
        Ok(())
    })?;
    state.controls_status = "Every control is back to its built-in binding.".to_string();
    Ok(controls_view(&state))
}

// Saves and replays --------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryView {
    pub items: Vec<Item>,
    pub status: String,
    /// The folder that holds the page's files (`Documents\Gran Turismo 4\Saves` or `...\Replays`), shown with an
    /// "Open folder" button so a player can find and back up the files.
    pub folder: String,
}

fn saves_view(state: &AppState) -> Result<LibraryView, String> {
    Ok(LibraryView {
        items: saves::saves(&state.library)?,
        status: state.saves_status.clone(),
        folder: state.user_dirs.saves().display().to_string(),
    })
}

fn replays_view(state: &AppState) -> Result<LibraryView, String> {
    Ok(LibraryView {
        items: saves::replays(&state.library)?,
        status: state.replays_status.clone(),
        folder: state.user_dirs.replays().display().to_string(),
    })
}

#[tauri::command]
pub fn saves_list(shared: State<'_, Shared>) -> Result<LibraryView, String> {
    saves_list_of(shared.inner())
}

pub fn saves_list_of(shared: &Shared) -> Result<LibraryView, String> {
    saves_view(&lock(shared))
}

#[tauri::command]
pub fn replays_list(shared: State<'_, Shared>) -> Result<LibraryView, String> {
    replays_list_of(shared.inner())
}

pub fn replays_list_of(shared: &Shared) -> Result<LibraryView, String> {
    replays_view(&lock(shared))
}

/// Picks a save file or card image and imports it into the library. Imports never overwrite.
#[tauri::command]
pub async fn saves_import(app: AppHandle, shared: State<'_, Shared>) -> Result<Option<LibraryView>, String> {
    import_with_picker(app, shared.inner().clone(), Kind::Saves).await
}

#[tauri::command]
pub async fn replays_import(app: AppHandle, shared: State<'_, Shared>) -> Result<Option<LibraryView>, String> {
    import_with_picker(app, shared.inner().clone(), Kind::Replays).await
}

#[derive(Clone, Copy)]
pub enum Kind {
    Saves,
    Replays,
}

async fn import_with_picker(app: AppHandle, shared: Shared, kind: Kind) -> Result<Option<LibraryView>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let picked = app
            .dialog()
            .file()
            .set_title("Import from a memory card or save file")
            .add_filter("Save or card", &["psu", "ps2", "mcd", "bin", "card"])
            .blocking_pick_file();
        let Some(picked) = picked else {
            return Ok(None);
        };
        let path = picked.into_path().map_err(|error| error.to_string())?;
        import_into(&shared, &path, kind).map(Some)
    })
    .await
    .map_err(|error| error.to_string())?
}

/// The import behind `saves_import` and `replays_import` for a picked path. Imports never overwrite.
pub fn import_into(shared: &Shared, path: &Path, kind: Kind) -> Result<LibraryView, String> {
    let mut state = lock(shared);
    let summary = match saves::import_path(&state.library, path) {
        Ok(report) => summary_of(&report),
        Err(message) => message,
    };
    match kind {
        Kind::Saves => {
            state.saves_status = summary;
            saves_view(&state)
        }
        Kind::Replays => {
            state.replays_status = summary;
            replays_view(&state)
        }
    }
}

fn summary_of(report: &Report) -> String {
    saves::import_summary(report)
}

/// Makes a save the one the next start uses. The save that was active moves to a new slot.
#[tauri::command]
pub fn saves_activate(id: String, shared: State<'_, Shared>) -> Result<LibraryView, String> {
    saves_activate_of(id, shared.inner())
}

pub fn saves_activate_of(id: String, shared: &Shared) -> Result<LibraryView, String> {
    let name = id;
    let mut state = lock(shared);
    let slot = saves::saves(&state.library)?
        .into_iter()
        .find(|item| item.name == name)
        .map(|item| item.label)
        .ok_or_else(|| "That save is not in the library any more.".to_string())?;
    if slot == "Active save" {
        return saves_view(&state);
    }
    saves::activate(&state.library, &name)?;
    state.saves_status = format!("{} will be used the next time you play.", name);
    saves_view(&state)
}

/// Sends a save or replay to the Recycle Bin.
#[tauri::command]
pub fn saves_delete(id: String, shared: State<'_, Shared>) -> Result<LibraryView, String> {
    saves_delete_of(id, shared.inner())
}

pub fn saves_delete_of(id: String, shared: &Shared) -> Result<LibraryView, String> {
    let name = id;
    delete_item(&name, shared, Kind::Saves)
}

#[tauri::command]
pub fn replays_delete(id: String, shared: State<'_, Shared>) -> Result<LibraryView, String> {
    replays_delete_of(id, shared.inner())
}

pub fn replays_delete_of(id: String, shared: &Shared) -> Result<LibraryView, String> {
    let name = id;
    delete_item(&name, shared, Kind::Replays)
}

fn delete_item(name: &str, shared: &Shared, kind: Kind) -> Result<LibraryView, String> {
    let mut state = lock(shared);
    let items = saves::saves(&state.library)?.into_iter().chain(saves::replays(&state.library)?);
    let item = items
        .into_iter()
        .find(|item| item.name == name)
        .ok_or_else(|| "That item is not in the library any more.".to_string())?;
    let result = saves::delete(&state.library, &item);
    let status = match result {
        Ok(()) => format!("{} moved to the Recycle Bin.", item.label),
        Err(message) => message,
    };
    match kind {
        Kind::Saves => {
            state.saves_status = status;
            saves_view(&state)
        }
        Kind::Replays => {
            state.replays_status = status;
            replays_view(&state)
        }
    }
}

// About -----------------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutInfo {
    pub text: String,
    pub licenses: String,
    pub disc_path: String,
    /// The launcher version (GT4Recomp), the same as `gt4recomp`. Kept for the existing pages.
    pub version: String,
    /// GT4Recomp (launcher, game and installer) version.
    pub gt4recomp: String,
    /// The gt4-gs version in use, or `not installed`.
    pub gt4gs: String,
    /// Where the game files live: the copy of the disc image (`<data>\disc`). The picked file is not needed.
    pub game_files: String,
    /// The large data folder (`<install>\data`: disc copy, prepared files, renderer, logs).
    pub data_folder: String,
    /// The user data folder with the settings, saves and replays (`%APPDATA%\GT4Recomp`).
    pub saves_folder: String,
}

#[tauri::command]
pub fn about_info(shared: State<'_, Shared>) -> Result<AboutInfo, String> {
    about_info_of(shared.inner())
}

pub fn about_info_of(shared: &Shared) -> Result<AboutInfo, String> {
    let state = lock(shared);
    let renderer = gs_release::installed(&state.dirs.renderer_root())
        .map(|installed| installed.version.text())
        .unwrap_or_else(|| "not installed".to_string());
    Ok(AboutInfo {
        text: model::about_text(LAUNCHER_DISPLAY_VERSION),
        licenses: model::licenses_text().to_string(),
        disc_path: state.disc_path().map(|path| path.display().to_string()).unwrap_or_default(),
        version: LAUNCHER_DISPLAY_VERSION.to_string(),
        gt4recomp: LAUNCHER_DISPLAY_VERSION.to_string(),
        gt4gs: renderer,
        game_files: state.dirs.disc_folder().display().to_string(),
        data_folder: state.dirs.root().display().to_string(),
        saves_folder: state.user_dirs.root().display().to_string(),
    })
}

/// Opens the log folder in Explorer. The path is fixed by the launcher; nothing from the page runs.
#[tauri::command]
pub fn open_log_folder(shared: State<'_, Shared>) -> Result<(), String> {
    let logs = lock(shared.inner()).dirs.logs();
    open_folder(&logs)
}

/// Opens the game files folder (the copy of the disc image) in Explorer. The path is fixed by the launcher.
#[tauri::command]
pub fn open_game_files_folder(shared: State<'_, Shared>) -> Result<(), String> {
    let folder = lock(shared.inner()).dirs.disc_folder();
    open_folder(&folder)
}

/// Opens the user data folder (`%APPDATA%\GT4Recomp`, with Saves and Replays) in Explorer.
#[tauri::command]
pub fn open_player_folder(shared: State<'_, Shared>) -> Result<(), String> {
    let folder = lock(shared.inner()).user_dirs.root().to_path_buf();
    open_folder(&folder)
}

/// Opens the Saves folder of the user data folder (`%APPDATA%\GT4Recomp\Saves`) in Explorer.
#[tauri::command]
pub fn open_saves_folder(shared: State<'_, Shared>) -> Result<(), String> {
    let folder = lock(shared.inner()).user_dirs.saves();
    open_folder(&folder)
}

/// Opens the Replays folder of the user data folder (`%APPDATA%\GT4Recomp\Replays`) in Explorer.
#[tauri::command]
pub fn open_replays_folder(shared: State<'_, Shared>) -> Result<(), String> {
    let folder = lock(shared.inner()).user_dirs.replays();
    open_folder(&folder)
}

/// Opens the custom texture folder (`<install>\data\Textures`) in Explorer. It is made first, so the owner can
/// drop a pack in (TASK-088; TASK-093: the pack goes straight in, no replacements subfolder).
#[tauri::command]
pub fn open_textures_folder(shared: State<'_, Shared>) -> Result<(), String> {
    let folder = lock(shared.inner()).dirs.textures();
    paths::ensure_texture_folders(&folder).map_err(|error| error.to_string())?;
    open_folder(&folder)
}

fn open_folder(folder: &Path) -> Result<(), String> {
    fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    #[cfg(windows)]
    {
        std::process::Command::new("explorer.exe")
            .arg(&folder)
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("could not open {}: {error}", folder.display()))
    }
    #[cfg(not(windows))]
    {
        let _ = folder;
        Ok(())
    }
}

/// Keeps the setup helpers in one place for tests of the command layer.
pub fn data_dirs_for(root: &Path) -> DataDirs {
    DataDirs::new(root)
}

/// The settings a fresh launcher starts with (used by the command tests).
pub fn default_settings() -> Settings {
    Settings::default()
}

/// Controls file path for a data folder (used by the command tests).
pub fn controls_file_for(root: &Path) -> PathBuf {
    root.join(controls_file::CONTROLS_FILE_NAME)
}

// RetroAchievements -----------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RaStatusView {
    pub enabled: bool,
    pub hardcore: bool,
    pub encore: bool,
    pub signed_in: bool,
    pub username: String,
    pub display_name: String,
    pub score: u32,
    pub score_softcore: u32,
    pub game: Option<gt4l_core::retroachievements::RaGameOverview>,
}

fn ra_base_url() -> String {
    std::env::var("GT4_RA_HOST")
        .ok()
        .filter(|val| !val.trim().is_empty())
        .unwrap_or_else(|| gt4l_core::retroachievements::DEFAULT_RA_BASE_URL.to_string())
}

fn build_ra_status_view(
    cfg: &gt4l_core::retroachievements::RaConfig,
    game: Option<gt4l_core::retroachievements::RaGameOverview>,
) -> RaStatusView {
    RaStatusView {
        enabled: cfg.enabled,
        hardcore: cfg.hardcore,
        encore: cfg.encore,
        signed_in: cfg.is_signed_in(),
        username: cfg.username.clone(),
        display_name: if cfg.display_name.is_empty() {
            cfg.username.clone()
        } else {
            cfg.display_name.clone()
        },
        score: cfg.score,
        score_softcore: cfg.score_softcore,
        game,
    }
}

/// Returns the current RetroAchievements account state, mode toggles, and GT4 achievement overview.
#[tauri::command]
pub async fn ra_status(shared: State<'_, Shared>) -> Result<RaStatusView, String> {
    let shared_clone = shared.inner().clone();
    tauri::async_runtime::spawn_blocking(move || ra_status_of(&shared_clone))
        .await
        .map_err(|e| e.to_string())?
}

pub fn ra_status_of(shared: &Shared) -> Result<RaStatusView, String> {
    ra_status_with(shared, |cfg| {
        let base_url = ra_base_url();
        let session = gt4l_core::retroachievements::login_with_token(&base_url, &cfg.username, &cfg.token).ok();
        let game = gt4l_core::retroachievements::fetch_game_overview(&base_url, &cfg.username, &cfg.token).ok();
        (session, game)
    })
}

/// The refresh callback runs without the state lock. Tests can interleave a real logout/options command here.
pub(crate) fn ra_status_with(
    shared: &Shared,
    refresh: impl FnOnce(&gt4l_core::retroachievements::RaConfig) -> (
        Option<gt4l_core::retroachievements::RaAccountSession>,
        Option<gt4l_core::retroachievements::RaGameOverview>,
    ),
) -> Result<RaStatusView, String> {
    let (snapshot, revision) = {
        let state = lock(shared);
        let cfg = gt4l_core::retroachievements::RaConfig::load(&state.user_dirs.retroachievements_file());
        (cfg, state.ra_revision)
    };
    if !snapshot.is_signed_in() {
        return Ok(build_ra_status_view(&snapshot, None));
    }
    let (session, game) = refresh(&snapshot);
    let state = lock(shared);
    let path = state.user_dirs.retroachievements_file();
    let mut current = gt4l_core::retroachievements::RaConfig::load(&path);
    // A refresh may update scores, but may never restore an old account or overwrite newer options.
    if state.ra_revision != revision || current.username != snapshot.username || current.token != snapshot.token {
        return Ok(build_ra_status_view(&current, None));
    }
    if let Some(session) = session {
        current.display_name = session.display_name;
        current.score = session.score;
        current.score_softcore = session.score_softcore;
        current.save(&path)?;
    }
    Ok(build_ra_status_view(&current, game))
}

/// Signs in to RetroAchievements with username and password, storing the API token in `retroachievements.toml`.
#[tauri::command]
pub async fn ra_login(username: String, password: String, shared: State<'_, Shared>) -> Result<RaStatusView, String> {
    let shared_clone = shared.inner().clone();
    tauri::async_runtime::spawn_blocking(move || ra_login_of(&username, &password, &shared_clone))
        .await
        .map_err(|e| e.to_string())?
}

pub fn ra_login_of(username: &str, password: &str, shared: &Shared) -> Result<RaStatusView, String> {
    let revision = {
        let mut state = lock(shared);
        state.ra_revision = state.ra_revision.wrapping_add(1);
        state.ra_revision
    };
    let base_url = ra_base_url();
    let session = gt4l_core::retroachievements::login_with_password(&base_url, username, password)?;
    let (cfg, dirs) = {
        let state = lock(shared);
        let ra_path = state.user_dirs.retroachievements_file();
        let mut cfg = gt4l_core::retroachievements::RaConfig::load(&ra_path);
        if state.ra_revision != revision {
            return Ok(build_ra_status_view(&cfg, None));
        }
        cfg.username = session.username;
        cfg.display_name = session.display_name;
        cfg.token = session.token;
        cfg.score = session.score;
        cfg.score_softcore = session.score_softcore;
        cfg.enabled = true;
        cfg.save(&ra_path)?;
        (cfg, state.dirs.clone())
    };
    state::log(&dirs, &format!("retroachievements: signed in as {}", cfg.display_name));
    let game = gt4l_core::retroachievements::fetch_game_overview(&base_url, &cfg.username, &cfg.token).ok();
    let state = lock(shared);
    let current = gt4l_core::retroachievements::RaConfig::load(&state.user_dirs.retroachievements_file());
    Ok(build_ra_status_view(&current, if state.ra_revision == revision { game } else { None }))
}

/// Signs out of RetroAchievements and removes the stored API token from `retroachievements.toml`.
#[tauri::command]
pub fn ra_logout(shared: State<'_, Shared>) -> Result<RaStatusView, String> {
    ra_logout_of(shared.inner())
}

pub fn ra_logout_of(shared: &Shared) -> Result<RaStatusView, String> {
    let mut state = lock(shared);
    let ra_path = state.user_dirs.retroachievements_file();
    let mut cfg = gt4l_core::retroachievements::RaConfig::load(&ra_path);
    cfg.clear_credentials();
    cfg.save(&ra_path)?;
    state.ra_revision = state.ra_revision.wrapping_add(1);
    state::log(&state.dirs, "retroachievements: signed out");
    Ok(build_ra_status_view(&cfg, None))
}

/// Updates RetroAchievements mode options (`enabled`, `hardcore`, `encore`).
#[tauri::command]
pub fn ra_set_options(
    enabled: bool,
    hardcore: bool,
    encore: bool,
    shared: State<'_, Shared>,
) -> Result<RaStatusView, String> {
    ra_set_options_of(enabled, hardcore, encore, shared.inner())
}

pub fn ra_set_options_of(
    enabled: bool,
    hardcore: bool,
    encore: bool,
    shared: &Shared,
) -> Result<RaStatusView, String> {
    let mut state = lock(shared);
    let ra_path = state.user_dirs.retroachievements_file();
    let mut cfg = gt4l_core::retroachievements::RaConfig::load(&ra_path);
    cfg.enabled = enabled;
    cfg.hardcore = hardcore;
    cfg.encore = encore;
    cfg.save(&ra_path)?;
    state.ra_revision = state.ra_revision.wrapping_add(1);
    Ok(build_ra_status_view(&cfg, None))
}

