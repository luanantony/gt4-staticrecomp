#pragma once

// TASK-077: Input filtering across context transitions (no stuck buttons).
//
// Rules implemented (docs/CONTEXT-INPUT-STUDY.md section 3.3):
//   1. Re-arm rule: a physical source held across a context switch does not fire in
//      the new context until it is physically released and pressed again.
//   2. Clean pad on transition: on the exact frame a context change occurs,
//      all logical buttons are released so the game receives a clean neutral state.
//   3. Loading: while context is Loading, logical pad outputs are held clean/released.

#include "context_probe.hpp"

#include <array>
#include <cstdint>

namespace gt4::context {

class ContextInputFilter {
public:
  ContextInputFilter() = default;

  // Called on each frame boundary with the newly debounced context.
  void set_context(GameContext new_context) {
    if (new_context != active_context_) {
      previous_context_ = active_context_;
      active_context_ = new_context;
      just_switched_ = true;
    } else {
      just_switched_ = false;
    }
  }

  GameContext active_context() const { return active_context_; }
  bool just_switched() const { return just_switched_; }

  // Filters raw physical keyboard keys (256 Win32 VK codes).
  // Returns true if `vk` is allowed to fire its logical mapping; false if it is
  // currently disarmed (waiting for release) or if the pad is in a transition frame.
  bool filter_key(std::uint8_t vk, bool physically_pressed);

  // Filters a digital gamepad button mask (bitmask of XINPUT_GAMEPAD_*).
  // Returns filtered button mask with held-across-switch buttons masked out until re-armed.
  std::uint16_t filter_gamepad_buttons(std::uint16_t raw_buttons);

  // Resets all state (e.g. on focus loss).
  void reset();

private:
  GameContext active_context_ = GameContext::Menu;
  GameContext previous_context_ = GameContext::Menu;
  bool just_switched_ = false;

  // Physical keyboard tracking:
  std::array<bool, 256> key_needs_rearm_{};
  std::array<bool, 256> key_was_pressed_{};

  // Gamepad button tracking:
  std::uint16_t gamepad_needs_rearm_ = 0;
  std::uint16_t gamepad_was_pressed_ = 0;
};

} // namespace gt4::context
