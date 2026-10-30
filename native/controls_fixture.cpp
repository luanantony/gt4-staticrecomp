// CTest fixture for the input binding table (controls.cpp). No Windows input or
// audio is touched: the readers are pure functions over raw XInput values.

#include "controls.hpp"

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
      {0x27, 1u << 5},  {'D', 1u << 5},       // D-pad right: Right, D
      {0x28, 1u << 6},                        // D-pad down: Down
      {0x25, 1u << 7},  {'A', 1u << 7},       // D-pad left: Left, A
      {'W', 1u << 14},  {0x0D, 1u << 14},     // Cross: W, Enter
      {0x20, 1u << 13},                       // Circle (handbrake): Space
      {'S', 1u << 15},                        // Square (brake): S
      {0x08, 1u << 12}, {0x1B, 1u << 12},     // Triangle (back): Backspace, Esc
      {'Q', 1u << 10},                        // L1 (look back): Q
      {0x10, 1u << 11},                       // R1 (nitro): Shift
      {'F', 1u << 8},                         // L2 (gear down): F
      {'R', 1u << 9},                         // R2 (gear up): R
      {'C', 1u << 0}, {0x09, 1u << 0},        // Select (camera): C, Tab
      {'P', 1u << 3}};                        // Start: P (hypothesis, see docs/CONTROLS.md)
  const auto keys = keyboard_bindings(default_controls());
  require(keys.size() == 20, "default keyboard table has 20 keys");
  for (const auto &expected : layout) {
    bool found = false;
    for (const auto &key : keys)
      found = found || (key.virtual_key == expected.first && key.button_mask == expected.second);
    require(found, "default keyboard key present");
  }
  std::fprintf(stderr, "PASS: default keyboard table is the TASK-076 layout\n");
}

void check_defaults_and_example_files() {
  const std::string directory = GT4_CONTROLS_EXAMPLES_DIR;
  const ControlMap from_file = load_controls_file(directory + "/default.toml");
  require(same_map(from_file, default_controls()),
          "default.toml equals the built-in defaults");
  const ControlMap pedals = load_controls_file(directory + "/triggers-as-pedals.toml");
  require(only_binding(pedals, "l2").kind == SourceKind::xinput_trigger &&
              only_binding(pedals, "r2").kind == SourceKind::xinput_trigger,
          "pedals example binds triggers to L2 and R2");
  const auto &pedal_l2 = only_binding(pedals, "l2");
  const auto &pedal_r2 = only_binding(pedals, "r2");
  require(pedal_l2.scale > 0.35f && pedal_l2.scale < 0.36f &&
              pedal_r2.scale > 0.35f && pedal_r2.scale < 0.36f,
          "pedals example scales triggers for full-range DS2 pressure");
  require(read_trigger(pedal_l2, 255).pressure == 90 &&
              read_trigger(pedal_r2, 255).pressure == 90,
          "pedals example maps full physical throw to in-game saturation (0x5A / 90)");
  require(pedals.bindings[input_index_of("cross")].size() == 3 &&
              pedals.bindings[input_index_of("cross")][0].button_mask == 0x1000,
          "pedals example keeps the default cross binding (XInput A, keys W and Enter)");
  const ControlMap keyboard = load_controls_file(directory + "/keyboard-only.toml");
  require(keyboard.bindings[input_index_of("lstick_x")].empty() &&
              keyboard.bindings[input_index_of("rstick_y")].empty(),
          "keyboard-only example leaves sticks unbound");
  require(keyboard_bindings(keyboard).size() == 16, "keyboard-only example binds 16 keys");
  std::fprintf(stderr, "PASS: default.toml equals defaults; example files parse\n");
}

void check_trigger_readings() {
  const Binding &left = only_binding(default_controls(), "l2");
  std::uint8_t previous = 0;
  for (int raw = 0; raw <= 255; ++raw) {
    const auto reading = read_trigger(left, static_cast<std::uint8_t>(raw));
    const auto expected = legacy_trigger(static_cast<std::uint8_t>(raw));
    require(reading.pressed == expected.pressed && reading.pressure == expected.pressure,
            "default trigger reading equals slice 157 at every raw value");
    require(reading.pressure >= previous, "trigger pressure is monotone");
    previous = reading.pressure;
  }
  require(!read_trigger(left, 30).pressed, "trigger deadzone: 30 is at rest");
  require(read_trigger(left, 31).pressure == 1, "trigger first step is pressure 1");
  require(read_trigger(left, 255).pressure == 255, "trigger full travel is pressure 255");

  // Deadzone 0.5 (raw 128 of 255): rest up to 128, then 255 / 127 per unit.
  const Binding half = only_binding(parse_text("schema = 1\n[bindings]\nl2 = [\"xinput:LT | deadzone=0.5\"]\n"), "l2");
  require(!read_trigger(half, 128).pressed && read_trigger(half, 129).pressure == 2,
          "custom trigger deadzone sets the rest point");

  // Quadratic: half travel gives about a quarter of full pressure.
  const Binding quadratic = only_binding(
      parse_text("schema = 1\n[bindings]\nl2 = [\"xinput:LT | curve=quadratic\"]\n"), "l2");
  const auto middle = read_trigger(quadratic, 142).pressure;
  require(middle >= 62 && middle <= 64, "quadratic curve: half travel near quarter pressure");

  // S-Curve, smooth power, sublinear and logarithmic curves
  const Binding scurve = only_binding(
      parse_text("schema = 1\n[bindings]\nl2 = [\"xinput:LT | curve=s_curve\"]\n"), "l2");
  const auto scurve_mid = read_trigger(scurve, 142).pressure;
  require(scurve_mid >= 125 && scurve_mid <= 128, "s-curve at half travel is near half pressure");

  const Binding sublinear = only_binding(
      parse_text("schema = 1\n[bindings]\nl2 = [\"xinput:LT | curve=sublinear\"]\n"), "l2");
  const auto sublinear_mid = read_trigger(sublinear, 142).pressure;
  require(sublinear_mid >= 175 && sublinear_mid <= 185, "sublinear curve at half travel is near 70% pressure");

  // Exact byte vectors also used by the browser preview checks: raw 128, zero deadzone.
  const struct { const char *name; std::uint8_t middle; } curves[] = {
      {"linear", 128}, {"quadratic", 64}, {"cubic", 32}, {"s_curve", 128},
      {"smooth_power", 90}, {"sublinear", 180}, {"logarithmic", 189}};
  for (const auto &curve : curves) {
    const auto controls = parse_text(std::string("schema = 1\n[bindings]\nl2 = [\"xinput:LT | deadzone=0 curve=") +
                                    curve.name + "\"]\n");
    const auto &binding = only_binding(controls, "l2");
    require(read_trigger(binding, 0).pressure == 0, "every curve rests at zero");
    require(read_trigger(binding, 128).pressure == curve.middle, "quantized curve middle vector");
    require(read_trigger(binding, 255).pressure == 255, "every curve reaches full pressure");
    std::uint8_t prior = 0;
    for (int raw = 0; raw <= 255; ++raw) {
      const auto pressure = read_trigger(binding, static_cast<std::uint8_t>(raw)).pressure;
      require(pressure >= prior, "all curve pressure bytes are monotone");
      prior = pressure;
    }
  }

  // Scale 0.5 halves full travel; scale 2 is clamped at full pressure.
  const Binding half_scale = only_binding(
      parse_text("schema = 1\n[bindings]\nr2 = [\"xinput:RT | scale=0.5\"]\n"), "r2");
  require(read_trigger(half_scale, 255).pressure == 127, "scale 0.5 halves full travel");
  const Binding double_scale = only_binding(
      parse_text("schema = 1\n[bindings]\nr2 = [\"xinput:RT | scale=2\"]\n"), "r2");
  require(read_trigger(double_scale, 255).pressure == 255, "scale 2 clamps at full pressure");
  std::fprintf(stderr, "PASS: trigger readings: slice 157 match, deadzone, curve, scale\n");
}

void check_axis_readings() {
  const auto &map = default_controls();
  const struct {
    const char *name;
    int deadzone;
    bool invert;
  } sticks[] = {{"rstick_x", 8689, false}, {"rstick_y", 8689, true},
                {"lstick_x", 7849, false}, {"lstick_y", 7849, true}};
  for (const auto &stick : sticks) {
    const Binding &binding = only_binding(map, stick.name);
    for (int raw = -32768; raw <= 32767; ++raw) {
      const auto value = static_cast<std::int16_t>(raw);
      require(read_axis(binding, value) == legacy_axis(value, stick.deadzone, stick.invert),
              std::string("default axis equals slice 157: ") + stick.name);
    }
  }
  // A custom deadzone and an inverted sign keep the rest value at 128 inside the zone.
  const ControlMap custom = parse_text(
      "schema = 1\n[bindings]\nlstick_y = [\"xinput:LY | deadzone=0.5 invert=true\"]\n");
  const Binding &custom_y = only_binding(custom, "lstick_y");
  require(read_axis(custom_y, 16000) == 128 && read_axis(custom_y, 0) == 128,
          "custom stick deadzone holds the rest value");
  require(read_axis(custom_y, 32767) < 128, "inverted stick: full up reads low");
  std::fprintf(stderr, "PASS: stick readings equal slice 157 for all 65536 raw values\n");
}

void check_remaps() {
  const ControlMap remap = parse_text(
      "schema = 1\n[bindings]\ncross = [\"key:J\"]\nsquare = [\"xinput:X\", \"key:H\"]\n"
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
  expect_config_error("", "missing schema = 1", 1, "empty file");
  expect_config_error("schema = 2\n", "schema must be 1", 1, "wrong schema");
  expect_config_error("schema = 1\n[pad]\n", "unknown table [pad]", 2, "unknown table");
  expect_config_error("schema = 1\n[bindings]\ntriangel = [\"key:S\"]\n", "unknown input 'triangel'",
                      3, "misspelled input");
  expect_config_error("schema = 1\n[bindings]\ncross = [\"xinput:Q\"]\n", "unknown XInput source",
                      3, "unknown XInput name");
  expect_config_error("schema = 1\n[bindings]\ncross = [\"key:Nope\"]\n", "unknown key", 3,
                      "unknown key name");
  expect_config_error("schema = 1\n[bindings]\ncross = [\"key:F8\"]\n", "reserved", 3,
                      "host shortcut key");
  expect_config_error("schema = 1\n[bindings]\nlstick_x = [\"xinput:A\"]\n", "is a stick axis", 3,
                      "button bound to an axis");
  expect_config_error("schema = 1\n[bindings]\ncross = [\"xinput:LX\"]\n",
                      "a stick cannot drive it", 3, "stick bound to a button");
  expect_config_error("schema = 1\n[bindings]\ncross = [\"key:Z | deadzone=0.1\"]\n",
                      "digital sources take no options", 3, "options on a key");
  expect_config_error("schema = 1\n[bindings]\nl2 = [\"xinput:LT | deadzone=1\"]\n",
                      "deadzone must be", 3, "deadzone of one");
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
                      "duplicate binding for cross", 4, "duplicate input");
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
    check_environment_selection();
    return 0;
  } catch (const std::exception &error) {
    std::fprintf(stderr, "FAIL: %s\n", error.what());
    return 1;
  }
}
