#pragma once
// Pure state machine and safety rules for the TASK-215 H-shifter and clutch gameplay mod.
// Specification: docs/H-SHIFTER-CLUTCH-MOD-STUDY.md and docs/decisions/0073-mod-support-in-bundle.md.
// This module has no OS or hardware dependencies and is unit-testable on any host.

#include <algorithm>
#include <cstdint>
#include <stdexcept>
#include <string_view>

namespace gt4::hshifter {

inline constexpr std::string_view kSwitch = "GT4_MOD_HSHIFTER";
inline constexpr std::string_view kIniKey = "mod_hshifter";

// Parses the mod switch from environment or ini string.
// As mandated by decision 0073: gameplay mods are opt-in and off by default.
// nullptr, empty string, and "0" return false. "1" returns true. Any other value throws std::invalid_argument.
bool parse_switch(const char *value);

enum class Gate : std::uint8_t {
  reverse = 0,
  neutral = 1,
  gear_1  = 2,
  gear_2  = 3,
  gear_3  = 4,
  gear_4  = 5,
  gear_5  = 6,
  gear_6  = 7,
  gear_7  = 8,
  gear_8  = 9,
};

// Clutch state for the game's drivetrain (Automobile + 0x626 / block 0x7F10).
enum class ClutchState : std::uint8_t {
  open     = 0, // disengaged, free-rev, zero clutch torque
  engaged  = 1, // locked to wheels / full torque transfer
  slipping = 2, // transitional slip
  syncing  = 3, // rpm synchronisation during shift
};

// Rejection reason when a shift command cannot be accepted safely.
enum class ShiftRejection : std::uint8_t {
  none = 0,
  speed_too_high_for_reverse,
  gear_exceeds_car_max,
  clutch_not_depressed,
  automatic_transmission_blocked,
};

struct DrivetrainInput {
  Gate gate = Gate::neutral;
  // Clutch pedal travel: 0.0f = completely released by driver (foot off pedal),
  // 1.0f = fully depressed (foot pushing pedal down to floor).
  float pedal_position = 0.0f;
  // Current vehicle speed in meters per second (signed, negative = rolling backward).
  float speed_mps = 0.0f;
  // Current engine RPM.
  float engine_rpm = 0.0f;
  // Vehicle maximum RPM limit (Engine + 0x200).
  std::uint16_t max_rpm = 8000;
  // Number of forward gears available on the car (Engine + 0x236).
  std::uint8_t forward_gears = 6;
  // Whether the current vehicle uses a manual transmission (Automobile + 0x546 == 6).
  bool is_manual_transmission = true;
};

struct ModConfig {
  // Speed threshold below which reverse gear is allowed (~1 km/h = 0.2778 m/s).
  float max_reverse_speed_mps = 0.2778f;
  // Pedal travel fraction above which the clutch is considered disengaged from flywheel.
  float bite_threshold = 0.20f;
  // If true, forward gear changes require pedal_position >= bite_threshold.
  bool require_clutch_to_shift = true;
  // If true, an auto-clutch is simulated when moving between gates if no pedal is pressed.
  bool auto_clutch_assist = false;
  // Gate debounce time in milliseconds.
  std::uint32_t debounce_ms = 50;
};

struct ModOutput {
  // Target gear for Automobile + 0x624 and host block 0x7F14 (0 = R or N, 1..forward_gears).
  std::int32_t target_gear = 0;
  // Clutch state for Automobile + 0x626 and host block 0x7F10.
  ClutchState clutch_state = ClutchState::open;
  // Analog clutch engagement factor (0.0f = open, 1.0f = fully engaged) for +0x618.
  float clutch_engagement = 0.0f;
  // HUD ASCII glyph for gear 0 (block 0x7F16): 'R' (0x52) or 'N' (0x4E). '\0' if forward gear.
  char hud_glyph = 'N';
  // If true, clear guest shift request flags (+0x567, +0x566, +0x62A).
  bool suppress_guest_shifts = true;
  // Whether the requested gate was rejected by safety invariants.
  ShiftRejection rejection = ShiftRejection::none;
};

class HShifterStateMachine {
public:
  explicit HShifterStateMachine(ModConfig config = {});

  // Resets the state machine to neutral at rest.
  void reset();

  // Evaluates one simulation tick (call every VBlank or physics step).
  ModOutput update(const DrivetrainInput &input, std::uint32_t delta_ms);

  [[nodiscard]] const ModConfig &config() const { return config_; }
  void set_config(const ModConfig &config) { config_ = config; }

  [[nodiscard]] Gate active_gate() const { return active_gate_; }
  [[nodiscard]] ClutchState current_clutch_state() const { return current_clutch_state_; }

private:
  ModConfig config_;
  Gate active_gate_ = Gate::neutral;
  Gate pending_gate_ = Gate::neutral;
  std::uint32_t debounce_elapsed_ms_ = 0;
  ClutchState current_clutch_state_ = ClutchState::open;
  std::int32_t current_target_gear_ = 0;
  char current_hud_glyph_ = 'N';
  bool is_syncing_ = false;
  std::uint32_t sync_elapsed_ms_ = 0;
};

} // namespace gt4::hshifter
