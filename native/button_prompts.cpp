#define _CRT_SECURE_NO_WARNINGS
#include "button_prompts.hpp"

#include <cstdio>
#include <cstdlib>
#include <cstring>

namespace gt4::button_prompts {
namespace {

Style g_style = Style::PlayStation;

}  // namespace

Style current_style() {
  return g_style;
}

bool parse_switch(const char *value, Style &style) {
  if (!value || *value == '\0' || std::strcmp(value, "playstation") == 0) {
    style = Style::PlayStation;
    return true;
  }
  if (std::strcmp(value, "xbox") == 0) {
    style = Style::Xbox;
    return true;
  }
  if (std::strcmp(value, "switch") == 0) {
    style = Style::Switch;
    return true;
  }
  return false;
}

void configure_from_environment() {
  const char *env = std::getenv(kSwitch);
  if (!parse_switch(env, g_style)) {
    std::fprintf(stderr,
                 "[prompts] refused invalid %s='%s': expected 'playstation', 'xbox', or 'switch'\n",
                 kSwitch, env ? env : "");
    std::exit(1);
  }
  if (g_style != Style::PlayStation) {
    std::fprintf(stderr,
                 "[prompts] in-game button prompt replacement active: %s\n",
                 g_style == Style::Xbox ? "Xbox" : "Nintendo Switch");
  }
}

void set_style_for_tests(Style style) {
  g_style = style;
}

}  // namespace gt4::button_prompts
