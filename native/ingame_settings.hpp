#pragma once

// TASK-078: In-game graphics settings manager and decoupling from PS2 memory card.
//
// Allows GT4's Options menu to drive host graphics configuration:
// - Output resolution presets: 480p (1x), 720p (2x), 1080p (3x), 1440p (4x), 4K (6x), 8K (12x).
// - Fullscreen toggle (synchronized with host Alt+Enter / F11 and launcher).
// - Anti-aliasing toggle (FXAA).
//
// Decoupling from memory card:
// Guest saves always retain the neutral 480p progressive index 1.
// Host settings persist via an atomic feedback file `game-changes.toml` (GT4_SETTINGS_FEEDBACK),
// which the launcher merges into `settings.toml` on game exit or next startup.

#include <cstdint>
#include <string>

namespace gt4::settings {

enum class ResolutionPreset : std::uint32_t {
  p480 = 0,   // 1x (640x480)
  p720 = 1,   // 2x (1280x720)
  p1080 = 2,  // 3x (1920x1080)
  p1440 = 3,  // 4x (2560x1440)
  p4k = 4,    // 6x (3840x2160)
  p8k = 5,    // 12x (7680x4320)
};

inline constexpr std::uint32_t kNumResolutionPresets = 6;

// Convert preset enum to upscale multiplier (1, 2, 3, 4, 6, 12).
std::uint32_t resolution_preset_multiplier(ResolutionPreset preset);

// Convert upscale multiplier to preset enum (clamps unknown/other values to p480).
ResolutionPreset resolution_preset_from_multiplier(std::uint32_t multiplier);

// Convert preset enum to canonical name string ("480p", "720p", "1080p", "1440p", "4k", "8k").
const char *resolution_preset_name(ResolutionPreset preset);

// Convert name string to preset enum ("480p", "1x", "1", "720p", "2x", "2", etc.).
ResolutionPreset parse_resolution_preset(const char *str);

// Canonical output pixel dimensions for a resolution preset.
void resolution_preset_dimensions(ResolutionPreset preset, std::uint32_t &width, std::uint32_t &height);

struct InGameSettings {
  ResolutionPreset resolution = ResolutionPreset::p480;
  bool fullscreen = true;
  bool anti_aliasing = false; // FXAA
};

class InGameSettingsManager {
public:
  InGameSettingsManager();
  explicit InGameSettingsManager(const char *feedback_path);

  static InGameSettingsManager &instance();

  const InGameSettings &current() const { return current_; }
  bool is_dirty() const { return dirty_; }
  const std::string &feedback_path() const { return feedback_path_; }

  void set_feedback_path(const std::string &path) { feedback_path_ = path; }
  void set_resolution(ResolutionPreset preset);
  void set_fullscreen(bool enabled);
  void set_anti_aliasing(bool enabled);

  // Initialize current settings from environment variables (GT4_FULLSCREEN, GT4_SETTINGS_FEEDBACK, etc.).
  void init_from_env();

  // Atomically writes settings changes to `game-changes.toml` (<path>.tmp then rename).
  bool save_feedback_file() const;

  // Serializes settings changes into a TOML document.
  std::string serialize_toml() const;

  // Loads settings from TOML text or a file.
  bool load_feedback_string(const std::string &toml_text);
  bool load_feedback_file(const char *path);

  // Intercepts guest option write to guest RAM (option + 0x16B0).
  // Maps `guest_choice_index` (0..5) to host ResolutionPreset, updates host state, writes feedback file,
  // and enforces `race_display_index_480p` (1) in guest RAM so saves on memory card stay standard 480p.
  bool handle_guest_option_write(std::uint8_t *ram, std::uint32_t ram_size, std::uint32_t guest_choice_index);

private:
  InGameSettings current_{};
  bool dirty_ = false;
  std::string feedback_path_;
};

} // namespace gt4::settings
