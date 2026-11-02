//! Settings table and config file: validation, defaults and round trips.

use gt4l_core::config::{self, ConfigFile};
use gt4l_core::settings::{self, Settings, SettingError, SETTINGS};

#[test]
fn every_definition_id_is_unique_and_has_a_valid_default() {
    let mut seen = std::collections::BTreeSet::new();
    for definition in SETTINGS {
        assert!(seen.insert(definition.id), "duplicate id {}", definition.id);
        assert!(
            settings::validate(definition, definition.default).is_ok(),
            "default of {} is not allowed",
            definition.id
        );
    }
}

#[test]
fn renderer_is_gsdx_and_neither_coregs_build_can_be_selected() {
    let mut settings = Settings::default();
    assert_eq!(settings.get("renderer"), "pcsx2");
    assert_eq!(
        settings.set("renderer", "coregs20"),
        Err(SettingError::InvalidValue { id: "renderer".into(), value: "coregs20".into() })
    );
    assert_eq!(
        settings.set("renderer", "coregs11"),
        Err(SettingError::InvalidValue { id: "renderer".into(), value: "coregs11".into() }),
        "CoreGS 1.1 is no longer a choice (TASK-096)"
    );
    assert_eq!(settings.get("renderer"), "pcsx2", "a refused value leaves the state unchanged");
}

#[test]
fn a_stored_coregs11_renderer_loads_as_gsdx() {
    let text = "schema_version = 6

[settings]
renderer = \"coregs11\"
vsync = \"fast\"
";
    let (config, _warnings) = config::parse(text).expect("parse");
    assert_eq!(config.settings.get("renderer"), "pcsx2", "schema 7 maps CoreGS 1.1 to GSdx");
    assert_eq!(config.settings.get("vsync"), "fast");
    let written = config::render(&config);
    assert!(written.contains("renderer = \"pcsx2\""), "{written}");
}

#[test]
fn unknown_ids_and_bad_toggle_values_are_refused() {
    let mut settings = Settings::default();
    assert!(matches!(settings.set("gamma", "2"), Err(SettingError::UnknownSetting(_))));
    assert!(settings.set("fullscreen", "yes").is_err());
    assert!(settings.set("super_sampling", "3").is_err());
    assert!(settings.set("high_resolution_output", "on").is_err());
}

#[test]
fn retired_and_forced_values_of_an_old_file_are_ignored_and_dropped_at_the_next_write() {
    // Owner, 2026-10-21: 480p and 16:9 are fixed, and the PCSX2 OSD rows are forced off. A file written by an earlier
    // 1.0.0 launcher may still hold the old values; they are ignored on load (no schema step) and gone after a save.
    let text = concat!(
        "schema_version = 3\n\n",
        "[settings]\ninput_resolution = \"1080i\"\nwidescreen = \"off\"\nvsync = \"fast\"\n\n",
        "[choices]\ninput_resolution = true\nwidescreen = true\n\n",
        "[gs]\naspect = \"4:3\"\nosd_fps = \"true\"\nosd_gpu = \"true\"\nosd_resolution = \"true\"\nfxaa = \"true\"\n",
    );
    let (config, warnings) = config::parse(text).expect("parse");
    assert_eq!(config.settings.get("vsync"), "fast", "other values are kept");
    assert_eq!(config.settings.gs_value("fxaa"), "true", "other GS values are kept");
    assert!(warnings.iter().any(|w| w.contains("'input_resolution' is no longer used")), "{warnings:?}");
    assert!(warnings.iter().any(|w| w.contains("'widescreen' is no longer used")), "{warnings:?}");
    for id in ["aspect", "osd_fps", "osd_gpu", "osd_resolution"] {
        assert!(!config.settings.gs_overrides().contains_key(id), "{id} is not taken from the file");
    }
    assert_eq!(config.settings.gs_value("aspect"), "16:9");
    assert_eq!(config.settings.gs_value("osd_fps"), "false");
    assert!(config.settings.chosen_ids().is_empty(), "retired ids are not kept as choices");
    let written = config::render(&config);
    assert!(written.contains("resolution = \"desktop\""), "the TASK-080 display rows replace them: {written}");
    for retired in ["input_resolution", "widescreen", "aspect =", "osd_"] {
        assert!(!written.contains(retired), "{retired} is dropped at the next write:\n{written}");
    }
}

#[test]
fn config_round_trip_keeps_values_and_paths() {
    let mut config = ConfigFile::default();
    config.settings.set("vsync", "fast").unwrap();
    config.settings.set_gs("fxaa", "true").unwrap();
    // TASK-080 addendum: the monitor identifier holds backslashes; it must survive the TOML round trip.
    config.settings.set("monitor", r"\\.\DISPLAY2@3440x1440").unwrap();
    config.settings.set("resolution", "2560x1080").unwrap();
    config.settings.set("render_scale", "175").unwrap();
    config.settings.set_gs("performance_mode", "no-readbacks").unwrap();
    config.disc_path = Some(r#"D:\Games\Gran Turismo 4 "quoted" (USA).iso"#.into());
    config.dev_game_dir = Some(r"C:\Antigravity\build\game".into());
    let text = config::render(&config);
    let (parsed, warnings) = config::parse(&text).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(parsed, config);
}

#[test]
fn invalid_values_and_unknown_keys_fall_back_with_warnings() {
    let text = "schema_version = 1\n[settings]\nvsync = \"maybe\"\nflux = \"on\"\nfullscreen = \"off\"\n";
    let (config, warnings) = config::parse(text).unwrap();
    assert_eq!(config.settings.get("vsync"), "on");
    assert_eq!(config.settings.get("fullscreen"), "off");
    assert_eq!(warnings.len(), 2, "{warnings:?}");
}

#[test]
fn newer_schema_is_refused_and_syntax_errors_are_reported() {
    assert!(matches!(config::parse("schema_version = 9\n"), Err(config::ConfigError::NewerSchema(9))));
    assert!(matches!(config::parse("[settings\n"), Err(config::ConfigError::Syntax(_))));
}

#[test]
fn load_of_a_damaged_file_moves_it_aside() {
    let directory = std::env::temp_dir().join(format!("gt4l-cfg-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("settings.toml");
    std::fs::write(&path, "[settings\n").unwrap();
    let (config, warnings) = config::load(&path);
    assert_eq!(config, ConfigFile::default());
    assert_eq!(warnings.len(), 1);
    assert!(directory.join("settings.toml.bad").exists());
    assert!(!path.exists());
    std::fs::remove_dir_all(&directory).unwrap();
}


#[test]
fn graphics_api_accepts_vulkan_d3d12_and_d3d11_and_refuses_opengl_and_software() {
    let mut settings = Settings::default();
    assert_eq!(settings.gs_value("graphics_api"), "vulkan");
    assert_eq!(settings.gs_value("renderer"), "vulkan");

    // Setting graphics_api to d3d12
    assert!(settings.set_gs("graphics_api", "d3d12").is_ok());
    assert_eq!(settings.gs_value("graphics_api"), "d3d12");
    assert_eq!(settings.gs_value("renderer"), "d3d12");
    assert_eq!(settings.gs_overrides().get("graphics_api").map(|s| s.as_str()), Some("d3d12"));

    // Setting graphics_api to d3d11
    assert!(settings.set_gs("graphics_api", "d3d11").is_ok());
    assert_eq!(settings.gs_value("graphics_api"), "d3d11");

    // Setting graphics_api to vulkan
    assert!(settings.set_gs("graphics_api", "vulkan").is_ok());
    assert_eq!(settings.gs_value("graphics_api"), "vulkan");

    // Legacy key "renderer" normalizes to graphics_api
    assert!(settings.set_gs("renderer", "d3d12").is_ok());
    assert_eq!(settings.gs_value("graphics_api"), "d3d12");
    assert_eq!(settings.gs_overrides().get("graphics_api").map(|s| s.as_str()), Some("d3d12"));
    assert!(!settings.gs_overrides().contains_key("renderer"));

    // Software and OpenGL are refused
    assert!(settings.set_gs("graphics_api", "opengl").is_err());
    assert!(settings.set_gs("graphics_api", "software").is_err());
    assert!(settings.set_gs("graphics_api", "13").is_err());
}

#[test]
fn graphics_api_in_config_round_trips_and_legacy_renderer_migrates() {
    let text = "schema_version = 7\n\n[settings]\nrenderer = \"pcsx2\"\nvsync = \"fast\"\n\n[gs]\ngraphics_api = \"d3d12\"\n";
    let (config, _warnings) = config::parse(text).expect("parse");
    assert_eq!(config.settings.gs_value("graphics_api"), "d3d12");
    let written = config::render(&config);
    assert!(written.contains("graphics_api = \"d3d12\""), "{written}");

    // Legacy renderer under [gs]
    let legacy = "schema_version = 7\n\n[settings]\nrenderer = \"pcsx2\"\nvsync = \"fast\"\n\n[gs]\nrenderer = \"d3d11\"\n";
    let (legacy_config, _warnings) = config::parse(legacy).expect("parse legacy");
    assert_eq!(legacy_config.settings.gs_value("graphics_api"), "d3d11");
    let legacy_written = config::render(&legacy_config);
    assert!(legacy_written.contains("graphics_api = \"d3d11\""), "{legacy_written}");
    assert!(!legacy_written.contains("renderer = \"d3d11\""), "legacy renderer under [gs] is migrated to graphics_api: {legacy_written}");
}

#[test]
fn graphics_api_renders_correct_ini_backend() {
    use std::collections::BTreeMap;
    use gt4l_core::gs_table;

    // Default renders Vulkan (14)
    let default_ini = gs_table::render_ini(&BTreeMap::new(), &BTreeMap::new()).unwrap();
    assert!(default_ini.contains("Renderer = 14"), "{default_ini}");

    // graphics_api = d3d12 renders DX12 (15)
    let d3d12_map = BTreeMap::from([("graphics_api".to_string(), "d3d12".to_string())]);
    let d3d12_ini = gs_table::render_ini(&d3d12_map, &BTreeMap::new()).unwrap();
    assert!(d3d12_ini.contains("Renderer = 15"), "{d3d12_ini}");

    // graphics_api = d3d11 renders DX11 (3)
    let d3d11_map = BTreeMap::from([("graphics_api".to_string(), "d3d11".to_string())]);
    let d3d11_ini = gs_table::render_ini(&d3d11_map, &BTreeMap::new()).unwrap();
    assert!(d3d11_ini.contains("Renderer = 3"), "{d3d11_ini}");

    // Legacy key renderer = d3d12 also renders DX12 (15)
    let legacy_map = BTreeMap::from([("renderer".to_string(), "d3d12".to_string())]);
    let legacy_ini = gs_table::render_ini(&legacy_map, &BTreeMap::new()).unwrap();
    assert!(legacy_ini.contains("Renderer = 15"), "{legacy_ini}");
}
