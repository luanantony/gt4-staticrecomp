// TASK-165: unit tests for controller button prompts configuration (Issue #161, Study #166).

#include "button_prompts.hpp"

#include <cstdio>
#include <cstdlib>
#include <string_view>

namespace {

void check(bool condition, const char *message) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", message);
    std::exit(1);
  }
}

}  // namespace

int main() {
  using namespace gt4::button_prompts;

  // Constants contract
  check(std::string_view(kSwitch) == "GT4_BUTTON_PROMPTS", "kSwitch matches GT4_BUTTON_PROMPTS");
  check(std::string_view(kIniKey) == "button_prompts", "kIniKey matches button_prompts");

  // Switch parsing
  Style style = Style::Xbox;
  check(parse_switch(nullptr, style) && style == Style::PlayStation, "nullptr parses as PlayStation");
  style = Style::Xbox;
  check(parse_switch("", style) && style == Style::PlayStation, "empty string parses as PlayStation");
  style = Style::Xbox;
  check(parse_switch("playstation", style) && style == Style::PlayStation, "'playstation' parses as PlayStation");

  check(parse_switch("xbox", style) && style == Style::Xbox, "'xbox' parses as Xbox");
  check(parse_switch("switch", style) && style == Style::Switch, "'switch' parses as Switch");

  check(!parse_switch("ps3", style), "'ps3' is refused");
  check(!parse_switch("nintendo", style), "'nintendo' is refused");
  check(!parse_switch("1", style), "'1' is refused");

  // State management
  set_style_for_tests(Style::PlayStation);
  check(current_style() == Style::PlayStation, "initial style is PlayStation");
  set_style_for_tests(Style::Xbox);
  check(current_style() == Style::Xbox, "style can be set to Xbox");
  set_style_for_tests(Style::Switch);
  check(current_style() == Style::Switch, "style can be set to Switch");

  // Reset to default
  set_style_for_tests(Style::PlayStation);
  std::printf("PASS: button prompts parsing and style configuration rules\n");
  return 0;
}
