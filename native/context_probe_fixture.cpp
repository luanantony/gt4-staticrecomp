// CTest fixture for TASK-077 context probe and input filter.
// Pure tests: synthetic guest RAM, no Windows window or GPU required.

#include "context_probe.hpp"
#include "context_input.hpp"
#include "controls.hpp"

#include <cstdio>
#include <cstdlib>
#include <stdexcept>
#include <string>
#include <vector>

using namespace gt4::context;

namespace {

void require(bool condition, const std::string &message) {
  if (!condition)
    throw std::runtime_error("FAIL: " + message);
}

void write_le32(std::uint8_t *bytes, std::uint32_t value) {
  for (int i = 0; i < 4; ++i)
    bytes[i] = static_cast<std::uint8_t>(value >> (8 * i));
}

void test_names_and_parsing() {
  require(std::string(context_name(GameContext::Menu)) == "menu", "context_name(Menu)");
  require(std::string(context_name(GameContext::Race)) == "race", "context_name(Race)");
  require(std::string(context_name(GameContext::Pause)) == "pause", "context_name(Pause)");
  require(std::string(context_name(GameContext::PhotoPause)) == "photo_pause", "context_name(PhotoPause)");
  require(std::string(context_name(GameContext::Unknown)) == "unknown", "context_name(Unknown)");

  require(parse_context_name("menu") == GameContext::Menu, "parse menu");
  require(parse_context_name("race") == GameContext::Race, "parse race");
  require(parse_context_name("pause") == GameContext::Pause, "parse pause");
  require(parse_context_name("photo_pause") == GameContext::PhotoPause, "parse photo_pause");
  require(parse_context_name("photo") == GameContext::PhotoPause, "parse photo");
  require(parse_context_name("something_invalid") == GameContext::Unknown, "parse unknown");
}

void test_vtable_predicates() {
  require(is_race_vtable(kVtableRaceSinglePlayer), "RaceSinglePlayer vtable");
  require(is_race_vtable(kVtableRaceArcade), "RaceArcade vtable");
  require(is_race_vtable(kVtableRaceArcadeSingle), "RaceArcadeSingle vtable");
  require(is_race_vtable(kVtableRaceGTmode), "RaceGTmode vtable");
  require(is_race_vtable(kVtableRaceFreeRun), "RaceFreeRun vtable");
  require(is_race_vtable(kVtableRaceLicense), "RaceLicense vtable");
  require(is_race_vtable(kVtableRaceSolitaire), "RaceSolitaire vtable");
  require(is_race_vtable(kVtableRacePhotoDevelop), "RacePhotoDevelop vtable");
  require(is_race_vtable(kVtableRacePhotoMode), "RacePhotoMode vtable");
  require(is_race_vtable(kVtableRacePS2Base), "RacePS2Base vtable");
  require(!is_race_vtable(0x12345678), "invalid race vtable");

  require(is_attract_demo_vtable(kVtableRaceArcadeDemo), "RaceArcadeDemo attract vtable");
  require(!is_race_vtable(kVtableRaceArcadeDemo), "RaceArcadeDemo is separate from active race vtable");

  require(is_pause_vtable(kVtableRacePause), "RacePause vtable");
  require(is_pause_vtable(kVtableSimplePause), "SimplePause vtable");
  require(!is_pause_vtable(0), "null pause vtable");

  require(is_photo_pause_vtable(kVtablePhotoPause), "PhotoPause vtable");
  require(!is_photo_pause_vtable(kVtableRacePause), "RacePause is not PhotoPause");
}

void test_synthetic_ram_detection() {
  // 32 MiB synthetic guest RAM
  constexpr std::uint32_t ram_size = 32 * 1024 * 1024;
  std::vector<std::uint8_t> ram(ram_size, 0);

  std::uint32_t cached_offset = 0;

  // 1. All zero memory: raw context is Menu (owner is null)
  require(detect_context_raw(ram.data(), ram_size, cached_offset) == GameContext::Menu,
          "all-zero RAM -> Menu");

  // 2. Setup owner pointer at 0x622F4C pointing to 0x00A00000
  constexpr std::uint32_t owner_ptr = 0x00A00000;
  write_le32(ram.data() + kOwnerPointerAddress, owner_ptr);

  // Still no race object inside owner -> Menu
  require(detect_context_raw(ram.data(), ram_size, cached_offset) == GameContext::Menu,
          "empty owner -> Menu");

  // 3. Put race object pointer inside owner at offset 0x200 (slot = 0x00A00200)
  constexpr std::uint32_t race_obj = 0x01500000;
  write_le32(ram.data() + owner_ptr + 0x200, race_obj);
  write_le32(ram.data() + race_obj, kVtableRaceGTmode);

  // Now raw context must be Race:
  cached_offset = 0;
  GameContext detected = detect_context_raw(ram.data(), ram_size, cached_offset);
  require(detected == GameContext::Race, "GTmode race detected");
  require(cached_offset == 0x200, "cached offset resolved to 0x200");

  // 4. Fast path using cached offset
  detected = detect_context_raw(ram.data(), ram_size, cached_offset);
  require(detected == GameContext::Race, "cached offset fast path");
  require(cached_offset == 0x200, "cached offset maintained");

  // 5. Active pause menu in race (+0xE418)
  constexpr std::uint32_t pause_obj = 0x01800000;
  write_le32(ram.data() + race_obj + kRacePausePointerOffset, pause_obj);
  write_le32(ram.data() + pause_obj + kPauseObjectVtableOffset, kVtableRacePause);

  detected = detect_context_raw(ram.data(), ram_size, cached_offset);
  require(detected == GameContext::Pause, "race pause detected");

  // 6. Photo pause in race
  write_le32(ram.data() + pause_obj + kPauseObjectVtableOffset, kVtablePhotoPause);
  detected = detect_context_raw(ram.data(), ram_size, cached_offset);
  require(detected == GameContext::PhotoPause, "photo pause detected");

  // 7. Clear pause -> back to Race
  write_le32(ram.data() + race_obj + kRacePausePointerOffset, 0);
  detected = detect_context_raw(ram.data(), ram_size, cached_offset);
  require(detected == GameContext::Race, "unpaused race restored");

  // 8. Attract mode arcade demo -> treated as Menu
  write_le32(ram.data() + race_obj, kVtableRaceArcadeDemo);
  detected = detect_context_raw(ram.data(), ram_size, cached_offset);
  require(detected == GameContext::Menu, "attract arcade demo -> Menu");
}

void test_debounce_state_machine() {
  constexpr std::uint32_t ram_size = 32 * 1024 * 1024;
  std::vector<std::uint8_t> ram(ram_size, 0);

  ContextProbeState state;
  state.debounce_threshold = 3;
  state.active_context = GameContext::Menu;
  state.raw_context = GameContext::Menu;

  // Frame 1: empty RAM -> Menu stays Menu
  update_context_at_vblank(ram.data(), ram_size, state);
  require(state.active_context == GameContext::Menu, "frame 1 menu");
  require(state.transitions == 0, "no transitions yet");

  // Now create a Race in RAM
  constexpr std::uint32_t owner_ptr = 0x00A00000;
  constexpr std::uint32_t race_obj = 0x01500000;
  write_le32(ram.data() + kOwnerPointerAddress, owner_ptr);
  write_le32(ram.data() + owner_ptr + 0x100, race_obj);
  write_le32(ram.data() + race_obj, kVtableRaceArcade);

  // Frame 2: first frame of Race -> raw is Race, active still Menu (debounce 1/3)
  update_context_at_vblank(ram.data(), ram_size, state);
  require(state.raw_context == GameContext::Race, "frame 2 raw race");
  require(state.active_context == GameContext::Menu, "frame 2 active menu");
  require(state.stable_count == 1, "stable_count 1");

  // Frame 3: second frame of Race -> active still Menu (debounce 2/3)
  update_context_at_vblank(ram.data(), ram_size, state);
  require(state.active_context == GameContext::Menu, "frame 3 active menu");
  require(state.stable_count == 2, "stable_count 2");

  // Frame 4: third frame of Race -> active switches to Race!
  update_context_at_vblank(ram.data(), ram_size, state);
  require(state.active_context == GameContext::Race, "frame 4 active race");
  require(state.stable_count == 3, "stable_count 3");
  require(state.transitions == 1, "transition recorded");

  // An isolated 1-frame glitch back to Menu does not change active context
  write_le32(ram.data() + race_obj, 0); // simulate 1 bad read
  update_context_at_vblank(ram.data(), ram_size, state);
  require(state.raw_context == GameContext::Menu, "frame 5 raw glitch menu");
  require(state.active_context == GameContext::Race, "frame 5 active held at race");
  require(state.stable_count == 1, "stable_count reset to 1");

  // Next frame race is restored
  write_le32(ram.data() + race_obj, kVtableRaceArcade);
  update_context_at_vblank(ram.data(), ram_size, state);
  require(state.active_context == GameContext::Race, "frame 6 still race");
}

void test_context_input_filter() {
  ContextInputFilter filter;
  constexpr std::uint8_t vk_esc = 0x1B;

  // In Menu context: Esc is pressed
  filter.set_context(GameContext::Menu);
  require(filter.filter_key(vk_esc, true), "esc pressed in menu passes");

  // Transition to Race context while Esc is still held:
  filter.set_context(GameContext::Race);
  // Switch frame: must return false (clean release)
  require(!filter.filter_key(vk_esc, true), "switch frame produces clean release");

  // Frame after switch: key is still physically down -> must be ignored (needs re-arm)
  filter.set_context(GameContext::Race); // subsequent frame in race
  require(!filter.filter_key(vk_esc, true), "held-across-switch key is blocked");

  // User releases Esc:
  require(!filter.filter_key(vk_esc, false), "released key returns false");

  // User presses Esc again:
  require(filter.filter_key(vk_esc, true), "re-armed key fires successfully");

  // Gamepad button re-arm test:
  constexpr std::uint16_t xinput_start = 0x0010;
  filter.set_context(GameContext::Race);
  filter.set_context(GameContext::Pause); // Switch to Pause
  // Exact switch frame:
  require(filter.filter_gamepad_buttons(xinput_start) == 0, "switch frame releases pad buttons");

  // Still held in Pause:
  filter.set_context(GameContext::Pause);
  require(filter.filter_gamepad_buttons(xinput_start) == 0, "still held button is masked");

  // Release:
  require(filter.filter_gamepad_buttons(0) == 0, "pad released");

  // Press again:
  require(filter.filter_gamepad_buttons(xinput_start) == xinput_start, "re-armed pad button fires");

  // Loading screen cleans pad:
  filter.set_context(GameContext::Loading);
  require(!filter.filter_key(vk_esc, true), "loading blocks key");
  require(filter.filter_gamepad_buttons(xinput_start) == 0, "loading blocks pad buttons");
}

void test_default_context_controls_mappings() {
  const auto &context_controls = gt4::controls::default_context_controls();
  require(context_controls.has_context_overrides, "default context controls has overrides");

  // In Menu: Esc is in triangle
  const auto &menu_map = context_controls.map_for(GameContext::Menu);
  bool esc_in_menu_triangle = false;
  for (const auto &b : menu_map.bindings[7]) { // 7 is triangle in inputs[]
    if (b.kind == gt4::controls::SourceKind::keyboard && b.virtual_key == 0x1B)
      esc_in_menu_triangle = true;
  }
  require(esc_in_menu_triangle, "Esc is Triangle in Menu");

  // In Race: Esc is in Start, NOT in triangle
  const auto &race_map = context_controls.map_for(GameContext::Race);
  bool esc_in_race_start = false;
  for (const auto &b : race_map.bindings[15]) { // 15 is start in inputs[]
    if (b.kind == gt4::controls::SourceKind::keyboard && b.virtual_key == 0x1B)
      esc_in_race_start = true;
  }
  require(esc_in_race_start, "Esc is Start in Race");

  bool esc_in_race_triangle = false;
  for (const auto &b : race_map.bindings[7]) {
    if (b.kind == gt4::controls::SourceKind::keyboard && b.virtual_key == 0x1B)
      esc_in_race_triangle = true;
  }
  require(!esc_in_race_triangle, "Esc is NOT Triangle in Race");

  // In Pause: Esc is in Triangle (back/resume) and Start
  const auto &pause_map = context_controls.map_for(GameContext::Pause);
  bool esc_in_pause_triangle = false;
  for (const auto &b : pause_map.bindings[7]) {
    if (b.kind == gt4::controls::SourceKind::keyboard && b.virtual_key == 0x1B)
      esc_in_pause_triangle = true;
  }
  require(esc_in_pause_triangle, "Esc is Triangle in Pause");
}

} // namespace

int main() {
  try {
    test_names_and_parsing();
    test_vtable_predicates();
    test_synthetic_ram_detection();
    test_debounce_state_machine();
    test_context_input_filter();
    test_default_context_controls_mappings();
    std::printf("context_probe_fixture: ALL TESTS PASSED\n");
    return 0;
  } catch (const std::exception &e) {
    std::fprintf(stderr, "context_probe_fixture: %s\n", e.what());
    return 1;
  }
}
