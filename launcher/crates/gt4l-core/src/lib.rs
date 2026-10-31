//! Core of the GT4Recomp launcher: settings, config file, paths, launch plan, exit classification, release
//! lists, updates, `version.json`, schema migrations and the data-path guard. No UI here. The one
//! Windows-specific piece is the process-lock file, which uses std APIs only.

pub mod config;
pub mod display;
pub mod exit;
pub mod guard;
pub mod gs_release;
pub mod gs_table;
pub mod launch;
pub mod lock;
pub mod migrate;
pub mod paths;
pub mod releases;
pub mod relocate;
pub mod retroachievements;
pub mod settings;
pub mod system_check;
pub mod textures;
pub mod update;
pub mod version;
pub mod versions;

pub use config::ConfigFile;
pub use settings::{SettingDef, Settings, SETTINGS};
pub use version::Version;

/// Numeric version used by the legacy updater and install receipts. The Cargo package is authoritative.
pub const LAUNCHER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Human-readable build version; `GT4_RELEASE_LABEL=rc1` adds `-rc1` without changing install/update semantics.
pub const LAUNCHER_DISPLAY_VERSION: &str = env!("GT4_LAUNCHER_DISPLAY_VERSION");
pub const RELEASE_LABEL: &str = env!("GT4_RELEASE_LABEL");
