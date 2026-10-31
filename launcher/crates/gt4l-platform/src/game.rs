//! Starting, watching and stopping one game session. The launch line itself comes from
//! `gt4l_core::launch`; this module only runs it and turns the result into a classified outcome.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gt4l_core::display::DisplayTarget;
use gt4l_core::exit::{self, Outcome};
use gt4l_core::gs_release::Installed;
use gt4l_core::gs_table;
use gt4l_core::launch::{self, LaunchInputs, LaunchPlan, Pcsx2Paths};
use gt4l_core::lock::GameLock;
use gt4l_core::paths::DataDirs;
use gt4l_core::Settings;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// Sessions kept in the sessions folder; older ones are removed when a new one starts.
pub const KEEP_SESSIONS: usize = 20;

/// A running game: the child process (shared with the stop button) and where its log goes.
pub struct Running {
    pub child: Arc<Mutex<Option<Child>>>,
    pub session_dir: PathBuf,
    pub log_path: PathBuf,
    /// Held for the whole session so that a second launch cannot share the card (dropped with `Running`).
    #[allow(dead_code)]
    pub lock: GameLock,
}

/// Starts a session. The disc and the prepared ELF must already be verified by the caller.
/// `pcsx2` is the installed PCSX2 GS (decision 0070, the only renderer the launcher starts since TASK-096): the
/// session runs with `GT4_RENDERER=pcsx2` and writes its own gt4-gs.ini.
/// `display` is the resolved Monitor and Resolution (TASK-080 addendum): `GT4_MONITOR`, `GT4_OUTPUT_SIZE` and the
/// gt4-gs internal resolution follow it; `None` when no monitor could be read.
#[allow(clippy::too_many_arguments)]
pub fn start(
    dirs: &DataDirs,
    game_executable: &Path,
    elf: &Path,
    disc: &Path,
    settings: &Settings,
    pcsx2: &Installed,
    display: Option<&DisplayTarget>,
    textures: &Path,
    extra_environment: &[(String, String)],
) -> Result<Running, String> {
    let lock = GameLock::acquire(&dirs.runtime_card().with_file_name("launcher.lock"))
        .map_err(|error| error.to_string())?;
    let card = dirs.runtime_card();
    if let Some(parent) = card.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("runtime folder: {error}"))?;
    }
    let session_dir = dirs.session(&session_name());
    fs::create_dir_all(&session_dir).map_err(|error| format!("session folder: {error}"))?;
    prune_sessions(&dirs.sessions(), KEEP_SESSIONS);

    let inherited: Vec<String> = std::env::vars_os()
        .filter_map(|(name, _)| name.into_string().ok())
        .collect();
    let inputs = LaunchInputs {
        game_executable,
        elf,
        disc,
        card: &card,
        session_dir: &session_dir,
    };
    let paths = Pcsx2Paths::for_session(pcsx2, &dirs.root().join("gs"), &session_dir, textures);
    let text = launch::gs_config_text(settings, &paths, display, crate::system::integrated_gpu_name().is_some())?;
    gs_table::write_ini_atomic(&paths.config, &text).map_err(|error| format!("gt4-gs.ini: {error}"))?;
    let mut plan = launch::plan_pcsx2_launch(&inputs, settings, &paths, display, &inherited);
    plan.environment.extend(extra_environment.iter().cloned());
    let child = spawn(&plan)?;
    Ok(Running {
        child: Arc::new(Mutex::new(Some(child))),
        session_dir,
        log_path: plan.log_path.clone(),
        lock,
    })
}

fn spawn(plan: &LaunchPlan) -> Result<Child, String> {
    let log = File::create(&plan.log_path).map_err(|error| format!("game log: {error}"))?;
    let log_copy = log.try_clone().map_err(|error| format!("game log: {error}"))?;
    let mut command = Command::new(&plan.program);
    command
        .args(&plan.arguments)
        .current_dir(&plan.working_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_copy));
    for name in &plan.removed_inherited {
        command.env_remove(name);
    }
    for (name, value) in &plan.environment {
        command.env(name, value);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
        .spawn()
        .map_err(|error| format!("could not start {}: {error}", plan.program.display()))
}

/// Waits for the child to end. Returns the outcome once the process has exited and the log is read.
pub fn wait_for_exit(running: &Running) -> Outcome {
    let code = loop {
        let status = {
            let mut guard = match running.child.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            match guard.as_mut() {
                None => break None,
                Some(child) => child.try_wait(),
            }
        };
        match status {
            Ok(Some(status)) => break status.code(),
            Ok(None) => std::thread::sleep(Duration::from_millis(250)),
            Err(_) => break None,
        }
    };
    let log = fs::read_to_string(&running.log_path).unwrap_or_default();
    exit::classify(&log, code)
}

/// `session-<unix seconds>`; the seconds keep names unique for sessions started a second apart.
fn session_name() -> String {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map(|value| value.as_secs()).unwrap_or(0);
    format!("session-{seconds}")
}

fn prune_sessions(sessions: &Path, keep: usize) {
    let Ok(entries) = fs::read_dir(sessions) else {
        return;
    };
    let mut names: Vec<PathBuf> = entries.filter_map(|entry| entry.ok().map(|entry| entry.path())).collect();
    names.sort();
    if names.len() > keep {
        for old in &names[..names.len() - keep] {
            let _ = fs::remove_dir_all(old);
        }
    }
}
