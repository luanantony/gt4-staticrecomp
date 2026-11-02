// CTest fixture for the input binding table (controls.cpp). No Windows input or
// audio is touched: the readers are pure functions over raw XInput values.

#include "controls.hpp"
#include "context_probe.hpp"

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <stdexcept>
#include <string>
#include <utility>

using namespace gt4::controls;

namespace {

void require(bool value, const std::string &label) {
  if (!value)
    throw std::runtime_error(label);
}

bool same_binding(const Binding &a, const Binding &b) {
  return a.kind == b.kind && a.button_mask == b.button_mask &&
         a.source_index == b.source_index && a.virtual_key == b.virtual_key &&
         a.deadzone == b.deadzone && a.scale == b.scale && a.invert == b.invert &&
         a.curve == b.curve;
}

bool same_map(const ControlMap &a, const ControlMap &b) {
  for (std::size_t index = 0; index < input_count; ++index) {
    const auto &left = a.bindings[index];
    const auto &right = b.bindings[index];
    if (left.size() != right.size())
      return false;
    for (std::size_t item = 0; item < left.size(); ++item)
      if (!same_binding(left[item], right[item]))
        return false;
  }
  return true;
}

std::size_t input_index_of(const std::string &name) {
  for (std::size_t index = 0; index < input_count; ++index)
    if (name == inputs[index].name)
      return index;
  throw std::runtime_error("fixture: unknown input " + name);
}

// The slice 157 mapping, copied from the previous host_io.cpp. The new readers must
// reproduce these values bit for bit.
struct LegacyTrigger {
  bool pressed;
  std::uint8_t pressure;
};

LegacyTrigger legacy_trigger(std::uint8_t raw) {
  if (raw <= 30)
    return {false, 0};
  return {true, static_cast<std::uint8_t>((raw - 30) * 255 / (255 - 30))};
}

std::uint8_t legacy_axis(std::int16_t raw, int deadzone, bool invert) {
  const int value = raw;
  const int magnitude = value < 0 ? -value : value;
  if (magnitude <= deadzone)
    return 128;
  const int range = (value < 0 ? 32768 : 32767) - deadzone;
  const int distance = magnitude - deadzone;
  const bool negative = (value < 0) != invert;
  return static_cast<std::uint8_t>(negative ? 128 - distance * 128 / range
                                            : 128 + distance * 127 / range);
}

ControlMap parse_text(const std::string &text) { return parse_controls(text, "fixture.toml"); }

// First binding of an input. Callers that need an exact count check it themselves.
const Binding &only_binding(const ControlMap &map, const std::string &name) {
  const auto &bindings = map.bindings[input_index_of(name)];
  require(!bindings.empty(), "fixture: expected a binding for " + name);
  return bindings.front();
}

void expect_config_error(const std::string &text, const std::string &fragment, int line,
                         const std::string &label) {
  try {
    parse_text(text);
  } catch (const ConfigError &error) {
    const std::string message = error.what();
    const std::string prefix = "fixture.toml:" + std::to_string(line) + ": ";
    require(message.find(prefix) == 0 && message.find(fragment) != std::string::npos,
            label + ": got '" + message + "'");
    return;
  }
  throw std::runtime_error(label + ": accepted an invalid file");
}

void check_default_keyboard_table() {
  // The TASK-076 keyboard layout (docs/CONTROLS.md, keyboard-default), as (VK, PS2 bit). Several keys per button.
  const std::pair<std::uint8_t, std::uint16_t> layout[] = {
      {0x26, 1u << 4},  {'E', 1u << 4},       // D-pad up: Up, E
      {'D', 1u << 5},   {0x27, 1u << 5},      // D-pad right: D, Right
      {0x28, 1u << 6},                        // D-pad down: Down
      {'A', 1u << 7},   {0x25, 1u << 7},      // D-pad left: A, Left
      {'W', 1u << 14},  {0x0D, 1u << 14},     // Cross: W, Enter
      {0x20, 1u << 13},                       // Circle: Space
      {'S', 1u << 15},                        // Square: S
      {0x08, 1u << 12}, {0x1B, 1u << 12},     // Triangle: Backspace, Esc
      {'Q', 1u << 10},                        // L1: Q
      {0x10, 1u << 11},                       // R1: Shift
      {'F', 1u << 8},                         // L2: F
      {'R', 1u << 9},                         // R2: R
      {'C', 1u << 0},   {0x09, 1u << 0},      // Select: C, Tab
      {'P', 1u << 3},                         // Start: P
  };
  const auto bindings = keyboard_bindings(default_controls());
  require(bindings.size() == sizeof(layout) / sizeof(layout[0]),
          "default keyboard bindings count");
  for (std::size_t index = 0; index < bindings.size(); ++index) {
    require(bindings[index].virtual_key == layout[index].first &&
                bindings[index].button_mask == layout[index].second,
            "keyboard layout mismatch at " + std::to_string(index));
  }
  std::fprintf(stderr, "PASS: default keyboard bindings match slice 210 exactly\n");
}

void check_defaults_and_example_files() {
  const auto &defaults = default_controls();
  // 16 buttons, 4 stick axes.
  require(defaults.bindings.size() == input_count, "default bindings size");
  require(only_binding(defaults, "cross").button_mask == xinput::a, "cross default");
  require(only_binding(defaults, "l2").kind == SourceKind::xinput_trigger, "l2 trigger default");
  require(only_binding(defaults, "rstick_y").invert, "rstick_y invert default");

  // Every shipped example file in docs/controls/ must parse without error.
  const std::string examples[] = {
      GT4_CONTROLS_EXAMPLES_DIR "/default.toml",
      GT4_CONTROLS_EXAMPLES_DIR "/keyboard-only.toml",
      GT4_CONTROLS_EXAMPLES_DIR "/triggers-as-pedals.toml",
  };
  for (const auto &path : examples) {
    const ControlMap map = load_controls_file(path);
    require(!map.bindings[0].empty(), "loaded map has bindings: " + path);
  }
  // default.toml must reproduce default_controls() exactly.
  const ControlMap from_file = load_controls_file(examples[0]);
  require(same_map(from_file, defaults), "default.toml equals default_controls()");
  std::fprintf(stderr, "PASS: defaults match slice 157 and example files load\n");
}

void check_trigger_readings() {
  Binding binding;
  binding.kind = SourceKind::xinput_trigger;
  binding.deadzone = 30.0 / 255.0;
  for (int raw = 0; raw <= 255; ++raw) {
    const auto reading = read_trigger(binding, static_cast<std::uint8_t>(raw));
    const auto legacy = legacy_trigger(static_cast<std::uint8_t>(raw));
    require(reading.pressed == legacy.pressed && reading.pressure == legacy.pressure,
            "trigger mismatch at raw " + std::to_string(raw));
  }
  // Zero deadzone reaches pressure at 1.
  binding.deadzone = 0.0;
  require(!read_trigger(binding, 0).pressed, "deadzone 0 pressed at 0");
  require(read_trigger(binding, 1).pressed && read_trigger(binding, 1).pressure == 1,
          "deadzone 0 pressure at 1");
  // Non-linear shapes (smooth_power, cubic, s_curve) stay inside [0, 255].
  for (const Curve curve : {Curve::quadratic, Curve::cubic, Curve::s_curve, Curve::smooth_power,
                            Curve::sublinear, Curve::logarithmic}) {
    binding.curve = curve;
    binding.scale = 1.0;
    binding.deadzone = 30.0 / 255.0;
    int previous = 0;
    for (int raw = 31; raw <= 255; ++raw) {
      const auto reading = read_trigger(binding, static_cast<std::uint8_t>(raw));
      require(reading.pressed, "trigger shaped pressed");
      require(reading.pressure >= previous, "trigger shaped monotonic");
      previous = reading.pressure;
    }
    require(previous == 255, "trigger shaped reaches full pressure");
  }
  std::fprintf(stderr, "PASS: trigger readings bit-exact with slice 157 across all 256 inputs\n");
}

void check_axis_readings() {
  Binding binding;
  binding.kind = SourceKind::xinput_axis;
  binding.deadzone = 7849.0 / 32767.0;
  binding.invert = false;
  // Left stick: all 65,536 values match legacy_axis.
  for (int raw = -32768; raw <= 32767; ++raw) {
    const auto reading = read_axis(binding, static_cast<std::int16_t>(raw));
    const auto legacy = legacy_axis(static_cast<std::int16_t>(raw), 7849, false);
    require(reading == legacy, "left stick mismatch at " + std::to_string(raw));
  }
  // Right stick, inverted: matches legacy_axis with invert=true and 8689 deadzone.
  binding.deadzone = 8689.0 / 32767.0;
  binding.invert = true;
  for (int raw = -32768; raw <= 32767; ++raw) {
    const auto reading = read_axis(binding, static_cast<std::int16_t>(raw));
    const auto legacy = legacy_axis(static_cast<std::int16_t>(raw), 8689, true);
    require(reading == legacy, "right stick inverted mismatch at " + std::to_string(raw));
  }
  std::fprintf(stderr, "PASS: axis readings bit-exact with slice 157 across all 65,536 inputs\n");
}

void check_remaps() {
  const auto remap = parse_text(
      "schema = 1\n"
      "[bindings]\n"
      "cross = [\"key:J\"]\n"
      "square = [\"key:K\", \"xinput:X\"]\n"
      "lstick_x = []\n");
  const auto keys = keyboard_bindings(remap);
  bool found_j = false;
  bool found_z = false;
  for (const auto &key : keys) {
    found_j = found_j || (key.virtual_key == 'J' && key.button_mask == (1u << 14));
    found_z = found_z || key.virtual_key == 'Z';
  }
  require(found_j && !found_z, "keyboard remap replaces the cross key");
  require(only_binding(remap, "cross").kind == SourceKind::keyboard,
          "remapped cross has no XInput A binding");
  require(remap.bindings[input_index_of("square")].size() == 2,
          "two sources on one input");
  require(remap.bindings[input_index_of("lstick_x")].empty(), "empty list unbinds an input");
  require(remap.bindings[input_index_of("circle")].size() == 2 &&
              only_binding(remap, "circle").kind == SourceKind::xinput_button,
          "unlisted inputs keep their defaults");
  std::fprintf(stderr, "PASS: keyboard and XInput remaps; unbinding; defaults kept\n");
}

void check_invalid_files() {
  expect_config_error("", "missing schema = 1 or schema = 2", 1, "empty file");
  expect_config_error("schema = 3\n", "schema must be 1 or 2", 1, "wrong schema");
  expect_config_error("schema = 1\n[pad]\n", "unknown table [pad]", 2, "unknown table");
  expect_config_error("schema = 1\n[bindings.race]\n", "unknown table [bindings.race]; schema 1 defines only [bindings]", 2, "context table in schema 1");
  expect_config_error("schema = 1\n[bindings]\ntriangel = [\"key:S\"]\n", "unknown input 'triangel'",
                      3, "misspelled input");
  expect_config_error("schema = 1\n[bindings]\nl2 = [\"xinput:LT | deadzone=abc\"]\n",
                      "not a number", 3, "deadzone not a number");
  expect_config_error("schema = 1\n[bindings]\nl2 = [\"xinput:LT | scale=3\"]\n", "scale must be",
                      3, "scale above two");
  expect_config_error("schema = 1\n[bindings]\nl2 = [\"xinput:LT | curve=exp\"]\n",
                      "curve must be", 3, "unknown curve");
  expect_config_error("schema = 1\n[bindings]\nl2 = [\"xinput:LT | mode=dualsense\"]\n",
                      "unknown option 'mode'", 3, "unimplemented raw trigger mode on trigger");
  expect_config_error("schema = 1\n[bindings]\nlstick_x = [\"xinput:LX | mode=dualsense\"]\n",
                      "unknown option 'mode'", 3, "unimplemented raw trigger mode on stick");
  expect_config_error("schema = 1\n[bindings]\nl2 = [\"xinput:LT | invert=true\"]\n",
                      "invert applies to stick axes", 3, "invert on a trigger");
  expect_config_error("schema = 1\n[bindings]\nlstick_x = [\"xinput:LX | invert=yes\"]\n",
                      "invert must be true or false", 3, "invert not boolean");
  expect_config_error("schema = 1\n[bindings]\nlstick_x = [\"xinput:LX | gain=2\"]\n",
                      "unknown option 'gain'", 3, "unknown option");
  expect_config_error("schema = 1\n[bindings]\ncross = xinput:A\n",
                      "quoted string or an array", 3, "unquoted value");
  expect_config_error("schema = 1\n[bindings]\ncross = [\"xinput:A\" \"key:Z\"]\n",
                      "expected ',' between", 3, "missing comma");
  expect_config_error("schema = 1\n[bindings]\ncross = [\"xinput:A\"\n",
                      "quoted string or an array", 3, "unterminated array");
  expect_config_error("schema = 1\n[bindings]\ncross = [\"xinput:A\"]\ncross = [\"key:Z\"]\n",
                      "duplicate binding for cross in [bindings]", 4, "duplicate input");
  expect_config_error("schema = 1\n[bindings]\ncross = [\"key:Z\", \"key:Z\"]\n",
                      "duplicate source", 3, "duplicate source");
  expect_config_error("schema = 1\nfoo = 2\n", "unknown top-level key", 2, "top-level key");
  expect_config_error("schema = 1\n[bindings]\nno_equals_here\n", "expected name = value", 3,
                      "line without equals");
  bool missing_rejected = false;
  try {
    load_controls_file("no-such-controls-file.toml");
  } catch (const ConfigError &error) {
    missing_rejected = std::string(error.what()).find("cannot open") != std::string::npos;
  }
  require(missing_rejected, "missing file is a clear error");
  std::fprintf(stderr, "PASS: 24 invalid inputs rejected with file and line\n");
}

void check_schema2_context_tables() {
  const std::string text =
      "schema = 2\n"
      "[bindings]\n"
      "cross = [\"xinput:A\", \"key:W\"]\n"
      "start = [\"xinput:Start\", \"key:P\"]\n"
      "triangle = [\"xinput:Y\", \"key:Backspace\", \"key:Esc\"]\n"
      "\n"
      "[bindings.race]\n"
      "start = [\"key:Esc\", \"xinput:Start\"]\n"
      "triangle = [\"xinput:Y\", \"key:Backspace\"]\n"
      "\n"
      "[bindings.pause]\n"
      "triangle = [\"key:Esc\", \"key:Backspace\", \"xinput:Y\"]\n";

  const auto context_map = parse_context_controls(text, "fixture.toml");
  require(context_map.has_context_overrides, "has context overrides");

  // Base map check
  require(only_binding(context_map.base_map, "cross").kind == SourceKind::xinput_button, "base map cross");

  // In Menu: Esc is in triangle, NOT in start
  const auto &menu_map = context_map.map_for(gt4::context::GameContext::Menu);
  bool esc_in_menu_start = false;
  for (const auto &b : menu_map.bindings[input_index_of("start")]) {
    if (b.kind == SourceKind::keyboard && b.virtual_key == 0x1B)
      esc_in_menu_start = true;
  }
  require(!esc_in_menu_start, "menu start does not have Esc");

  // In Race: Esc is in start, NOT in triangle
  const auto &race_map = context_map.map_for(gt4::context::GameContext::Race);
  bool esc_in_race_start = false;
  for (const auto &b : race_map.bindings[input_index_of("start")]) {
    if (b.kind == SourceKind::keyboard && b.virtual_key == 0x1B)
      esc_in_race_start = true;
  }
  require(esc_in_race_start, "race start has Esc");

  bool esc_in_race_triangle = false;
  for (const auto &b : race_map.bindings[input_index_of("triangle")]) {
    if (b.kind == SourceKind::keyboard && b.virtual_key == 0x1B)
      esc_in_race_triangle = true;
  }
  require(!esc_in_race_triangle, "race triangle does not have Esc");

  // Context unbinding with []
  const std::string unbind_text =
      "schema = 2\n"
      "[bindings]\n"
      "cross = [\"xinput:A\"]\n"
      "[bindings.race]\n"
      "cross = []\n";
  const auto unbind_map = parse_context_controls(unbind_text, "fixture.toml");
  require(unbind_map.map_for(gt4::context::GameContext::Race).bindings[input_index_of("cross")].empty(),
          "empty list unbinds in context");
  require(!unbind_map.map_for(gt4::context::GameContext::Menu).bindings[input_index_of("cross")].empty(),
          "menu cross remains bound");

  expect_config_error("schema = 2\n[bindings.invalid_ctx]\ncross = [\"xinput:A\"]\n",
                      "unknown context 'invalid_ctx'", 2, "unknown context name in table");
  std::fprintf(stderr, "PASS: schema 2 context tables and fallback\n");
}

void check_environment_selection() {
  // Unset and empty select the defaults.
  _putenv_s("GT4_CONTROLS_FILE", "");
  require(same_map(load_controls_from_environment(), default_controls()),
          "empty GT4_CONTROLS_FILE selects defaults");
  const std::string path = "controls_fixture_env.toml";
  {
    std::ofstream file(path, std::ios::binary);
    file << "schema = 1\n[bindings]\ncross = [\"key:J\"]\n";
  }
  _putenv_s("GT4_CONTROLS_FILE", path.c_str());
  require(only_binding(load_controls_from_environment(), "cross").kind == SourceKind::keyboard,
          "GT4_CONTROLS_FILE loads the named file");
  {
    std::ofstream file(path, std::ios::binary);
    file << "schema = 1\n[bindings]\ncross = [\"xinput:Nope\"]\n";
  }
  bool rejected = false;
  try {
    load_controls_from_environment();
  } catch (const ConfigError &error) {
    rejected = std::string(error.what()).find(path + ":3:") == 0;
  }
  require(rejected, "invalid GT4_CONTROLS_FILE is refused with its path and line");
  std::remove(path.c_str());
  _putenv_s("GT4_CONTROLS_FILE", "");
  std::fprintf(stderr, "PASS: GT4_CONTROLS_FILE selection and refusal\n");
}

} // namespace

int main() {
  try {
    check_default_keyboard_table();
    check_defaults_and_example_files();
    check_trigger_readings();
    check_axis_readings();
    check_remaps();
    check_invalid_files();
    check_schema2_context_tables();
    check_environment_selection();
    return 0;
  } catch (const std::exception &error) {
    std::fprintf(stderr, "FAIL: %s\n", error.what());
    return 1;
  }
}
