#pragma once
// TASK-163: Bonnet camera wind volume attenuation (issue #135, study #171).
//
// In Gran Turismo 4, driving at high speeds in bonnet/roof view (camera view index 1)
// produces a continuous high-amplitude aerodynamic wind noise synthesis loop.
// This module intercepts the camera visual perspective and attenuates or mutes
// the wind voice volume while preserving mechanical engine, transmission,
// curb strikes, and tire friction audio at 100% fidelity.
//
// GT4_BONNET_WIND_VOLUME: volume factor or percentage (0..100, or 0.0..1.0).
//   Unset, empty, 100, 1.0, 1, "default": original game sound (default, 100% volume);
//   0, 0.0, "muted", "off": muted (0% wind noise in bonnet cam);
//   50, 0.5: 50% wind volume attenuation.
//   Invalid values throw std::invalid_argument and refuse the start.
//
// game.ini key: bonnet_wind_volume

#include <cstdint>

namespace gt4::bonnet_wind {

inline constexpr const char *kSwitch = "GT4_BONNET_WIND_VOLUME";
inline constexpr const char *kIniKey = "bonnet_wind_volume";
inline constexpr float kDefaultVolume = 1.0f;

// Camera perspective index for bonnet / hood / roof view in GT4.
inline constexpr std::uint32_t kBonnetCameraIndex = 1;

struct Addresses {
  std::uint32_t visual = 0;          // RaceCarSound::visual_ (0x006213E8 on v2.00)
  std::uint32_t visual_volume = 0;   // RaceCarSound::visual_volume_ (0x006213DC on v2.00)
  std::uint32_t external_volume = 0; // RaceCarSound::external_volume_ (0x006213E0 on v2.00)
  std::uint32_t dive = 0;            // RaceCarSound::dive_ (0x006213EC on v2.00)

  constexpr bool complete() const { return visual != 0 && visual_volume != 0; }
};

struct AttenuationState {
  bool active = false;
  float original_volume = 1.0f;
  float applied_volume = 1.0f;
};

// Parses GT4_BONNET_WIND_VOLUME into a factor in [0.0, 1.0].
// Unset / empty / "100" / "1" / "1.0" / "default" = 1.0f.
// "0" / "0.0" / "off" / "muted" = 0.0f.
// Percentages 0..100 or fractions 0.0..1.0 are scaled to [0.0, 1.0].
// Invalid strings throw std::invalid_argument. Pure.
float parse_volume(const char *value);

// Returns true if the camera view index corresponds to the bonnet/roof camera.
constexpr bool is_bonnet_camera(std::uint32_t visual_index) {
  return visual_index == kBonnetCameraIndex;
}

// Configures the bonnet wind volume factor from environment variable GT4_BONNET_WIND_VOLUME.
// Refuses invalid values on start.
void configure_from_environment();

// Active volume factor [0.0, 1.0]. 1.0f = default stock behavior.
float configured_volume();

// Returns true if wind attenuation is active (factor < 1.0f).
bool attenuation_active();

// Target hook addresses.
Addresses active_addresses();

// For unit testing and fixtures.
void set_volume_for_tests(float volume);

// Pure VBlank evaluation logic operating on memory buffer.
// Reads visual perspective; if in bonnet view, scales visual_volume by volume factor.
// Restores original volume when exiting bonnet view.
// Returns true if a write to ram occurred.
bool apply_vblank(std::uint8_t *ram, std::uint32_t ram_size, const Addresses &addresses,
                  float volume_factor, AttenuationState &state);

// Called at guest VBlank from mtvu_runtime.cpp / window.cpp.
void at_guest_vblank(std::uint8_t *ram, std::uint32_t ram_size);

}  // namespace gt4::bonnet_wind
