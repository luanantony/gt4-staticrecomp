//! Launch contract tests: the argument and environment construction is compared with what
//! `scripts/play_performance.py --mode main --force-480p` sends through `external_runtime_probe.run_probe`.
//!
//! Python reference (read from the scripts, not run): `run_probe` builds
//!   arguments = [exe, <session>/SCUS_973.28, --ntsc, --disc, <iso>, --frames, 0,
//!                --memcard, <card>, --misses, <session>/misses.txt, --window]
//! and, for performance profiles, `GT4_PERF=1 PS2_SCALE=1 PS2_GS=gpu`; `--force-480p` adds
//! `GT4_FORCE_480P=1`. The launcher matches that except where the approved design says otherwise:
//! `GT4_PERF` is only set when the Performance report is on (default off), and `GT4_FULLSCREEN` and
//! `GT4_PRESENT_MODE` are always stated. Those two differences are asserted explicitly below.

use std::path::{Path, PathBuf};

use gt4l_core::display::{DisplayTarget, Size};
use gt4l_core::launch::{self, LaunchInputs};
use gt4l_core::settings::{self, Settings};

fn env_map(pairs: &[(String, String)]) -> std::collections::BTreeMap<String, String> {
    pairs.iter().cloned().collect()
}

/// The variables of a GSdx session (the only renderer since TASK-096).
fn game_environment(settings: &Settings) -> Vec<(String, String)> {
    launch::pcsx2_environment(settings, &pcsx2_paths())
}

/// The launch plan of a GSdx session.
fn plan_launch(
    inputs: &LaunchInputs<'_>,
    settings: &Settings,
    display: Option<&DisplayTarget>,
    inherited_names: &[String],
) -> launch::LaunchPlan {
    launch::plan_pcsx2_launch(inputs, settings, &pcsx2_paths(), display, inherited_names)
}

#[test]
fn default_settings_send_the_main_profile_variables_plus_the_launcher_defaults() {
    let environment = env_map(&game_environment(&Settings::default()));
    // Same as `--mode main --force-480p`: the platform variables and the 480p hold.
    assert_eq!(environment["PS2_GS"], "gpu");
    assert_eq!(environment["GT4_FORCE_480P"], "1");
    assert_eq!(environment["GT4_RENDERER"], "pcsx2");
    // Differences with the Python launcher, approved in LAUNCHER-PLAN section 6:
    assert!(!environment.contains_key("GT4_PERF"), "Performance report is off by default");
    assert_eq!(environment["GT4_FULLSCREEN"], "1");
    // 16:9 is fixed (owner, 2026-10-21): GT4_FORCE_WIDESCREEN=1 is always stated; there is no setting.
    assert_eq!(environment["GT4_FORCE_WIDESCREEN"], "1");
    // TASK-080 addendum: the aspect is always Auto (the output size's shape); there is no setting.
    assert_eq!(environment["GT4_DISPLAY_ASPECT"], "auto");
    assert!(!environment.contains_key("GT4_RACE_DISPLAY"), "480p is fixed: no 480i or 1080i path");
    assert_eq!(environment["GT4_PRESENT_MODE"], "fifo");
    // Asynchronous GS is the default, so GT4_GS_SYNC stays unset.
    assert!(!environment.contains_key("GT4_GS_SYNC"));
    // Nothing paraLLEl-GS specific, and no CoreGS 1.1 variable (TASK-096: the launcher never starts CoreGS 1.1).
    assert!(!environment.keys().any(|name| name.starts_with("GT4_PGS_")));
    for gone in ["PS2_SCALE", "GT4_DEINTERLACE", "GT4_AA", "GT4_PRESENT_FILTER"] {
        assert!(!environment.contains_key(gone), "{gone} is a CoreGS 1.1 variable");
    }
    assert_eq!(environment.len(), 10, "the GSdx variables: renderer, process pin, config, PS2_GS and the common holds");
}

#[test]
fn argument_list_matches_run_probe_order() {
    let session = Path::new(r"C:\Users\me\AppData\Local\GT4Recomp\sessions\session-1");
    let inputs = LaunchInputs {
        game_executable: Path::new(r"C:\Program Files\GT4Recomp\game\gt4vulkan.exe"),
        elf: Path::new(r"C:\Users\me\AppData\Local\GT4Recomp\prepared\SCUS_973.28"),
        disc: Path::new(r"D:\Games\GT4 (USA) (v2.00).iso"),
        card: Path::new(r"C:\Users\me\AppData\Local\GT4Recomp\runtime\card.ps2"),
        session_dir: session,
    };
    let plan = plan_launch(&inputs, &Settings::default(), None, &[]);
    assert_eq!(
        plan.arguments,
        vec![
            r"C:\Users\me\AppData\Local\GT4Recomp\prepared\SCUS_973.28",
            "--ntsc",
            "--disc",
            r"D:\Games\GT4 (USA) (v2.00).iso",
            "--frames",
            "0",
            "--memcard",
            r"C:\Users\me\AppData\Local\GT4Recomp\runtime\card.ps2",
            "--misses",
            r"C:\Users\me\AppData\Local\GT4Recomp\sessions\session-1\misses.txt",
            "--window",
        ]
    );
    assert_eq!(plan.working_directory, session);
    assert_eq!(plan.log_path, session.join("game.log"));
    assert_eq!(plan.program, Path::new(r"C:\Program Files\GT4Recomp\game\gt4vulkan.exe"));
}

#[test]
fn inherited_gt4_and_ps2_variables_are_scrubbed_and_others_kept() {
    let inputs = LaunchInputs {
        game_executable: Path::new("gt4vulkan.exe"),
        elf: Path::new("SCUS_973.28"),
        disc: Path::new("disc.iso"),
        card: Path::new("card.ps2"),
        session_dir: Path::new("session"),
    };
    let inherited = vec![
        "GT4_PERF".to_string(),
        "PS2_DUMP_AUDIO".to_string(),
        "PATH".to_string(),
        "VK_LAYER_PATH".to_string(),
        "GT4_ASPECT_FOV_EXTRA".to_string(),
        "GT4_PACING".to_string(),
        "GT4_AUDIO_LATENCY_MS".to_string(),
        "GT4_DISPLAY_ASPECT".to_string(),
    ];
    let plan = plan_launch(&inputs, &Settings::default(), None, &inherited);
    // TASK-080 T1 & TASK-138: FOV factor, pacing and audio latency pass through; the aspect itself is always auto.
    assert_eq!(plan.removed_inherited, vec!["GT4_DISPLAY_ASPECT", "GT4_PERF", "PS2_DUMP_AUDIO"]);
    assert!(!plan.removed_inherited.iter().any(|name| name == "PATH" || name == "VK_LAYER_PATH"));
    assert!(!plan.removed_inherited.iter().any(|name| name == "GT4_PACING" || name == "GT4_AUDIO_LATENCY_MS"));
}

#[test]
fn settings_map_to_their_variables() {
    let mut settings = Settings::default();
    settings.set("vsync", "fast").unwrap();
    settings.set("async_gs", "off").unwrap();
    settings.set("performance_report", "on").unwrap();
    settings.set("check_vu_code", "1000").unwrap();
    let environment = env_map(&game_environment(&settings));
    assert_eq!(environment["GT4_FORCE_480P"], "1", "480p is fixed whatever the other settings");
    assert_eq!(environment["GT4_PRESENT_MODE"], "mailbox");
    assert_eq!(environment["GT4_GS_SYNC"], "1");
    assert_eq!(environment["GT4_PERF"], "1");
    assert_eq!(environment["GT4_VU1_VERIFY"], "1000");
    assert_eq!(environment["GT4_VU0_VERIFY"], "1000");

    assert_eq!(environment["GT4_FORCE_WIDESCREEN"], "1", "16:9 is fixed whatever the other settings");
    // TASK-080 addendum: no aspect or internal-resolution setting exists any more.
    for gone in ["aspect_ratio", "aspect_custom"] {
        assert!(settings.set(gone, "auto").is_err(), "{gone} is not a setting");
    }
    assert!(settings.set_gs("upscale", "4K").is_err(), "the internal resolution is derived, not set");
    for (id, good, bad) in [
        ("render_scale", "150", "110"),
        ("resolution", "2560x1080", "2560*1080"),
        ("monitor", r"\\.\DISPLAY2@3440x1440", "DISPLAY2"),
    ] {
        settings.set(id, good).unwrap();
        assert!(settings.set(id, bad).is_err(), "{id} refuses {bad}");
    }
    assert_eq!(env_map(&game_environment(&settings))["GT4_DISPLAY_ASPECT"], "auto");
    settings.set("vsync", "off").unwrap();
    let environment = env_map(&game_environment(&settings));
    assert_eq!(environment["GT4_PRESENT_MODE"], "immediate");
}

#[test]
fn check_every_100th_call_maps_as_documented() {
    let mut settings = Settings::default();
    settings.set("check_vu_code", "100").unwrap();
    let environment = env_map(&game_environment(&settings));
    assert_eq!(environment["GT4_VU1_VERIFY"], "100");
    assert_eq!(environment["GT4_VU0_VERIFY"], "100");
}

#[test]
fn input_resolution_and_widescreen_are_retired_and_fixed() {
    // Owner, 2026-10-21: the game always runs at 480p and 16:9. The rows are gone; old values cannot be set.
    let mut settings = Settings::default();
    for id in ["input_resolution", "widescreen"] {
        assert!(settings::definition(id).is_none(), "{id} has no row");
        assert!(settings::retired(id).is_some(), "{id} is listed as retired");
        assert!(settings.set(id, "480i").is_err());
    }
    for environment in [env_map(&game_environment(&settings))] {
        assert_eq!(environment["GT4_FORCE_480P"], "1");
        assert_eq!(environment["GT4_FORCE_WIDESCREEN"], "1");
        assert!(!environment.contains_key("GT4_RACE_DISPLAY"));
    }
}

#[test]
fn removed_parallel_gs_renderer_and_its_options_are_refused() {
    let mut settings = Settings::default();
    assert!(settings.set("renderer", "parallel").is_err(), "paraLLEl-GS is dropped (decision 0070)");
    assert!(settings::definition("super_sampling").is_none());
    assert!(settings::definition("high_resolution_output").is_none());
    assert!(!env_map(&game_environment(&settings)).keys().any(|name| name.starts_with("GT4_PGS_")));
}

#[test]
fn the_coregs11_rows_are_gone_and_retired_so_their_stored_values_are_ignored() {
    let mut settings = Settings::default();
    for id in ["internal_scale", "present_filter", "fsr_sharpness", "deinterlacing", "anti_aliasing"] {
        assert!(settings::definition(id).is_none(), "{id} is a CoreGS 1.1 row, not a setting");
        assert!(settings::retired(id).is_some(), "{id} is listed as retired");
        assert!(settings.set(id, "1").is_err(), "{id} cannot be set");
    }
    let environment = env_map(&game_environment(&settings));
    assert!(!environment.contains_key("GT4_PRESENT_FILTER"));
    assert!(!environment.contains_key("GT4_FSR_SHARPNESS"));
}

#[test]
fn gsdx_is_the_only_renderer_and_play_needs_it_installed() {
    let mut settings = Settings::default();
    assert_eq!(settings.get("renderer"), "pcsx2", "GSdx is the default");
    assert!(settings.set("renderer", "coregs11").is_err(), "CoreGS 1.1 is not a choice (TASK-096)");
    assert!(settings.set("renderer", "coregs20").is_err(), "coming soon is not selectable");
    settings.set("renderer", "pcsx2").unwrap();

    let installed = installed_fixture();
    assert!(launch::choose_renderer(Some(&installed), Path::new("logs")).is_ok());
    let error = launch::choose_renderer(None, Path::new("logs")).unwrap_err();
    assert_eq!(
        error,
        "GSdx could not run the game: GSdx is not installed yet. Check your internet connection and try again. Details are in the log folder: logs."
    );
    assert!(!error.contains("CoreGS"), "no CoreGS 1.1 fallback");
}

#[test]
fn pcsx2_environment_carries_the_process_pin_config_and_480p() {
    let paths = pcsx2_paths();
    let environment = env_map(&launch::pcsx2_environment(&Settings::default(), &paths));
    assert_eq!(environment["GT4_RENDERER"], "pcsx2");
    assert_eq!(environment["GT4_GS_PROCESS"], paths.executable.display().to_string());
    assert_eq!(environment["GT4_GS_PROCESS_SHA256"], paths.executable_sha256);
    assert_eq!(environment["GT4_GS_CONFIG"], paths.config.display().to_string());
    assert_eq!(environment["GT4_FORCE_480P"], "1", "480p by default");
    assert_eq!(environment["GT4_PRESENT_MODE"], "fifo");
    assert!(!environment.contains_key("GT4_GS_READBACK"), "readback is written in the INI");
    assert!(!environment.contains_key("PS2_SCALE"), "CoreGS 1.1 variables are not sent to PCSX2 GS");
}

#[test]
fn pcsx2_plan_keeps_the_probe_argument_list_and_scrubs_inherited_names() {
    let paths = pcsx2_paths();
    let inherited = vec!["GT4_GS_PROCESS".to_string(), "PATH".to_string(), "PS2_SCALE".to_string()];
    let plan = launch::plan_pcsx2_launch(&inputs(), &Settings::default(), &paths, None, &inherited);
    assert_eq!(plan.arguments.last().unwrap(), "--window");
    assert_eq!(plan.arguments[0], "C:/work/SCUS_973.28");
    assert_eq!(plan.removed_inherited, vec!["GT4_GS_PROCESS".to_string(), "PS2_SCALE".to_string()]);
}

#[test]
fn session_ini_matches_the_python_writer_for_the_default_settings() {
    let paths = pcsx2_paths();
    let text = launch::gs_config_text(&Settings::default(), &paths, None).unwrap();
    let bytes = gt4l_core::gs_table::ini_file_bytes(&text);
    let expected = include_bytes!("fixtures/gs/default-session.ini");
    assert_eq!(bytes.as_slice(), expected.as_slice(), "Rust gt4-gs.ini differs from scripts/gs_settings.py");
    // A 1920x1080 output at 100% is the same 3x: the default INI did not change with the display plan.
    let full_hd = display_target(r"\\.\DISPLAY1", 1920, 1080);
    assert_eq!(launch::gs_config_text(&Settings::default(), &paths, Some(&full_hd)).unwrap(), text);
}

fn display_target(device: &str, width: u32, height: u32) -> DisplayTarget {
    DisplayTarget { device: device.to_string(), output: Size::new(width, height) }
}

fn multiplier_line(text: &str) -> &str {
    text.lines().find(|line| line.starts_with("upscale_multiplier = ")).unwrap()
}

#[test]
fn the_internal_resolution_follows_the_output_size_and_the_render_scale() {
    let paths = pcsx2_paths();
    let mut settings = Settings::default();
    for (width, height, scale, expected) in [
        (1920, 1080, "100", "3"),
        (2560, 1080, "100", "4"),
        (2560, 1440, "50", "2"),
        (2560, 1440, "100", "4"),
        (2560, 1440, "200", "8"),
        (3440, 1440, "100", "6"),
        (3440, 1440, "200", "12"),
        (3088, 1440, "100", "5"),
        (3840, 2160, "75", "5"),
        (1280, 720, "100", "2"),
    ] {
        settings.set("render_scale", scale).unwrap();
        let target = display_target(r"\\.\DISPLAY1", width, height);
        let text = launch::gs_config_text(&settings, &paths, Some(&target)).unwrap();
        assert_eq!(multiplier_line(&text), format!("upscale_multiplier = {expected}"), "{width}x{height} at {scale}%");
    }
    // A stored upscale (an old file edited by hand) never wins: the plan does.
    settings.set("render_scale", "100").unwrap();
    let text = launch::gs_config_text(&settings, &paths, Some(&display_target("x", 2560, 1440))).unwrap();
    assert_eq!(multiplier_line(&text), "upscale_multiplier = 4");
}

#[test]
fn the_display_target_becomes_gt4_monitor_and_gt4_output_size() {
    let target = display_target(r"\\.\DISPLAY2", 3440, 1440);
    let plan = plan_launch(&inputs(), &Settings::default(), Some(&target), &[]);
    let environment = env_map(&plan.environment);
    assert_eq!(environment["GT4_MONITOR"], r"\\.\DISPLAY2");
    assert_eq!(environment["GT4_OUTPUT_SIZE"], "3440x1440");
    assert_eq!(environment["GT4_DISPLAY_ASPECT"], "auto");
    let gs_plan = launch::plan_pcsx2_launch(&inputs(), &Settings::default(), &pcsx2_paths(), Some(&target), &[]);
    assert_eq!(env_map(&gs_plan.environment)["GT4_OUTPUT_SIZE"], "3440x1440");
    let sorted = {
        let mut names: Vec<&String> = gs_plan.environment.iter().map(|(name, _)| name).collect();
        let before = names.clone();
        names.sort();
        names == before
    };
    assert!(sorted, "the environment stays sorted by name");
    // No monitor known: no display variables (the game opens on the primary monitor at its desktop size).
    let unknown = plan_launch(&inputs(), &Settings::default(), None, &[]);
    assert!(!env_map(&unknown.environment).contains_key("GT4_MONITOR"));
    assert!(!env_map(&unknown.environment).contains_key("GT4_OUTPUT_SIZE"));
}

#[test]
fn restart_marker_is_detected_in_the_game_log() {
    assert!(launch::restart_requested(&format!("boot\n{}\nmore\n", launch::GS_RESTART_MARKER)));
    assert!(!launch::restart_requested("[gs-ipc] restart requested: renderer pcsx2\n"));
}

fn installed_fixture() -> gt4l_core::gs_release::Installed {
    gt4l_core::gs_release::Installed {
        version: gt4l_core::gs_release::Version(0, 3, 2),
        executable: PathBuf::from("C:/Users/x/AppData/Local/GT4Recomp/gs/0.3.2/gt4-gs.exe"),
        executable_sha256: "f".repeat(64),
    }
}

fn pcsx2_paths() -> launch::Pcsx2Paths {
    launch::Pcsx2Paths {
        executable: PathBuf::from("C:/gs/0.3.2/gt4-gs.exe"),
        executable_sha256: "a".repeat(64),
        config: PathBuf::from("C:/session/gt4-gs.ini"),
        cache: PathBuf::from("C:/gs/cache"),
        data_root: PathBuf::from("C:/gs/data"),
        textures: PathBuf::from("C:/Users/x/Documents/Gran Turismo 4/Textures"),
    }
}

fn inputs() -> LaunchInputs<'static> {
    LaunchInputs {
        game_executable: Path::new("C:/game/gt4vulkan.exe"),
        elf: Path::new("C:/work/SCUS_973.28"),
        disc: Path::new("C:/disc/game.iso"),
        card: Path::new("C:/cards/card.ps2"),
        session_dir: Path::new("C:/session"),
    }
}

/// Both window modes start the game window (`--window` is required for any window). Only GT4_FULLSCREEN changes,
/// and the native game reads it at start: 1 starts borderless fullscreen, 0 starts windowed.
#[test]
fn both_window_modes_pass_window_and_send_gt4_fullscreen_to_the_game_and_the_gs_child() {
    let mut fullscreen = Settings::default();
    fullscreen.set("fullscreen", "on").unwrap();
    let mut windowed = Settings::default();
    windowed.set("fullscreen", "off").unwrap();

    for (settings, expected) in [(&fullscreen, "1"), (&windowed, "0")] {
        let plan = plan_launch(&inputs(), settings, None, &[]);
        assert_eq!(plan.arguments.last().unwrap(), "--window", "a window is always started");
        assert!(!plan.arguments.iter().any(|argument| argument == "--fullscreen"));
        assert_eq!(env_map(&plan.environment)["GT4_FULLSCREEN"], expected);

        let gs_plan = launch::plan_pcsx2_launch(&inputs(), settings, &pcsx2_paths(), None, &[]);
        assert_eq!(gs_plan.arguments.last().unwrap(), "--window");
        assert_eq!(env_map(&gs_plan.environment)["GT4_FULLSCREEN"], expected, "the PCSX2 GS path gets the same mode");
    }
}
