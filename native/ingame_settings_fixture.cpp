#define _CRT_SECURE_NO_WARNINGS

#include "ingame_settings.hpp"
#include "race_display.hpp"

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <vector>

namespace {

using namespace gt4::settings;
using namespace gt4::display;

constexpr std::uint32_t ram_size = 32u << 20;
constexpr std::uint32_t owner = 0x00300000u;
constexpr std::uint32_t index_address = owner + 0x38CB0u + 0x16B0u;

void require(bool condition, const char *message) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", message);
    std::exit(1);
  }
}

void store(std::vector<std::uint8_t> &ram, std::uint32_t address, std::uint32_t value) {
  for (int i = 0; i < 4; ++i)
    ram[address + i] = static_cast<std::uint8_t>(value >> (8 * i));
}

std::uint32_t load(const std::vector<std::uint8_t> &ram, std::uint32_t address) {
  return static_cast<std::uint32_t>(ram[address]) | (static_cast<std::uint32_t>(ram[address + 1]) << 8) |
         (static_cast<std::uint32_t>(ram[address + 2]) << 16) |
         (static_cast<std::uint32_t>(ram[address + 3]) << 24);
}

std::vector<std::uint8_t> make_ram(std::uint32_t owner_pointer, std::uint32_t index_value) {
  std::vector<std::uint8_t> ram(ram_size, 0);
  store(ram, race_owner_pointer_address, owner_pointer);
  if (owner_pointer != 0 && index_address + 4 <= ram_size)
    store(ram, index_address, index_value);
  return ram;
}

void test_presets_and_multipliers() {
  require(resolution_preset_multiplier(ResolutionPreset::p480) == 1, "480p is 1x");
  require(resolution_preset_multiplier(ResolutionPreset::p720) == 2, "720p is 2x");
  require(resolution_preset_multiplier(ResolutionPreset::p1080) == 3, "1080p is 3x");
  require(resolution_preset_multiplier(ResolutionPreset::p1440) == 4, "1440p is 4x");
  require(resolution_preset_multiplier(ResolutionPreset::p4k) == 6, "4k is 6x");
  require(resolution_preset_multiplier(ResolutionPreset::p8k) == 12, "8k is 12x");

  require(resolution_preset_from_multiplier(1) == ResolutionPreset::p480, "1x -> 480p");
  require(resolution_preset_from_multiplier(2) == ResolutionPreset::p720, "2x -> 720p");
  require(resolution_preset_from_multiplier(3) == ResolutionPreset::p1080, "3x -> 1080p");
  require(resolution_preset_from_multiplier(4) == ResolutionPreset::p1440, "4x -> 1440p");
  require(resolution_preset_from_multiplier(6) == ResolutionPreset::p4k, "6x -> 4k");
  require(resolution_preset_from_multiplier(12) == ResolutionPreset::p8k, "12x -> 8k");
  require(resolution_preset_from_multiplier(99) == ResolutionPreset::p480, "unknown -> 480p");
}

void test_preset_names_and_parsing() {
  require(std::strcmp(resolution_preset_name(ResolutionPreset::p480), "480p") == 0, "name 480p");
  require(std::strcmp(resolution_preset_name(ResolutionPreset::p720), "720p") == 0, "name 720p");
  require(std::strcmp(resolution_preset_name(ResolutionPreset::p1080), "1080p") == 0, "name 1080p");
  require(std::strcmp(resolution_preset_name(ResolutionPreset::p1440), "1440p") == 0, "name 1440p");
  require(std::strcmp(resolution_preset_name(ResolutionPreset::p4k), "4k") == 0, "name 4k");
  require(std::strcmp(resolution_preset_name(ResolutionPreset::p8k), "8k") == 0, "name 8k");

  require(parse_resolution_preset("480p") == ResolutionPreset::p480, "parse 480p");
  require(parse_resolution_preset("1x") == ResolutionPreset::p480, "parse 1x");
  require(parse_resolution_preset("720p") == ResolutionPreset::p720, "parse 720p");
  require(parse_resolution_preset("2x") == ResolutionPreset::p720, "parse 2x");
  require(parse_resolution_preset("1080p") == ResolutionPreset::p1080, "parse 1080p");
  require(parse_resolution_preset("3x") == ResolutionPreset::p1080, "parse 3x");
  require(parse_resolution_preset("1920x1080") == ResolutionPreset::p1080, "parse 1920x1080");
  require(parse_resolution_preset("1440p") == ResolutionPreset::p1440, "parse 1440p");
  require(parse_resolution_preset("4x") == ResolutionPreset::p1440, "parse 4x");
  require(parse_resolution_preset("4k") == ResolutionPreset::p4k, "parse 4k");
  require(parse_resolution_preset("6x") == ResolutionPreset::p4k, "parse 6x");
  require(parse_resolution_preset("3840x2160") == ResolutionPreset::p4k, "parse 3840x2160");
  require(parse_resolution_preset("8k") == ResolutionPreset::p8k, "parse 8k");
  require(parse_resolution_preset("12x") == ResolutionPreset::p8k, "parse 12x");
  require(parse_resolution_preset("invalid") == ResolutionPreset::p480, "parse invalid");
}

void test_dimensions() {
  std::uint32_t w = 0, h = 0;
  resolution_preset_dimensions(ResolutionPreset::p1080, w, h);
  require(w == 1920 && h == 1080, "1080p dimensions");
  resolution_preset_dimensions(ResolutionPreset::p4k, w, h);
  require(w == 3840 && h == 2160, "4K dimensions");
  resolution_preset_dimensions(ResolutionPreset::p8k, w, h);
  require(w == 7680 && h == 4320, "8K dimensions");
}

void test_toml_serialization_and_parsing() {
  InGameSettingsManager manager;
  manager.set_fullscreen(false);
  manager.set_resolution(ResolutionPreset::p1440);
  manager.set_anti_aliasing(true);

  const std::string toml = manager.serialize_toml();
  require(toml.find("fullscreen = \"off\"") != std::string::npos, "serialized fullscreen off");
  require(toml.find("resolution = \"2560x1440\"") != std::string::npos, "serialized 1440p size");
  require(toml.find("upscale = \"1440p\"") != std::string::npos, "serialized upscale 1440p");
  require(toml.find("fxaa = \"true\"") != std::string::npos, "serialized fxaa true");

  InGameSettingsManager receiver;
  require(receiver.load_feedback_string(toml), "parsed toml feedback");
  require(!receiver.current().fullscreen, "received fullscreen false");
  require(receiver.current().resolution == ResolutionPreset::p1440, "received 1440p");
  require(receiver.current().anti_aliasing, "received fxaa true");
}

void test_feedback_file_roundtrip() {
  const std::filesystem::path test_dir = std::filesystem::temp_directory_path() / "gt4_test_settings";
  std::filesystem::create_directories(test_dir);
  const std::filesystem::path feedback_file = test_dir / "game-changes.toml";

  InGameSettingsManager writer(feedback_file.string().c_str());
  writer.set_resolution(ResolutionPreset::p4k);
  writer.set_fullscreen(true);
  writer.set_anti_aliasing(false);
  require(writer.save_feedback_file(), "saved feedback file atomically");
  require(std::filesystem::exists(feedback_file), "feedback file exists on disk");

  InGameSettingsManager reader;
  require(reader.load_feedback_file(feedback_file.string().c_str()), "loaded feedback file");
  require(reader.current().resolution == ResolutionPreset::p4k, "reloaded 4K");
  require(reader.current().fullscreen, "reloaded fullscreen true");
  require(!reader.current().anti_aliasing, "reloaded fxaa false");

  std::filesystem::remove_all(test_dir);
}

void test_guest_decoupling_and_480p_hold() {
  const std::filesystem::path test_dir = std::filesystem::temp_directory_path() / "gt4_decouple_test";
  std::filesystem::create_directories(test_dir);
  const std::filesystem::path feedback_file = test_dir / "game-changes.toml";

  InGameSettingsManager manager(feedback_file.string().c_str());

  // RAM initialized with normal index 0
  auto ram = make_ram(owner, race_display_index_normal);

  // Guest option write simulates user selecting preset index 2 (1080p) in GT4 options
  require(manager.handle_guest_option_write(ram.data(), ram_size, 2u), "handled option write");

  // Host manager updated resolution to 1080p
  require(manager.current().resolution == ResolutionPreset::p1080, "host resolution is 1080p");

  // Guest RAM index was decoupled: held at 1 (race_display_index_480p)
  require(load(ram, index_address) == race_display_index_480p, "guest RAM index is decoupled to 480p");

  // Feedback file was saved with the host choice
  require(std::filesystem::exists(feedback_file), "feedback file saved after option write");

  // Simulate another write: preset index 4 (4K)
  require(manager.handle_guest_option_write(ram.data(), ram_size, 4u), "handled 4K option write");
  require(manager.current().resolution == ResolutionPreset::p4k, "host resolution is 4K");
  require(load(ram, index_address) == race_display_index_480p, "guest RAM index stays 480p");

  std::filesystem::remove_all(test_dir);
}

} // namespace

int main() {
  test_presets_and_multipliers();
  test_preset_names_and_parsing();
  test_dimensions();
  test_toml_serialization_and_parsing();
  test_feedback_file_roundtrip();
  test_guest_decoupling_and_480p_hold();
  std::fprintf(stderr, "ingame_settings fixture OK\n");
  return 0;
}
