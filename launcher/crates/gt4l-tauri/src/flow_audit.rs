//! The launcher flow, driven the way the page drives it (docs/launcher/FLOW-AUDIT.md): the same command bodies the
//! Tauri commands call, in the page's order, without a window.
//!
//! - `synthetic_flow_end_to_end` runs on every `cargo test` with a small synthetic disc (no game data) and a fake
//!   renderer installer.
//! - `real_flow_end_to_end` is ignored by default. It uses the owner's discs (read-only), copies the real image into
//!   a sandbox data folder and downloads the real gt4-gs release from GitHub:
//!     set GT4_AUDIT_DATA_DIR=<fresh scratch folder>
//!     set GT4_REAL_ISO=<Gran Turismo 4 (USA) (v2.00).iso>
//!     set GT4_SPEC2_ISO=<Gran Turismo 4 Spec II v1.10.iso>        (optional)
//!     set GT4_AUDIT_GAME_DIR=<folder with a stand-in gt4vulkan.exe>  (optional; Play is planned, never started)
//!     cargo test -p gt4l-tauri --release real_flow -- --ignored --nocapture
//!
//! Play is never started here: the test stops at the launch plan (program, arguments, environment).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gt4l_core::launch::{self, LaunchInputs};
use gt4l_core::paths::{DataDirs, UserDirs, UserRootSource, USER_FOLDER_NAME};
use gt4l_disc::testing::{build_iso, pins_for, synthetic_game, system_cnf};
use gt4l_disc::Pins;
use serde_json::{json, Value};

use crate::commands::{self, Kind};
use crate::setup::{self, Notify, RendererInstaller};
use crate::state::{lock, AppState, Shared};

/// Every event the page would have received, in order.
#[derive(Clone, Default)]
struct Recorder {
    events: Arc<Mutex<Vec<(String, Value)>>>,
}

impl Recorder {
    fn notify(&self) -> Notify {
        let events = self.events.clone();
        Arc::new(move |name: &str, payload: Value| events.lock().unwrap().push((name.to_string(), payload)))
    }

    fn names(&self, name: &str) -> Vec<Value> {
        self.events.lock().unwrap().iter().filter(|(event, _)| event == name).map(|(_, payload)| payload.clone()).collect()
    }
}

fn launch(dirs: &DataDirs, pins: &Pins) -> Shared {
    // A stand-in for %APPDATA%\GT4Recomp inside the sandbox: the flow never touches the real user folder. The
    // sandbox itself stands in for the install folder (the data folder is its `data`).
    let user = UserDirs::new(dirs.root().join("appdata-stand-in").join(USER_FOLDER_NAME), UserRootSource::Override);
    let install = dirs.root().parent().map(Path::to_path_buf).unwrap_or_else(|| dirs.root().to_path_buf());
    Arc::new(Mutex::new(AppState::load_with(dirs.clone(), pins.clone(), user, None, install).expect("state loads")))
}

fn screen(shared: &Shared) -> Value {
    commands::app_state_of(&lock(shared))
}

fn setup_state(shared: &Shared) -> String {
    screen(shared)["setup"]["state"].as_str().unwrap().to_string()
}

/// `setup_start`, then wait like the page's poll until the thread ends. Returns the states seen while polling.
fn run_setup(shared: &Shared, recorder: &Recorder, renderer: RendererInstaller) -> Vec<String> {
    let handle = setup::start_with(shared, recorder.notify(), renderer).expect("setup starts");
    let mut seen = Vec::new();
    while !handle.is_finished() {
        let view = screen(shared);
        let label = format!("{}:{}", view["setup"]["state"], view["setup"]["step"]);
        if seen.last() != Some(&label) {
            seen.push(label);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    handle.join().unwrap();
    seen
}

fn fake_renderer() -> RendererInstaller {
    Arc::new(|_: &DataDirs, progress: &mut dyn FnMut(gt4l_core::gs_release::Step)| {
        use gt4l_core::gs_release::Step;
        progress(Step::Listing);
        progress(Step::Downloading);
        progress(Step::Transfer { done: 50, total: Some(100) });
        progress(Step::Transfer { done: 100, total: Some(100) });
        progress(Step::Installing);
        Ok("1.0.1".to_string())
    })
}

fn offline_renderer() -> RendererInstaller {
    Arc::new(|_: &DataDirs, progress: &mut dyn FnMut(gt4l_core::gs_release::Step)| {
        progress(gt4l_core::gs_release::Step::Listing);
        Err("release list: offline".to_string())
    })
}

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("gt4l-flow-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

/// A synthetic memory card with one synthetic save (no game data).
fn synthetic_card(path: &Path) {
    use gt4saves::card::{DirEntry, Node, GAME_SAVE_NAME, MODE_DIR_DEFAULT, MODE_FILE_DEFAULT};
    let stamp = [0, 39, 50, 7, 12, 10, 0xEA, 0x07];
    let file = Node::file(DirEntry::new("icon.sys", MODE_FILE_DEFAULT, stamp, stamp).unwrap(), vec![3; 964]);
    let save = Node::dir(DirEntry::new(GAME_SAVE_NAME, MODE_DIR_DEFAULT, stamp, stamp).unwrap(), vec![file]);
    fs::write(path, gt4saves::Card::build(&[save]).unwrap().to_image()).unwrap();
}

/// The settings, saves, controls and about steps of the walk. Returns a short log.
fn walk_pages(shared: &Shared, card: &Path) -> Vec<String> {
    let mut log = Vec::new();
    let table = commands::get_settings_table_of(shared).expect("settings table");
    let rows = table["rows"].as_array().unwrap();
    assert!(rows.iter().any(|row| row["id"] == "renderer"), "renderer row");
    commands::set_setting_of("fxaa".into(), json!(true), shared).expect("fxaa on");
    let table = commands::get_settings_table_of(shared).unwrap();
    let fxaa = table["rows"].as_array().unwrap().iter().find(|row| row["id"] == "fxaa").unwrap().clone();
    assert_eq!(fxaa["current"], json!(true));
    assert!(commands::set_setting_of("no_such_setting".into(), json!(1), shared).is_err());
    let reset = commands::reset_gs_settings_to_defaults_of(shared).expect("reset");
    log.push(format!("settings: {} rows, fxaa set and read back, reset ok", reset["rows"].as_array().unwrap().len()));

    let before = commands::saves_list_of(shared).unwrap().items.len();
    synthetic_card(card);
    let after = commands::import_into(shared, card, Kind::Saves).expect("import");
    assert!(after.items.len() > before, "the card's save is in the library: {}", after.status);
    let again = commands::import_into(shared, card, Kind::Saves).expect("second import");
    assert_eq!(again.items.len(), after.items.len(), "imports never duplicate: {}", again.status);
    let replays = commands::replays_list_of(shared).unwrap();
    log.push(format!("saves: {} -> {} items ({}); replays: {}", before, after.items.len(), after.status, replays.items.len()));

    let controls = commands::get_controls_of(shared).expect("controls");
    assert!(controls["controls"].as_array().unwrap().len() >= 16);
    commands::reset_binding_of("cross".into(), shared).expect("reset cross");
    // A taken source is refused without swap; with swap both buttons change and the file follows at once.
    assert!(commands::assign_binding_of("cross".into(), "key:Q".into(), false, shared).is_err());
    let swapped = commands::assign_binding_of("cross".into(), "key:Q".into(), true, shared).expect("swap");
    assert_eq!(swapped["swapped_with"], json!("l1"));
    let file = fs::read_to_string(lock(shared).controls_path()).expect("controls file written");
    assert!(file.contains("cross = [\"xinput:A\", \"key:Q\"]") && file.contains("l1 = [\"xinput:LB\", \"key:W\"]"), "{file}");
    commands::set_setting_of("lstick_deadzone".into(), json!(10), shared).expect("stick deadzone");
    let tuned = fs::read_to_string(lock(shared).controls_path()).unwrap();
    assert!(tuned.contains("xinput:LX | deadzone=0.100"), "{tuned}");
    let reset = commands::reset_all_bindings_of(shared).expect("reset all");
    let cross = reset["controls"].as_array().unwrap().iter().find(|row| row["id"] == "cross").unwrap().clone();
    assert_eq!(cross["key"], json!("W"));
    log.push(format!("controls: {} inputs, cross reset, swap with l1, stick deadzone, reset all", controls["controls"].as_array().unwrap().len()));

    let about = commands::about_info_of(shared).expect("about");
    assert!(about.game_files.ends_with("disc"), "{}", about.game_files);
    log.push(format!("about: GT4Recomp {} gt4-gs {} files {}", about.gt4recomp, about.gt4gs, about.game_files));
    log
}

/// What Play would run, without running it.
fn play_plan(shared: &Shared, program_dir: &Path) -> Result<launch::LaunchPlan, String> {
    let inputs = commands::prepare_play(shared, program_dir)?;
    let session = inputs.dirs.session("session-plan-only");
    let card = inputs.dirs.runtime_card();
    let launch_inputs = LaunchInputs {
        game_executable: &inputs.executable,
        elf: &inputs.elf,
        disc: &inputs.disc,
        card: &card,
        session_dir: &session,
    };
    let inherited: Vec<String> = std::env::vars().map(|(name, _)| name).collect();
    let textures = crate::state::lock(shared).dirs.textures();
    let paths = launch::Pcsx2Paths::for_session(&inputs.pcsx2, &inputs.dirs.root().join("gs"), &session, &textures);
    Ok(launch::plan_pcsx2_launch(&launch_inputs, &inputs.settings, &paths, inputs.display.as_ref(), &inherited))
}

#[test]
fn synthetic_flow_end_to_end() {
    let root = scratch("synthetic");
    let data = root.join("data");
    let dirs = DataDirs::new(&data);
    let game = synthetic_game(0x0010_0000, 0x0010_0008, b"synthetic guest code, not game data, 0123456789abcdef");
    let image = |version: &str, serial_boot: &str| {
        build_iso(
            "GT4 USA",
            &[
                ("SYSTEM.CNF;1", system_cnf(serial_boot, version, "NTSC")),
                ("CORE.GT4;1", game.core.clone()),
                ("SCUS_973.28;1", vec![0xA5; 3000]),
            ],
        )
    };
    let good = image("2.00", "SCUS_973.28");
    let pins = pins_for(&good, &game, "SCUS-97328", "2.00");
    let picked = root.join("picked").join("Gran Turismo 4 (USA) (v2.00).iso");
    fs::create_dir_all(picked.parent().unwrap()).unwrap();
    fs::write(&picked, &good).unwrap();

    // First launch: Setup, never Home.
    let shared = launch(&dirs, &pins);
    assert_eq!(setup_state(&shared), "needed");

    // Refused files: not an ISO, a truncated copy, Spec II. The page shows the message and "Choose another file".
    let text = root.join("notes.iso");
    fs::write(&text, b"not a disc image").unwrap();
    assert!(setup::choose_disc_path(&shared, &text).is_err());
    assert_eq!(setup_state(&shared), "wrong_disc");
    let truncated = root.join("truncated.iso");
    fs::write(&truncated, &good[..good.len() - 2048]).unwrap();
    let message = setup::choose_disc_path(&shared, &truncated).unwrap_err();
    assert!(message.contains("incomplete"), "{message}");
    let spec = root.join("spec2.iso");
    fs::write(&spec, image("1.00", "SCUS_974.36")).unwrap();
    let message = setup::choose_disc_path(&shared, &spec).unwrap_err();
    assert!(message.starts_with("This disc is not supported yet."), "{message}");
    assert_eq!(screen(&shared)["setup"]["problem"], json!(message));
    // Setup cannot start before a disc was accepted.
    let refused = setup::start_with(&shared, Recorder::default().notify(), fake_renderer()).map(|_| ()).unwrap_err();
    assert_eq!(refused, "Choose your disc image first.");

    // Accept the good disc; the page then calls setup_start.
    let shared = launch(&dirs, &pins);
    let info = setup::choose_disc_path(&shared, &picked).expect("accepted");
    assert_eq!(info.name, "Gran Turismo 4 \u{b7} USA \u{b7} v2.00");
    let recorder = Recorder::default();
    let seen = run_setup(&shared, &recorder, fake_renderer());
    // The poll can catch the final state ("done") between the last state write and the thread's exit (it still sends
    // setup-finished), so only the last label may be the final one.
    let (last, before) = seen.split_last().expect("at least one label");
    assert!(before.iter().all(|label| label.starts_with("\"checking\"")), "the page shows setup until it ends: {seen:?}");
    assert!(last.starts_with("\"checking\"") || last.starts_with("\"done\""), "the page shows setup until it ends: {seen:?}");
    let view = screen(&shared);
    assert_eq!(view["setup"]["state"], "done", "{view}");
    assert_eq!(view["setup"]["note"], "");
    let copying: Vec<Value> = recorder.names("setup-progress").into_iter().filter(|event| event["step"] == "copying").collect();
    assert!(copying.iter().any(|event| event["percent"] == 100 && event["bytesDone"] == json!(good.len())), "{copying:?}");
    let renderer = recorder.names("renderer-progress");
    assert!(renderer.iter().any(|event| event["step"] == "downloading" && event["percent"] == 50), "{renderer:?}");
    assert!(renderer.iter().any(|event| event["step"] == "done"));
    assert_eq!(recorder.names("setup-finished"), vec![json!({ "ok": true, "message": "" })]);
    assert!(data.join("disc").join("SCUS-97328.iso").exists());

    // The picked file may go away: the next launch goes straight to Home, with no hash (well under the time a hash
    // of a real image takes).
    fs::remove_file(&picked).unwrap();
    let started = Instant::now();
    let second = launch(&dirs, &pins);
    assert_eq!(setup_state(&second), "done");
    assert!(started.elapsed() < Duration::from_secs(2));

    // Settings, saves, controls, about.
    let log = walk_pages(&second, &root.join("test-card.ps2"));
    assert_eq!(log.len(), 4);

    // Play: GSdx is not installed in this test, so Play refuses with the GSdx error and starts nothing. The stand-in
    // program is a text file. The GSdx launch line is covered by the launch contract tests.
    let program = root.join("program");
    fs::create_dir_all(program.join("game")).unwrap();
    fs::write(program.join("game").join("gt4vulkan.exe"), b"stand-in, never started").unwrap();
    let refusal = play_plan(&second, &program).unwrap_err();
    assert!(refusal.starts_with("GSdx could not run the game: GSdx is not installed yet"), "{refusal}");
    assert!(refusal.contains("log folder"), "{refusal}");

    // The player cleaned the data folder: Home is not shown, Play refuses, Setup offers to choose the disc again.
    fs::remove_file(data.join("disc").join("SCUS-97328.iso")).unwrap();
    let third = launch(&dirs, &pins);
    let view = screen(&third);
    assert_eq!(view["setup"]["state"], "files_missing", "{view}");
    assert!(view["setup"]["problem"].as_str().unwrap().contains("Choose your disc image again"));
    assert!(play_plan(&third, &program).unwrap_err().contains("Game files missing"));

    // Offline renderer: setup still finishes, with the note; Play then refuses until GSdx is installed.
    let picked_again = root.join("again.iso");
    fs::write(&picked_again, &good).unwrap();
    setup::choose_disc_path(&third, &picked_again).unwrap();
    run_setup(&third, &Recorder::default(), offline_renderer());
    let view = screen(&third);
    assert_eq!(view["setup"]["state"], "done");
    assert!(view["setup"]["note"].as_str().unwrap().contains("cannot start yet"));
    assert!(play_plan(&third, &root.join("program")).unwrap_err().contains("GSdx could not run the game"));

    // Cancel: back to a failed run with nothing half-written, and the earlier copy stays usable.
    let shared = launch(&dirs, &pins);
    setup::choose_disc_path(&shared, &picked_again).unwrap();
    // start_with clears the flag; Cancel is pressed right after the start, as the page's Cancel button would.
    let handle = setup::start_with(&shared, Recorder::default().notify(), fake_renderer()).unwrap();
    setup::cancel_with(&shared);
    handle.join().unwrap();
    let state = setup_state(&shared);
    assert!(state == "failed" || state == "done", "{state}");
    assert!(!data.join("disc").join("SCUS-97328.iso.tmp").exists());
    assert!(!data.join("prepared.tmp").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn settings_store_no_absolute_path_inside_the_install_folder() {
    // TASK-091: a copied install folder (another drive or PC) must still find what the settings name.
    let root = scratch("portable-settings");
    let dirs = DataDirs::new(root.join("data"));
    let game = synthetic_game(0x0010_0000, 0x0010_0008, b"synthetic guest code, not game data, 0123456789abcdef");
    let image = build_iso("GT4 USA", &[("SYSTEM.CNF;1", system_cnf("SCUS_973.28", "2.00", "NTSC")), ("CORE.GT4;1", game.core.clone())]);
    let pins = pins_for(&image, &game, "SCUS-97328", "2.00");
    let picked = root.join("images").join("gt4.iso");
    fs::create_dir_all(picked.parent().unwrap()).unwrap();
    fs::write(&picked, &image).unwrap();
    let shared = launch(&dirs, &pins);
    setup::choose_disc_path(&shared, &picked).expect("accepted");
    {
        let mut state = lock(&shared);
        state.config.dev_game_dir = Some(root.join("game-build"));
        state.save_config();
    }
    let file = lock(&shared).user_dirs.config_file();
    let text = fs::read_to_string(&file).unwrap();
    // TOML doubles each backslash, so the stored form of the sandbox path is checked too.
    let root_text = root.display().to_string().replace('\\', "\\\\");
    assert!(!text.contains(&root_text) && !text.contains(&root.display().to_string()), "{text}");
    assert!(text.contains("disc_path = \"images\\\\gt4.iso\""), "{text}");
    // The next start resolves it against the install folder again.
    let again = launch(&dirs, &pins);
    assert_eq!(lock(&again).config.disc_path.as_deref(), Some(picked.as_path()));
    assert_eq!(lock(&again).config.dev_game_dir.as_deref(), Some(root.join("game-build").as_path()));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn without_gsdx_setup_finishes_and_play_refuses_with_the_log_folder() {
    let root = scratch("gsdx-required");
    let dirs = DataDirs::new(root.join("data"));
    let game = synthetic_game(0x0010_0000, 0x0010_0008, b"synthetic guest code, not game data, 0123456789abcdef");
    let image = build_iso(
        "GT4 USA",
        &[
            ("SYSTEM.CNF;1", system_cnf("SCUS_973.28", "2.00", "NTSC")),
            ("CORE.GT4;1", game.core.clone()),
            ("SCUS_973.28;1", vec![0xA5; 3000]),
        ],
    );
    let pins = pins_for(&image, &game, "SCUS-97328", "2.00");
    let picked = root.join("disc.iso");
    fs::write(&picked, image).unwrap();
    let shared = launch(&dirs, &pins);
    setup::choose_disc_path(&shared, &picked).unwrap();
    let recorder = Recorder::default();
    run_setup(&shared, &recorder, offline_renderer());
    let view = screen(&shared);
    assert_eq!(view["setup"]["state"], "done");
    assert!(view["setup"]["note"].as_str().unwrap().contains("cannot start yet"));
    assert_eq!(recorder.names("setup-finished"), vec![json!({ "ok": true, "message": "" })]);

    let second = launch(&dirs, &pins);
    let program = root.join("program");
    fs::create_dir_all(program.join("game")).unwrap();
    fs::write(program.join("game").join("gt4vulkan.exe"), b"stand-in, never started").unwrap();
    let refusal = play_plan(&second, &program).unwrap_err();
    assert_eq!(
        refusal,
        launch::gsdx_unavailable_message(launch::GSDX_NOT_INSTALLED, &dirs.logs()),
        "the reason, what to do and the log folder"
    );
    assert!(refusal.contains("Check your internet connection and try again"), "{refusal}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn retroachievements_options_and_launch_environment() {
    let root = scratch("retroachievements");
    let dirs = DataDirs::new(root.join("data"));
    let game = synthetic_game(0x0010_0000, 0x0010_0008, b"synthetic guest code, not game data, 0123456789abcdef");
    let image = build_iso(
        "GT4 USA",
        &[
            ("SYSTEM.CNF;1", system_cnf("SCUS_973.28", "2.00", "NTSC")),
            ("CORE.GT4;1", game.core.clone()),
            ("SCUS_973.28;1", vec![0xA5; 3000]),
        ],
    );
    let pins = pins_for(&image, &game, "SCUS-97328", "2.00");
    let picked = root.join("disc.iso");
    fs::write(&picked, image).unwrap();
    let shared = launch(&dirs, &pins);
    setup::choose_disc_path(&shared, &picked).unwrap();
    run_setup(&shared, &Recorder::default(), fake_renderer());

    // Simulate a signed-in RetroAchievements profile in the user config folder.
    let ra_cfg = gt4l_core::retroachievements::RaConfig {
        enabled: true,
        username: "GranTurismoDriver".to_string(),
        display_name: "GranTurismoDriver".to_string(),
        token: "secret-ra-token".to_string(),
        hardcore: false,
        encore: false,
        score: 1500,
        score_softcore: 250,
    };
    ra_cfg.save(&lock(&shared).user_dirs.retroachievements_file()).unwrap();

    // Update options via command helper and verify persistence.
    let updated = commands::ra_set_options_of(true, true, true, &shared).expect("ra_set_options_of");
    assert!(updated.enabled);
    assert!(updated.signed_in);
    assert_eq!(updated.username, "GranTurismoDriver");
    assert!(updated.hardcore);
    assert!(updated.encore);

    // Verify prepare_play injects GT4_RA_* environment variables when GSdx is present.
    let gs_root = dirs.renderer_root();
    let gs_ver = gt4l_core::gs_release::MINIMUM_VERSION.text();
    let gs_ver_dir = gs_root.join(&gs_ver);
    fs::create_dir_all(&gs_ver_dir).unwrap();
    let gs_bytes = b"stand-in gt4-gs.exe";
    fs::write(gs_ver_dir.join(gt4l_core::gs_release::EXECUTABLE_NAME), gs_bytes).unwrap();
    fs::write(gs_ver_dir.join("sha256.txt"), gt4l_core::gs_release::sha256_hex(gs_bytes)).unwrap();
    fs::write(gs_root.join(gt4l_core::gs_release::CURRENT_FILE), &gs_ver).unwrap();

    let program = root.join("program");
    fs::create_dir_all(program.join("game")).unwrap();
    fs::write(program.join("game").join("gt4vulkan.exe"), b"stand-in gt4vulkan.exe").unwrap();

    let play_inputs = commands::prepare_play(&shared, &program).expect("prepare_play succeeds with stand-in binaries");
    assert!(play_inputs.extra.contains(&("GT4_RA_ENABLED".to_string(), "1".to_string())));
    assert!(play_inputs.extra.contains(&("GT4_RA_USERNAME".to_string(), "GranTurismoDriver".to_string())));
    assert!(play_inputs.extra.contains(&("GT4_RA_TOKEN".to_string(), "secret-ra-token".to_string())));
    assert!(play_inputs.extra.contains(&("GT4_RA_HARDCORE".to_string(), "1".to_string())));
    assert!(play_inputs.extra.contains(&("GT4_RA_ENCORE".to_string(), "1".to_string())));
    assert!(play_inputs.extra.contains(&(
        "GT4_RA_HASH".to_string(),
        gt4l_core::retroachievements::GT4_NTSC_U_RA_HASH.to_string()
    )));

    // Sign out clears token and disables launch env injection.
    let signed_out = commands::ra_logout_of(&shared).expect("ra_logout_of");
    assert!(!signed_out.signed_in);
    let play_inputs_after_logout = commands::prepare_play(&shared, &program).expect("prepare_play after logout");
    assert!(!play_inputs_after_logout.extra.iter().any(|(k, _)| k.starts_with("GT4_RA_")));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn retroachievements_refresh_cannot_restore_a_logged_out_account_or_old_options() {
    use gt4l_core::retroachievements::{RaAccountSession, RaConfig};
    let root = scratch("ra-refresh-race");
    let shared = launch(&DataDirs::new(root.join("data")), &Pins::embedded().unwrap());
    let path = lock(&shared).user_dirs.retroachievements_file();
    let original = RaConfig {
        username: "TestDriver".into(), token: "test-only-token".into(), ..RaConfig::default()
    };
    original.save(&path).unwrap();
    let session = || RaAccountSession {
        username: "TestDriver".into(), token: "test-only-token".into(), display_name: "TestDriver".into(),
        score: 42, score_softcore: 7,
    };

    let logged_out = commands::ra_status_with(&shared, |_| {
        // This executes inside the in-flight network seam, while no application lock is held.
        commands::ra_logout_of(&shared).unwrap();
        (Some(session()), None)
    }).unwrap();
    assert!(!logged_out.signed_in);
    assert!(!RaConfig::load(&path).is_signed_in(), "a late refresh must not put the token back on disk");

    original.save(&path).unwrap();
    let changed = commands::ra_status_with(&shared, |_| {
        commands::ra_set_options_of(false, true, true, &shared).unwrap();
        (Some(session()), None)
    }).unwrap();
    assert!(!changed.enabled);
    assert!(changed.hardcore && changed.encore);
    let saved = RaConfig::load(&path);
    assert!(!saved.enabled && saved.hardcore && saved.encore);
    assert_eq!(saved.score, 0, "discard the invalidated response instead of saving its account snapshot");

    let refreshed = commands::ra_status_with(&shared, |_| (Some(session()), None)).unwrap();
    assert_eq!(refreshed.score, 42);
    assert!(!refreshed.enabled && refreshed.hardcore && refreshed.encore);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn about_identifies_the_candidate_display_version() {
    let root = scratch("about-release-label");
    let shared = launch(&DataDirs::new(root.join("data")), &Pins::embedded().unwrap());
    let about = commands::about_info_of(&shared).unwrap();
    assert_eq!(about.version, gt4l_core::LAUNCHER_DISPLAY_VERSION);
    assert_eq!(about.gt4recomp, gt4l_core::LAUNCHER_DISPLAY_VERSION);
    assert!(about.text.contains(gt4l_core::LAUNCHER_DISPLAY_VERSION));
    let _ = fs::remove_dir_all(root);
}

#[test]
#[ignore = "real discs, a real copy of the image and a real download; see the module comment"]
fn real_flow_end_to_end() {
    let data = PathBuf::from(std::env::var("GT4_AUDIT_DATA_DIR").expect("set GT4_AUDIT_DATA_DIR"));
    let real = PathBuf::from(std::env::var("GT4_REAL_ISO").expect("set GT4_REAL_ISO"));
    let dirs = DataDirs::new(&data);
    let pins = Pins::embedded().unwrap();
    let mut report: Vec<String> = Vec::new();
    let mut note = |line: String| {
        println!("{line}");
        report.push(line);
    };

    let shared = launch(&dirs, &pins);
    note(format!("first launch: {}", screen(&shared)["setup"]));
    assert_ne!(setup_state(&shared), "done");

    let scratch = data.parent().unwrap().join("audit-inputs");
    fs::create_dir_all(&scratch).unwrap();
    let text = scratch.join("not-a-disc.iso");
    fs::write(&text, b"plain text, not a disc image").unwrap();
    note(format!("non-ISO: {}", setup::choose_disc_path(&shared, &text).unwrap_err()));
    let missing = scratch.join("missing.iso");
    note(format!("missing file: {}", setup::choose_disc_path(&shared, &missing).unwrap_err()));
    // A truncated copy: the first 64 MiB of the real image (holds SYSTEM.CNF and CORE.GT4, not the rest).
    let truncated = scratch.join("truncated.iso");
    {
        use std::io::{Read, Write};
        let mut input = fs::File::open(&real).unwrap().take(64 * 1024 * 1024);
        let mut output = fs::File::create(&truncated).unwrap();
        let mut buffer = Vec::new();
        input.read_to_end(&mut buffer).unwrap();
        output.write_all(&buffer).unwrap();
    }
    note(format!("truncated copy: {}", setup::choose_disc_path(&shared, &truncated).unwrap_err()));
    let _ = fs::remove_file(&truncated);
    if let Ok(spec) = std::env::var("GT4_SPEC2_ISO") {
        note(format!("Spec II: {}", setup::choose_disc_path(&shared, Path::new(&spec)).unwrap_err()));
        assert_eq!(setup_state(&shared), "wrong_disc");
    }

    let info = setup::choose_disc_path(&shared, &real).expect("the v2.00 disc is accepted");
    note(format!("accepted: {} ({})", info.name, info.serial));
    let free = gt4l_platform::disk::free_space(&dirs.disc_folder());
    note(format!("free space before the copy: {free:?} bytes"));
    let recorder = Recorder::default();
    let started = Instant::now();
    let seen = run_setup(&shared, &recorder, setup::github_renderer());
    note(format!("setup took {:.1} s; states seen: {seen:?}", started.elapsed().as_secs_f64()));
    let progress = recorder.names("setup-progress");
    note(format!("setup-progress events: {} (first {:?}, last {:?})", progress.len(), progress.first(), progress.last()));
    note(format!("renderer-progress events: {:?}", recorder.names("renderer-progress").into_iter().filter(|event| event["step"] != "downloading").collect::<Vec<_>>()));
    note(format!("setup-finished: {:?}", recorder.names("setup-finished")));
    let view = screen(&shared);
    note(format!("after setup: {}", view["setup"]));
    assert_eq!(view["setup"]["state"], "done");

    let started = Instant::now();
    let second = launch(&dirs, &pins);
    let state = setup_state(&second);
    note(format!("second launch: {state} in {} ms", started.elapsed().as_millis()));
    assert_eq!(state, "done");

    for line in walk_pages(&second, &scratch.join("test-card.ps2")) {
        note(line);
    }
    let check = crate::updates::check(&dirs, None, None);
    note(format!(
        "check_updates: {} offers, errors {:?}, gt4-gs installed {:?}",
        check.offers.len(),
        check.errors,
        check.renderer_installed.map(|version| version.text())
    ));
    if let Ok(game_dir) = std::env::var("GT4_AUDIT_GAME_DIR") {
        lock(&second).config.dev_game_dir = Some(PathBuf::from(game_dir));
        match play_plan(&second, Path::new(".")) {
            Ok(plan) => {
                note(format!("play program: {}", plan.program.display()));
                note(format!("play arguments: {:?}", plan.arguments));
                note(format!("play environment: {:?}", plan.environment));
                note(format!("play removes inherited: {:?}", plan.removed_inherited));
            }
            Err(message) => note(format!("play plan refused: {message}")),
        }
    }
    fs::write(data.parent().unwrap().join("real-flow-report.txt"), report.join("\n")).unwrap();
}
