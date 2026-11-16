#pragma once
// TASK-165: controller button prompt style (Issue #161, Study #166).
//
// In-game button glyph replacement for menus and races.
// Replaces PlayStation glyphs with Xbox (A/B/X/Y) or Nintendo Switch (B/A/Y/X).
//
// Environment switch: GT4_BUTTON_PROMPTS
//   unset, empty, or "playstation" = default PlayStation button glyphs.
//   "xbox"   = Xbox style prompts.
//   "switch" = Nintendo Switch style prompts.
// Anything else refuses the start.
//
// game.ini key: button_prompts (written by the launcher).

#include <string_view>

namespace gt4::button_prompts {

inline constexpr const char *kSwitch = "GT4_BUTTON_PROMPTS";
inline constexpr const char *kIniKey = "button_prompts";

enum class Style {
  PlayStation,
  Xbox,
  Switch,
};

Style current_style();
bool parse_switch(const char *value, Style &style);

// Reads GT4_BUTTON_PROMPTS from the environment; an invalid value exits with status 1.
void configure_from_environment();

// For fixtures: sets style directly.
void set_style_for_tests(Style style);

}  // namespace gt4::button_prompts
