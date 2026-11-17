#include "bonnet_wind.hpp"

#include "game_target.hpp"

#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <stdexcept>
#include <string>

namespace gt4::bonnet_wind {

namespace {

float g_volume_factor = kDefaultVolume;
AttenuationState g_state;

}  // namespace

float parse_volume(const char *value) {
  if (value == nullptr || *value == '\0') return kDefaultVolume;
  if (std::strcmp(value, "default") == 0 || std::strcmp(value, "100") == 0 ||
      std::strcmp(value, "100%") == 0 || std::strcmp(value, "1.0") == 0 ||
      std::strcmp(value, "1") == 0) {
    return 1.0f;
  }
  if (std::strcmp(value, "0") == 0 || std::strcmp(value, "0%") == 0 ||
      std::strcmp(value, "0.0") == 0 || std::strcmp(value, "off") == 0 ||
      std::strcmp(value, "muted") == 0) {
    return 0.0f;
  }

  char *end = nullptr;
  const double v = std::strtod(value, &end);
  if (end == value) {
    throw std::invalid_argument(std::string("GT4_BONNET_WIND_VOLUME '") + value +
                                "' is not valid (use 0..100, 0.0..1.0, 'default' or 'muted')");
  }
  while (*end == ' ' || *end == '\t') ++end;
  if (*end == '%') ++end;
  while (*end == ' ' || *end == '\t') ++end;
  if (*end != '\0') {
    throw std::invalid_argument(std::string("GT4_BONNET_WIND_VOLUME '") + value +
                                "' contains trailing characters");
  }
  if (v < 0.0 || v > 100.0) {
    throw std::invalid_argument(std::string("GT4_BONNET_WIND_VOLUME '") + value +
                                "' is out of range (use 0..100 or 0.0..1.0)");
  }
  if (v > 1.0) {
    return static_cast<float>(v / 100.0);
  }
  return static_cast<float>(v);
}

Addresses active_addresses() {
  const target::HookAddresses &hooks = target::active().hooks;
  return {hooks.bonnet_wind_visual, hooks.bonnet_wind_visual_volume,
          hooks.bonnet_wind_external_volume, hooks.bonnet_wind_dive};
}

void configure_from_environment() {
  try {
    g_volume_factor = parse_volume(std::getenv(kSwitch));
  } catch (const std::invalid_argument &error) {
    std::fprintf(stderr, "[bonnet-wind] %s, game not started\n", error.what());
    std::exit(1);
  }

  const Addresses addresses = active_addresses();
  if (attenuation_active() && !addresses.complete()) {
    target::note_hook_unavailable("bonnet camera wind volume attenuation (GT4_BONNET_WIND_VOLUME)");
    g_volume_factor = kDefaultVolume;
  }

  if (attenuation_active()) {
    std::fprintf(stderr,
                 "[bonnet-wind] active: volume factor %.2f (%d%%) in bonnet camera (visual 0x%08X, volume 0x%08X)\n",
                 g_volume_factor, static_cast<int>(g_volume_factor * 100.0f + 0.5f),
                 addresses.visual, addresses.visual_volume);
  } else {
    std::fprintf(stderr, "[bonnet-wind] default (100%% volume, stock audio)\n");
  }
}

float configured_volume() { return g_volume_factor; }

bool attenuation_active() { return g_volume_factor < 1.0f; }

void set_volume_for_tests(float volume) { g_volume_factor = volume; }

bool apply_vblank(std::uint8_t *ram, std::uint32_t ram_size, const Addresses &addresses,
                  float volume_factor, AttenuationState &state) {
  if (ram == nullptr || !addresses.complete()) return false;

  if (volume_factor >= 1.0f) {
    if (state.active) {
      if (addresses.visual_volume + sizeof(float) <= ram_size) {
        std::memcpy(ram + addresses.visual_volume, &state.original_volume, sizeof(float));
      }
      state.active = false;
      return true;
    }
    return false;
  }

  if (addresses.visual + sizeof(std::uint32_t) > ram_size ||
      addresses.visual_volume + sizeof(float) > ram_size) {
    return false;
  }

  std::uint32_t current_visual = 0;
  std::memcpy(&current_visual, ram + addresses.visual, sizeof(std::uint32_t));

  float current_volume = 0.0f;
  std::memcpy(&current_volume, ram + addresses.visual_volume, sizeof(float));

  if (is_bonnet_camera(current_visual)) {
    if (!state.active) {
      state.active = true;
      state.original_volume = current_volume;
    } else if (current_volume != state.applied_volume) {
      state.original_volume = current_volume;
    }

    const float target_volume = state.original_volume * volume_factor;
    if (current_volume != target_volume) {
      state.applied_volume = target_volume;
      std::memcpy(ram + addresses.visual_volume, &target_volume, sizeof(float));
      return true;
    }
    return false;
  } else {
    if (state.active) {
      if (current_volume == state.applied_volume) {
        std::memcpy(ram + addresses.visual_volume, &state.original_volume, sizeof(float));
        state.active = false;
        return true;
      }
      state.active = false;
    }
    return false;
  }
}

void at_guest_vblank(std::uint8_t *ram, std::uint32_t ram_size) {
  if (!attenuation_active()) return;
  static const Addresses addresses = active_addresses();
  apply_vblank(ram, ram_size, addresses, g_volume_factor, g_state);
}

}  // namespace gt4::bonnet_wind
