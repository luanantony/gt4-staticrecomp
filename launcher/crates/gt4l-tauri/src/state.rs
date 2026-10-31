//! Launcher state shared by the commands: settings, controls, the save library and the running game.
//! The state lives behind one mutex. Long jobs (setup, the game, controller capture) run on their
//! own threads and lock the state only to read or write their results.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use gt4controls::{self as controls_file, Controls, CONTROLS_FILE_NAME};
use gt4l_core::config::{self, ConfigFile};
use gt4l_core::paths::{self, DataDirs, UserDirs};
use gt4l_disc::setup as disc_setup;
use gt4l_disc::Pins;
use gt4saves::library::Library;

use crate::saves;

pub struct AppState {
    pub dirs: DataDirs,
    pub pins: Pins,
    pub config: ConfigFile,
    pub controls: Controls,
    /// The user data in use: `%APPDATA%\GT4Recomp`, or the 1.2.0 places until their move has happened.
    pub user_dirs: UserDirs,
    /// The folder of `GT4Recomp.exe`: stored paths inside it are kept relative (portable copies).
    pub install_dir: PathBuf,
    /// The move of the 1.2.0 large data, while it runs and its result (TASK-091).
    pub migration: MigrationRun,
    pub library: Library,
    /// Set by the Cancel button of setup; the setup thread checks it between steps.
    pub setup_cancel: Arc<AtomicBool>,
    pub setup_busy: bool,
    /// Progress of the running setup (the page polls it through `app_state`).
    pub setup_run: SetupRun,
    /// How the last pick or setup run ended, until the next one starts.
    pub setup_outcome: SetupOutcome,
    /// The running game's child process (shared with the watcher thread and Stop).
    pub game_child: Option<Arc<Mutex<Option<Child>>>>,
    /// Control name while a rebind waits for a press from the pad service (`pad_service`).
    pub capturing: Option<String>,
    /// Message shown on Home, e.g. why the last start failed.
    pub home_message: String,
    /// Status line of the Controls page.
    pub controls_status: String,
    /// Result of the last import, activate or delete on the Saves page.
    pub saves_status: String,
    /// Result of the last import or delete on the Replays page.
    pub replays_status: String,
    /// True when the controls file was written by a newer launcher: it is kept as it is (never overwritten).
    pub controls_locked: bool,
    /// Invalidates in-flight RetroAchievements account work after any account/options mutation.
    pub ra_revision: u64,
}

pub type Shared = Arc<Mutex<AppState>>;

/// The background move of the 1.2.0 large data. The page polls it through `app_state` and shows a notice.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MigrationRun {
    pub running: bool,
    /// The unit being moved (`game files`, `gt4-gs`, `textures`, ...).
    pub item: String,
    pub bytes_done: u64,
    pub bytes_total: u64,
    /// The result for the player, shown once (empty when there is nothing to say).
    pub notice: String,
    /// Whether the notice reports a problem.
    pub problem: bool,
}

/// The step setup is on and how far it is. `step` is `copying`, `checking`, `preparing` or `renderer`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SetupRun {
    pub step: String,
    /// 0..1 of the current step.
    pub fraction: f32,
    /// Bytes done and total of a copy or a download (0 when not known).
    pub bytes_done: u64,
    pub bytes_total: u64,
    /// The renderer sub-step: `listing`, `downloading`, `installing`, `done` or `failed`.
    pub renderer_step: String,
}

/// The result of the last disc pick or setup run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SetupOutcome {
    #[default]
    None,
    /// The chosen file is not a supported disc (the page says "Not supported").
    Refused(String),
    /// The run stopped (cancel, no space, write error).
    Failed(String),
    /// The run finished. `renderer_note` is set when the PCSX2 GS could not be installed.
    Done { renderer_note: String },
}

/// Locks the state; a poisoned lock still gives its data (a panicked job must not hang the UI).
pub fn lock(shared: &Shared) -> MutexGuard<'_, AppState> {
    match shared.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Appends one line to the launcher log in the data folder.
pub fn log(dirs: &DataDirs, message: &str) {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map(|value| value.as_secs()).unwrap_or(0);
    let _ = std::fs::create_dir_all(dirs.logs());
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(dirs.logs().join("launcher.log")) {
        let _ = writeln!(file, "[{seconds}] {message}");
    }
}

impl AppState {
    /// Loads the settings and controls from the user data folder and opens the save library there.
    /// `pre_1_0_root` is the 1.2.0 `%LOCALAPPDATA%\GT4Recomp`, which may still hold a library of launchers before 1.0.0.
    pub fn load(dirs: DataDirs, user: UserDirs, pre_1_0_root: Option<PathBuf>) -> Result<AppState, String> {
        let pins = Pins::embedded()?;
        AppState::load_with(dirs, pins, user, pre_1_0_root, crate::updates::install_directory())
    }

    /// [`AppState::load`] with given pins, user folder and install folder (the flow test uses synthetic pins for a
    /// synthetic disc and scratch folders).
    pub fn load_with(dirs: DataDirs, pins: Pins, user: UserDirs, pre_1_0_root: Option<PathBuf>, install_dir: PathBuf) -> Result<AppState, String> {
        std::fs::create_dir_all(dirs.root()).map_err(|error| format!("data folder: {error}"))?;
        let (mut config, warnings) = config::load(&user.config_file());
        for warning in &warnings {
            log(&dirs, &format!("settings: {warning}"));
        }
        // Stored paths inside the install folder are relative (see `save_config`); resolve them here.
        config.disc_path = config.disc_path.map(|path| paths::resolve_portable(&path, &install_dir));
        config.dev_game_dir = config.dev_game_dir.map(|path| paths::resolve_portable(&path, &install_dir));
        config.textures_path = config.textures_path.map(|path| paths::resolve_portable(&path, &install_dir));
        if let Some(custom) = &config.textures_path {
            dirs.set_custom_textures(Some(custom.clone()));
            log(&dirs, &format!("textures: custom directory configured at {}", custom.display()));
        }
        let controls_path = user.controls_file();
        let mut controls_locked = false;
        let controls = match controls_file::load(&controls_path) {
            Ok(Some(controls)) => controls,
            Ok(None) => Controls::default(),
            Err(message) => {
                // A file from a newer launcher is kept as it is. Any other damaged file gets a copy before it is
                // replaced by the defaults (docs/launcher/MIGRATIONS.md).
                let text = fs::read_to_string(&controls_path).unwrap_or_default();
                if controls_schema_is_newer(&text) {
                    controls_locked = true;
                    log(&dirs, &format!("controls: {message}; the file is from a newer launcher and is kept"));
                } else if controls_path.exists() {
                    let copy = controls_path.with_file_name(format!("{CONTROLS_FILE_NAME}.bak-corrupt-{}", unix_seconds()));
                    let _ = fs::copy(&controls_path, &copy);
                    log(&dirs, &format!("controls: {message}; copy kept at {}", copy.display()));
                } else {
                    log(&dirs, &format!("controls: {message}; built-in bindings used"));
                }
                Controls::default()
            }
        };
        let opened = saves::open(&dirs, &user, pre_1_0_root.as_deref())?;
        for line in &opened.log {
            log(&dirs, line);
        }
        Ok(AppState {
            dirs,
            pins,
            config,
            controls,
            user_dirs: opened.user,
            install_dir,
            migration: MigrationRun::default(),
            library: opened.library,
            setup_cancel: Arc::new(AtomicBool::new(false)),
            setup_busy: false,
            setup_run: SetupRun::default(),
            setup_outcome: SetupOutcome::None,
            game_child: None,
            capturing: None,
            home_message: String::new(),
            controls_status: String::new(),
            saves_status: opened.notice.unwrap_or_default(),
            replays_status: String::new(),
            controls_locked,
            ra_revision: 0,
        })
    }

    /// The cheap check of the copied disc image and the prepared files (no hash; see `gt4l_disc::setup`).
    pub fn installed(&self) -> disc_setup::Installed {
        disc_setup::check_installed(&self.dirs.prepared(), &self.dirs.disc_folder(), &self.pins)
    }

    /// Home is ready when setup is not running and the copied image and prepared files pass the cheap check.
    pub fn home_ready(&self) -> bool {
        !self.setup_busy && matches!(self.installed(), disc_setup::Installed::Ready { .. })
    }

    /// Writes the settings to the user data folder. A path inside the install folder is stored relative to it, so a
    /// copied install folder (another drive or PC) still finds it; no absolute data path is stored.
    pub fn save_config(&self) {
        let mut stored = self.config.clone();
        stored.disc_path = stored.disc_path.map(|path| paths::portable_path(&path, &self.install_dir));
        stored.dev_game_dir = stored.dev_game_dir.map(|path| paths::portable_path(&path, &self.install_dir));
        stored.textures_path = stored.textures_path.map(|path| paths::portable_path(&path, &self.install_dir));
        if let Err(error) = config::save(&self.user_dirs.config_file(), &stored) {
            log(&self.dirs, &format!("settings not saved: {error}"));
        }
    }

    /// The controls file. Commands write it through `commands::change_controls`, which refuses while
    /// `controls_locked` is set and keeps the change only when the write succeeded.
    pub fn controls_path(&self) -> PathBuf {
        self.user_dirs.controls_file()
    }

    pub fn disc_path(&self) -> Option<&Path> {
        self.config.disc_path.as_deref()
    }
}

/// Whether a controls file declares a schema above the one this launcher reads (`schema = 1`).
fn controls_schema_is_newer(text: &str) -> bool {
    text.parse::<toml::Table>()
        .ok()
        .and_then(|table| table.get("schema").and_then(|value| value.as_integer()))
        .is_some_and(|schema| schema > 1)
}

fn unix_seconds() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|value| value.as_secs()).unwrap_or(0)
}
