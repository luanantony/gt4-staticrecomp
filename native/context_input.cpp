#include "context_input.hpp"

namespace gt4::context {

bool ContextInputFilter::filter_key(std::uint8_t vk, bool physically_pressed) {
  if (active_context_ == GameContext::Loading) {
    key_was_pressed_[vk] = physically_pressed;
    return false;
  }

  if (just_switched_) {
    // If key was already pressed at the moment of the switch, it must be re-armed.
    if (physically_pressed) {
      key_needs_rearm_[vk] = true;
    }
  }

  if (!physically_pressed) {
    // Release event clears the re-arm requirement.
    key_needs_rearm_[vk] = false;
    key_was_pressed_[vk] = false;
    return false;
  }

  // Key is physically pressed:
  key_was_pressed_[vk] = true;

  // On the exact switch frame, produce a clean release.
  if (just_switched_)
    return false;

  // If waiting for physical re-arm, ignore until released.
  if (key_needs_rearm_[vk])
    return false;

  return true;
}

std::uint16_t ContextInputFilter::filter_gamepad_buttons(std::uint16_t raw_buttons) {
  if (active_context_ == GameContext::Loading) {
    gamepad_was_pressed_ = raw_buttons;
    return 0;
  }

  if (just_switched_) {
    // Any button physically held when the context changes must be re-armed.
    gamepad_needs_rearm_ |= raw_buttons;
  }

  // Any released button has its re-arm flag cleared:
  // (~raw_buttons) are the bits currently released.
  gamepad_needs_rearm_ &= raw_buttons;
  gamepad_was_pressed_ = raw_buttons;

  // Clean transition frame:
  if (just_switched_)
    return 0;

  // Mask out any buttons still waiting for re-arm:
  return raw_buttons & ~gamepad_needs_rearm_;
}

void ContextInputFilter::reset() {
  key_needs_rearm_.fill(false);
  key_was_pressed_.fill(false);
  gamepad_needs_rearm_ = 0;
  gamepad_was_pressed_ = 0;
  just_switched_ = false;
}

} // namespace gt4::context
