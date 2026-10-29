//! Builds the exact command line and environment for `gt4vulkan.exe`.
//!
//! Contract (from `scripts/external_runtime_probe.py::run_probe` and `gs_stream_environment`):
//! - arguments: `<elf> --ntsc --disc <iso> --frames 0 --memcard <card> --misses <session>\misses.txt --window`
//! - the working directory is the session folder;
//! - inherited `GT4_*` and `PS2_*` variables are removed, then only the variables below are set.
//!
//! The renderer is the PCSX2 GS (`GT4_RENDERER=pcsx2`, the downloaded `gt4-gs.exe` in its own process, see
//! [`pcsx2_environment`], decision 0070). CoreGS 1.1 is still inside the game executable, but the launcher never
//! starts it (TASK-096, owner 2026-10-22). Only the child process gets these variables. Nothing is written to the
//! user or system environment.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::display::{self, DisplayTarget};
use crate::gs_release::Installed;
use crate::gs_table;
use crate::settings::Settings;

/// Inherited variables with these prefixes are removed before the game starts.
pub const SCRUBBED_PREFIXES: &[&str] = &["GT4_", "PS2_"];

/// Printed by the game when the player asked for "restart with the built-in renderer" after a GS process failure.
/// The launcher no longer starts CoreGS 1.1 (TASK-096): it ends the session and shows the GSdx error instead.
pub const GS_RESTART_MARKER: &str = "[gs-ipc] restart requested: renderer donor (CoreGS 1.1)";

/// The reason shown when the PCSX2 GS (GSdx) is not installed or its files fail verification.
pub const GSDX_NOT_INSTALLED: &str = "GSdx is not installed yet";

/// The reason shown when GSdx stopped and asked for a restart with CoreGS 1.1.
pub const GSDX_RESTART_REQUESTED: &str = "GSdx stopped while playing";

/// Everything needed to start one game session.
#[derive(Clone, Debug)]
pub struct LaunchInputs<'a> {
    /// `gt4vulkan.exe`.
    pub game_executable: &'a Path,
    /// The rebuilt ELF in the prepared folder. Its file name must stay `SCUS_973.28` (argv0 rule).
    pub elf: &'a Path,
    pub disc: &'a Path,
    pub card: &'a Path,
    /// A new session folder; it becomes the working directory and receives `misses.txt` and `game.log`.
    pub session_dir: &'a Path,
}

/// The planned launch. `removed_inherited` lists variables to remove from the child environment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchPlan {
    pub program: PathBuf,
    pub arguments: Vec<String>,
    pub environment: Vec<(String, String)>,
    pub removed_inherited: Vec<String>,
    pub working_directory: PathBuf,
    pub log_path: PathBuf,
    pub misses_path: PathBuf,
}

/// The files and pins of the PCSX2 GS process for one session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pcsx2Paths {
    /// `gt4-gs.exe` in the installed version folder.
    pub executable: PathBuf,
    /// SHA-256 of that file (`GT4_GS_PROCESS_SHA256`, checked by the game).
    pub executable_sha256: String,
    /// `gt4-gs.ini` written for this session.
    pub config: PathBuf,
    /// Shader and pipeline cache folder (`%LOCALAPPDATA%\GT4Recomp\gs\cache`).
    pub cache: PathBuf,
    /// Data folder of the GS process (`%LOCALAPPDATA%\GT4Recomp\gs\data`).
    pub data_root: PathBuf,
    /// The custom texture folder, `Documents\Gran Turismo 4\Textures` (TASK-088, gt4-gs `[Folders] Textures`).
    pub textures: PathBuf,
}

impl Pcsx2Paths {
    /// The paths of an installed renderer, with the cache and data folders under the renderer root.
    pub fn for_session(installed: &Installed, renderer_root: &Path, session_dir: &Path, textures: &Path) -> Pcsx2Paths {
        if let Some(parent) = installed.executable.parent() {
            crate::gs_release::ensure_portable_marker(parent);
        }
        Pcsx2Paths {
            executable: installed.executable.clone(),
            executable_sha256: installed.executable_sha256.clone(),
            config: session_dir.join("gt4-gs.ini"),
            cache: renderer_root.join("cache"),
            data_root: renderer_root.join("data"),
            textures: textures.to_path_buf(),
        }
    }
}

/// Chooses the renderer of a session. Only GSdx runs the game (owner, 2026-10-22): when it is not installed, or its
/// files failed verification, the session does not start and the error says why and where the log is. There is no
/// CoreGS 1.1 fallback.
pub fn choose_renderer<'a>(installed: Option<&'a Installed>, log_folder: &Path) -> Result<&'a Installed, String> {
    installed.ok_or_else(|| gsdx_unavailable_message(GSDX_NOT_INSTALLED, log_folder))
}

/// The message of a session GSdx cannot run: the reason, what to do, and the log folder. Shown on the home page and in
/// the tray, so the player is never left without a reason.
pub fn gsdx_unavailable_message(reason: &str, log_folder: &Path) -> String {
    format!(
        "GSdx could not run the game: {reason}. Check your internet connection and try again. Details are in the log folder: {}.",
        log_folder.display()
    )
}

/// The variables of a PCSX2 GS session: `GT4_RENDERER=pcsx2`, the process and its pin, the session INI, the
/// folders, and the fixed 480p and 16:9 holds (see `add_common`). Readback is accurate unless the table row
/// `performance_mode` (Performance Mode, TASK-091) is On; it is written into the INI, so no variable is needed.
pub fn pcsx2_environment(settings: &Settings, paths: &Pcsx2Paths) -> Vec<(String, String)> {
    let mut variables: BTreeMap<&'static str, String> = BTreeMap::new();
    variables.insert("GT4_RENDERER", "pcsx2".to_string());
    variables.insert("GT4_GS_PROCESS", paths.executable.display().to_string());
    variables.insert("GT4_GS_PROCESS_SHA256", paths.executable_sha256.clone());
    variables.insert("GT4_GS_CONFIG", paths.config.display().to_string());
    variables.insert("PS2_GS", "gpu".to_string());
    add_common(&mut variables, settings);
    variables.into_iter().map(|(name, value)| (name.to_string(), value)).collect()
}

/// The variables of a GSdx session: the fixed 480p and 16:9 holds, display mode, present mode, async GS and
/// diagnostics.
fn add_common(variables: &mut BTreeMap<&'static str, String>, settings: &Settings) {
    // Fixed (owner, 2026-10-21): the game always runs at 480p with its own 16:9. No setting changes these, and no stored
    // value of the retired `input_resolution` or `widescreen` settings is read (settings::RETIRED_SETTINGS).
    variables.insert("GT4_FORCE_480P", "1".to_string());
    variables.insert("GT4_FORCE_WIDESCREEN", "1".to_string());
    variables.insert("GT4_FULLSCREEN", flag(settings.is_on("fullscreen")));
    variables.insert("GT4_DISPLAY_ASPECT", "auto".to_string());
    variables.insert(
        "GT4_PRESENT_MODE",
        match settings.get("vsync") {
            "fast" => "mailbox",
            "off" => "immediate",
            _ => "fifo",
        }
        .to_string(),
    );
    if !settings.is_on("async_gs") {
        variables.insert("GT4_GS_SYNC", "1".to_string());
    }
    if settings.is_on("performance_report") {
        variables.insert("GT4_PERF", "1".to_string());
    }
    match settings.get("check_vu_code") {
        "100" => {
            variables.insert("GT4_VU1_VERIFY", "100".to_string());
            variables.insert("GT4_VU0_VERIFY", "100".to_string());
        }
        "1000" => {
            variables.insert("GT4_VU1_VERIFY", "1000".to_string());
            variables.insert("GT4_VU0_VERIFY", "1000".to_string());
        }
        _ => {}
    }
}

/// `GT4_MONITOR` (the monitor's device name) and `GT4_OUTPUT_SIZE` (`WxH`) for a resolved display target; nothing
/// when no monitor is known (the game then opens on the primary monitor at its desktop size).
pub fn display_environment(display: Option<&DisplayTarget>) -> Vec<(String, String)> {
    match display {
        Some(target) => vec![
            ("GT4_MONITOR".to_string(), target.device.clone()),
            ("GT4_OUTPUT_SIZE".to_string(), target.output.text()),
        ],
        None => Vec::new(),
    }
}

/// The render plan of a session (TASK-080 addendum): the output size of the display target (1920 x 1080 when none is
/// known) and the Render scale, for the PCSX2 GS. Never fails: an output the plan refuses falls back to 1920 x 1080.
pub fn session_plan(settings: &Settings, display: Option<&DisplayTarget>) -> display::DisplayPlan {
    let scale = settings.get("render_scale").parse::<u32>().unwrap_or(display::RENDER_SCALE_DEFAULT);
    let output = display.map(|target| target.output).unwrap_or(display::FALLBACK_OUTPUT);
    let fallback = || {
        display::display_plan(
            display::FALLBACK_OUTPUT.width,
            display::FALLBACK_OUTPUT.height,
            display::RENDER_SCALE_DEFAULT,
            display::PCSX2_GS_CAPS,
        )
        .expect("1920x1080 at 100% is a valid plan")
    };
    display::display_plan(output.width, output.height, scale, display::PCSX2_GS_CAPS).unwrap_or_else(|_| fallback())
}

/// The gt4-gs.ini text of a session, rendered from the table (`gs_table::render_ini`, same rules as
/// scripts/gs_settings.py). The vsync and mailbox rows follow the present mode the way the probe does:
/// fifo writes both true, mailbox writes mailbox false, immediate writes vsync false. `upscale_multiplier` comes from
/// the session plan ([`session_plan`]); a stored `upscale` value is never used (there is no such setting). With no
/// display known and Render scale 100% the plan gives 3x, the table default, so that INI is unchanged.
pub fn gs_config_text(settings: &Settings, paths: &Pcsx2Paths, display: Option<&DisplayTarget>) -> Result<String, String> {
    let mut user: BTreeMap<String, String> = settings.gs_overrides().clone();
    user.insert("upscale".to_string(), session_plan(settings, display).multiplier.to_string());
    let present = match settings.get("vsync") {
        "fast" => "mailbox",
        "off" => "immediate",
        _ => "fifo",
    };
    let vsync = if present == "immediate" { "false" } else { "true" };
    let mailbox = if present == "mailbox" { "false" } else { "true" };
    user.entry("vsync".to_string()).or_insert_with(|| vsync.to_string());
    user.entry("mailbox".to_string()).or_insert_with(|| mailbox.to_string());
    let mut launcher: BTreeMap<String, String> = BTreeMap::new();
    launcher.insert("cache_folder".to_string(), paths.cache.display().to_string());
    launcher.insert("data_root".to_string(), paths.data_root.display().to_string());
    launcher.insert("textures_folder".to_string(), paths.textures.display().to_string());
    gs_table::render_ini(&user, &launcher).map_err(|error| error.to_string())
}

/// Whether the game asked for a restart with the built-in renderer (its log holds the marker line). The launcher
/// answers with the GSdx error, not with a restart (TASK-096).
pub fn restart_requested(log_text: &str) -> bool {
    log_text.lines().any(|line| line.contains(GS_RESTART_MARKER))
}

fn flag(on: bool) -> String {
    if on { "1" } else { "0" }.to_string()
}

/// Inherited variables that are passed through to the game although they carry a scrubbed prefix. Each one is a
/// hidden owner test knob that the game validates itself (an invalid value refuses the start):
/// - `GT4_ASPECT_FOV_EXTRA` (TASK-080 owner test T1, 0.5..3.0, default 1.0): an extra horizontal FOV factor on top of
///   the geometry-correct Hor+ widening, to compare roundness. It is not a setting and no row writes it.
pub const PASSED_THROUGH: &[&str] = &["GT4_ASPECT_FOV_EXTRA"];

/// Whether an inherited variable must be removed for the game process.
pub fn is_scrubbed(name: &str) -> bool {
    !PASSED_THROUGH.contains(&name) && SCRUBBED_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
}

/// Plans one PCSX2 GS launch. Pure: the caller writes `paths.config` (see [`gs_config_text`]) before starting.
pub fn plan_pcsx2_launch(
    inputs: &LaunchInputs<'_>,
    settings: &Settings,
    paths: &Pcsx2Paths,
    display: Option<&DisplayTarget>,
    inherited_names: &[String],
) -> LaunchPlan {
    plan_with(inputs, with_display(pcsx2_environment(settings, paths), display), inherited_names)
}

fn with_display(mut environment: Vec<(String, String)>, display: Option<&DisplayTarget>) -> Vec<(String, String)> {
    environment.extend(display_environment(display));
    environment.sort();
    environment
}

fn plan_with(inputs: &LaunchInputs<'_>, environment: Vec<(String, String)>, inherited_names: &[String]) -> LaunchPlan {
    let misses = inputs.session_dir.join("misses.txt");
    let arguments = vec![
        inputs.elf.display().to_string(),
        "--ntsc".to_string(),
        "--disc".to_string(),
        inputs.disc.display().to_string(),
        "--frames".to_string(),
        "0".to_string(),
        "--memcard".to_string(),
        inputs.card.display().to_string(),
        "--misses".to_string(),
        misses.display().to_string(),
        "--window".to_string(),
    ];
    let mut removed: Vec<String> = inherited_names.iter().filter(|name| is_scrubbed(name)).cloned().collect();
    removed.sort();
    removed.dedup();
    LaunchPlan {
        program: inputs.game_executable.to_path_buf(),
        arguments,
        environment,
        removed_inherited: removed,
        working_directory: inputs.session_dir.to_path_buf(),
        log_path: inputs.session_dir.join("game.log"),
        misses_path: misses,
    }
}
