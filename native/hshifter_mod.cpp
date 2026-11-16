#include "hshifter_mod.hpp"

#include <cmath>
#include <cstring>

namespace gt4::hshifter {

bool parse_switch(const char *value) {
  if (value == nullptr || *value == '\0' || std::strcmp(value, "0") == 0) {
    return false;
  }
  if (std::strcmp(value, "1") == 0) {
    return true;
  }
  throw std::invalid_argument("invalid switch value for GT4_MOD_HSHIFTER");
}

HShifterStateMachine::HShifterStateMachine(ModConfig config) : config_(config) {
  reset();
}

void HShifterStateMachine::reset() {
  active_gate_ = Gate::neutral;
  pending_gate_ = Gate::neutral;
  debounce_elapsed_ms_ = 0;
  current_clutch_state_ = ClutchState::open;
  current_target_gear_ = 0;
  current_hud_glyph_ = 'N';
  is_syncing_ = false;
  sync_elapsed_ms_ = 0;
}

ModOutput HShifterStateMachine::update(const DrivetrainInput &input, std::uint32_t delta_ms) {
  ModOutput output;

  // Mod does not interfere with automatic transmission vehicles.
  if (!input.is_manual_transmission) {
    output.rejection = ShiftRejection::automatic_transmission_blocked;
    output.suppress_guest_shifts = false;
    output.target_gear = current_target_gear_;
    output.clutch_state = current_clutch_state_;
    output.clutch_engagement = 1.0f;
    output.hud_glyph = current_hud_glyph_;
    return output;
  }

  // Handle gate input debounce.
  if (input.gate != pending_gate_) {
    pending_gate_ = input.gate;
    debounce_elapsed_ms_ = delta_ms;
  } else {
    debounce_elapsed_ms_ += delta_ms;
  }

  // Process gate transition once debounce window has elapsed.
  if (debounce_elapsed_ms_ >= config_.debounce_ms && pending_gate_ != active_gate_) {
    std::int32_t desired_gear = 0;
    bool valid = true;

    if (pending_gate_ == Gate::reverse) {
      desired_gear = 0;
      // Invariant: reverse gear is only permitted when vehicle is near standstill.
      if (std::abs(input.speed_mps) >= config_.max_reverse_speed_mps) {
        output.rejection = ShiftRejection::speed_too_high_for_reverse;
        valid = false;
      }
    } else if (pending_gate_ == Gate::neutral) {
      desired_gear = 0;
    } else {
      const auto forward_idx = static_cast<std::int32_t>(pending_gate_) - static_cast<std::int32_t>(Gate::gear_1) + 1;
      // Invariant: gear cannot exceed vehicle's transmission gear count.
      if (forward_idx > static_cast<std::int32_t>(input.forward_gears)) {
        output.rejection = ShiftRejection::gear_exceeds_car_max;
        valid = false;
      } else {
        desired_gear = forward_idx;
      }
    }

    if (valid) {
      const bool pedal_depressed = (input.pedal_position >= config_.bite_threshold);
      if (desired_gear != current_target_gear_ && !pedal_depressed) {
        if (config_.require_clutch_to_shift && !config_.auto_clutch_assist) {
          output.rejection = ShiftRejection::clutch_not_depressed;
          valid = false;
        } else if (config_.auto_clutch_assist) {
          is_syncing_ = true;
          sync_elapsed_ms_ = 0;
        }
      }
    }

    if (valid) {
      active_gate_ = pending_gate_;
      current_target_gear_ = desired_gear;

      if (active_gate_ == Gate::reverse) {
        current_hud_glyph_ = 'R';
      } else if (active_gate_ == Gate::neutral) {
        current_hud_glyph_ = 'N';
      } else {
        current_hud_glyph_ = '\0';
      }

      // Starting a shift enters open/sync state.
      is_syncing_ = false;
      sync_elapsed_ms_ = 0;
    }
  }

  // Evaluate clutch state and continuous engagement factor.
  const float bite = std::max(0.01f, config_.bite_threshold);
  const bool pedal_depressed = (input.pedal_position >= bite);

  if (active_gate_ == Gate::neutral) {
    // In neutral, drivetrain clutch is open (zero torque transfer, engine free-revs).
    current_clutch_state_ = ClutchState::open;
    output.clutch_engagement = 0.0f;
    is_syncing_ = false;
  } else {
    // In gear (reverse or forward 1..N).
    if (pedal_depressed) {
      current_clutch_state_ = ClutchState::open;
      output.clutch_engagement = 0.0f;
      is_syncing_ = true; // when pedal begins release, we will transition through syncing
      sync_elapsed_ms_ = 0;
    } else {
      // Pedal released or releasing.
      if (is_syncing_) {
        sync_elapsed_ms_ += delta_ms;
        if (sync_elapsed_ms_ < 35) { // ~2 physics ticks at 60 Hz for synchronisation
          current_clutch_state_ = ClutchState::syncing;
        } else {
          current_clutch_state_ = ClutchState::engaged;
          is_syncing_ = false;
        }
      } else {
        current_clutch_state_ = ClutchState::engaged;
      }

      // Linear engagement ramp between bite point and full release.
      const float release_fraction = (bite - input.pedal_position) / bite;
      output.clutch_engagement = std::clamp(release_fraction, 0.0f, 1.0f);
      if (current_clutch_state_ == ClutchState::engaged) {
        output.clutch_engagement = 1.0f;
      }
    }
  }

  output.target_gear = current_target_gear_;
  output.clutch_state = current_clutch_state_;
  output.hud_glyph = current_hud_glyph_;
  output.suppress_guest_shifts = true;
  return output;
}

} // namespace gt4::hshifter
