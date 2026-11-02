#define _CRT_SECURE_NO_WARNINGS

#include "ingame_settings.hpp"
#include "race_display.hpp"

#include <algorithm>
#include <cctype>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <sstream>

#if defined(_WIN32)
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#endif

namespace gt4::settings {
namespace {

std::string to_lower(std::string_view str) {
  std::string lower(str);
  std::transform(lower.begin(), lower.end(), lower.begin(),
                 [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
  return lower;
}

std::string trim(const std::string &str) {
  const auto start = str.find_first_not_of(" \t\r\n\"'");
  if (start == std::string::npos)
    return "";
  const auto end = str.find_last_not_of(" \t\r\n\"'");
  return str.substr(start, end - start + 1);
}

bool parse_boolean(const std::string &val) {
  const std::string lower = to_lower(trim(val));
  return lower == "1" || lower == "on" || lower == "true" || lower == "yes";
}

void write_le32(std::uint8_t *bytes, std::uint32_t value) {
  for (int i = 0; i < 4; ++i)
    bytes[i] = static_cast<std::uint8_t>(value >> (8 * i));
}

std::uint32_t read_le32(const std::uint8_t *bytes) {
  return static_cast<std::uint32_t>(bytes[0]) | (static_cast<std::uint32_t>(bytes[1]) << 8) |
         (static_cast<std::uint32_t>(bytes[2]) << 16) | (static_cast<std::uint32_t>(bytes[3]) << 24);
}

} // namespace

std::uint32_t resolution_preset_multiplier(ResolutionPreset preset) {
  switch (preset) {
  case ResolutionPreset::p480:
    return 1;
  case ResolutionPreset::p720:
    return 2;
  case ResolutionPreset::p1080:
    return 3;
  case ResolutionPreset::p1440:
    return 4;
  case ResolutionPreset::p4k:
    return 6;
  case ResolutionPreset::p8k:
    return 12;
  default:
    return 1;
  }
}

ResolutionPreset resolution_preset_from_multiplier(std::uint32_t multiplier) {
  switch (multiplier) {
  case 1:
    return ResolutionPreset::p480;
  case 2:
    return ResolutionPreset::p720;
  case 3:
    return ResolutionPreset::p1080;
  case 4:
    return ResolutionPreset::p1440;
  case 6:
    return ResolutionPreset::p4k;
  case 12:
    return ResolutionPreset::p8k;
  default:
    return ResolutionPreset::p480;
  }
}

const char *resolution_preset_name(ResolutionPreset preset) {
  switch (preset) {
  case ResolutionPreset::p480:
    return "480p";
  case ResolutionPreset::p720:
    return "720p";
  case ResolutionPreset::p1080:
    return "1080p";
  case ResolutionPreset::p1440:
    return "1440p";
  case ResolutionPreset::p4k:
    return "4k";
  case ResolutionPreset::p8k:
    return "8k";
  default:
    return "480p";
  }
}

ResolutionPreset parse_resolution_preset(const char *str) {
  if (!str || !*str)
    return ResolutionPreset::p480;

  const std::string lower = to_lower(trim(str));
  if (lower == "480p" || lower == "1x" || lower == "1" || lower == "640x480")
    return ResolutionPreset::p480;
  if (lower == "720p" || lower == "2x" || lower == "2" || lower == "1280x720")
    return ResolutionPreset::p720;
  if (lower == "1080p" || lower == "3x" || lower == "3" || lower == "1920x1080")
    return ResolutionPreset::p1080;
  if (lower == "1440p" || lower == "4x" || lower == "4" || lower == "2560x1440")
    return ResolutionPreset::p1440;
  if (lower == "4k" || lower == "2160p" || lower == "6x" || lower == "6" || lower == "3840x2160")
    return ResolutionPreset::p4k;
  if (lower == "8k" || lower == "4320p" || lower == "12x" || lower == "12" || lower == "7680x4320")
    return ResolutionPreset::p8k;

  return ResolutionPreset::p480;
}

void resolution_preset_dimensions(ResolutionPreset preset, std::uint32_t &width, std::uint32_t &height) {
  switch (preset) {
  case ResolutionPreset::p480:
    width = 640;
    height = 480;
    break;
  case ResolutionPreset::p720:
    width = 1280;
    height = 720;
    break;
  case ResolutionPreset::p1080:
    width = 1920;
    height = 1080;
    break;
  case ResolutionPreset::p1440:
    width = 2560;
    height = 1440;
    break;
  case ResolutionPreset::p4k:
    width = 3840;
    height = 2160;
    break;
  case ResolutionPreset::p8k:
    width = 7680;
    height = 4320;
    break;
  default:
    width = 640;
    height = 480;
    break;
  }
}

InGameSettingsManager::InGameSettingsManager() {
  init_from_env();
}

InGameSettingsManager::InGameSettingsManager(const char *feedback_path) {
  if (feedback_path)
    feedback_path_ = feedback_path;
  init_from_env();
}

InGameSettingsManager &InGameSettingsManager::instance() {
  static InGameSettingsManager manager;
  return manager;
}

void InGameSettingsManager::init_from_env() {
  if (feedback_path_.empty()) {
    if (const char *env_fb = std::getenv("GT4_SETTINGS_FEEDBACK"))
      feedback_path_ = env_fb;
  }

  if (const char *fs = std::getenv("GT4_FULLSCREEN")) {
    current_.fullscreen = (std::strcmp(fs, "1") == 0 || to_lower(fs) == "on" || to_lower(fs) == "true");
  }

  if (const char *out_size = std::getenv("GT4_OUTPUT_SIZE")) {
    current_.resolution = parse_resolution_preset(out_size);
  } else if (const char *scale = std::getenv("GT4_GS_SCALE")) {
    current_.resolution = parse_resolution_preset(scale);
  }

  if (const char *aa = std::getenv("GT4_AA")) {
    current_.anti_aliasing = parse_boolean(aa);
  }
}

void InGameSettingsManager::set_resolution(ResolutionPreset preset) {
  if (current_.resolution != preset) {
    current_.resolution = preset;
    dirty_ = true;
    save_feedback_file();
  }
}

void InGameSettingsManager::set_fullscreen(bool enabled) {
  if (current_.fullscreen != enabled) {
    current_.fullscreen = enabled;
    dirty_ = true;
    save_feedback_file();
  }
}

void InGameSettingsManager::set_anti_aliasing(bool enabled) {
  if (current_.anti_aliasing != enabled) {
    current_.anti_aliasing = enabled;
    dirty_ = true;
    save_feedback_file();
  }
}

std::string InGameSettingsManager::serialize_toml() const {
  std::uint32_t w = 0, h = 0;
  resolution_preset_dimensions(current_.resolution, w, h);
  const char *res_name = resolution_preset_name(current_.resolution);

  std::ostringstream ss;
  ss << "# In-game settings changes written by GT4Recomp (TASK-078)\n";
  ss << "[settings]\n";
  ss << "fullscreen = \"" << (current_.fullscreen ? "on" : "off") << "\"\n";
  ss << "resolution = \"" << w << "x" << h << "\"\n";
  ss << "\n";
  ss << "[gs]\n";
  ss << "upscale = \"" << res_name << "\"\n";
  ss << "fxaa = \"" << (current_.anti_aliasing ? "true" : "false") << "\"\n";
  return ss.str();
}

bool InGameSettingsManager::save_feedback_file() const {
  if (feedback_path_.empty())
    return false;

  const std::string tmp_path = feedback_path_ + ".tmp";
  {
    std::ofstream out(tmp_path, std::ios::binary | std::ios::trunc);
    if (!out.is_open())
      return false;
    const std::string toml_data = serialize_toml();
    out.write(toml_data.data(), static_cast<std::streamsize>(toml_data.size()));
    if (!out.good())
      return false;
  }

#if defined(_WIN32)
  if (!MoveFileExA(tmp_path.c_str(), feedback_path_.c_str(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)) {
    // Fallback if MoveFileExA fails
    std::remove(feedback_path_.c_str());
    return std::rename(tmp_path.c_str(), feedback_path_.c_str()) == 0;
  }
  return true;
#else
  return std::rename(tmp_path.c_str(), feedback_path_.c_str()) == 0;
#endif
}

bool InGameSettingsManager::load_feedback_string(const std::string &toml_text) {
  std::istringstream stream(toml_text);
  std::string line;
  std::string section;

  while (std::getline(stream, line)) {
    line = trim(line);
    if (line.empty() || line[0] == '#')
      continue;

    if (line.front() == '[' && line.back() == ']') {
      section = to_lower(trim(line.substr(1, line.size() - 2)));
      continue;
    }

    const auto eq = line.find('=');
    if (eq == std::string::npos)
      continue;

    const std::string key = to_lower(trim(line.substr(0, eq)));
    const std::string val = trim(line.substr(eq + 1));

    if (section == "settings") {
      if (key == "fullscreen") {
        current_.fullscreen = parse_boolean(val);
      } else if (key == "resolution") {
        current_.resolution = parse_resolution_preset(val.c_str());
      }
    } else if (section == "gs") {
      if (key == "upscale") {
        current_.resolution = parse_resolution_preset(val.c_str());
      } else if (key == "fxaa" || key == "anti_aliasing") {
        current_.anti_aliasing = parse_boolean(val);
      }
    }
  }

  return true;
}

bool InGameSettingsManager::load_feedback_file(const char *path) {
  if (!path)
    return false;
  std::ifstream in(path, std::ios::binary);
  if (!in.is_open())
    return false;
  std::ostringstream ss;
  ss << in.rdbuf();
  return load_feedback_string(ss.str());
}

bool InGameSettingsManager::handle_guest_option_write(std::uint8_t *ram, std::uint32_t ram_size,
                                                      std::uint32_t guest_choice_index) {
  using namespace gt4::display;

  if (!ram || ram_size < race_owner_pointer_address + 4)
    return false;

  const std::uint32_t owner_pointer = read_le32(ram + race_owner_pointer_address);
  const std::uint32_t index_address = race_display_index_address(owner_pointer, ram_size);
  if (index_address == 0)
    return false;

  // Map choice to ResolutionPreset
  const auto preset = static_cast<ResolutionPreset>(guest_choice_index % kNumResolutionPresets);
  set_resolution(preset);

  // Decouple: ensure guest RAM option index stays at neutral 480p (1)
  write_le32(ram + index_address, race_display_index_480p);
  return true;
}

} // namespace gt4::settings
