// TASK-163: Unit test fixture for bonnet camera wind volume (native/bonnet_wind.hpp).
#include "bonnet_wind.hpp"

#include <cmath>
#include <cstdio>
#include <cstring>
#include <stdexcept>
#include <vector>

namespace {

int failures = 0;

void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "bonnet_wind_fixture: FAILED %s\n", what);
    ++failures;
  }
}

bool near_float(float a, float b, float epsilon = 0.001f) {
  return std::abs(a - b) <= epsilon;
}

void test_parse_volume() {
  using namespace gt4::bonnet_wind;

  // Defaults and 100%
  check(near_float(parse_volume(nullptr), 1.0f), "nullptr is 1.0");
  check(near_float(parse_volume(""), 1.0f), "empty is 1.0");
  check(near_float(parse_volume("default"), 1.0f), "'default' is 1.0");
  check(near_float(parse_volume("100"), 1.0f), "'100' is 1.0");
  check(near_float(parse_volume("100%"), 1.0f), "'100%' is 1.0");
  check(near_float(parse_volume("1.0"), 1.0f), "'1.0' is 1.0");
  check(near_float(parse_volume("1"), 1.0f), "'1' is 1.0");

  // Muted and 0%
  check(near_float(parse_volume("0"), 0.0f), "'0' is 0.0");
  check(near_float(parse_volume("0%"), 0.0f), "'0%' is 0.0");
  check(near_float(parse_volume("0.0"), 0.0f), "'0.0' is 0.0");
  check(near_float(parse_volume("off"), 0.0f), "'off' is 0.0");
  check(near_float(parse_volume("muted"), 0.0f), "'muted' is 0.0");

  // Intermediate values
  check(near_float(parse_volume("50"), 0.5f), "'50' is 0.5");
  check(near_float(parse_volume("50%"), 0.5f), "'50%' is 0.5");
  check(near_float(parse_volume("0.5"), 0.5f), "'0.5' is 0.5");
  check(near_float(parse_volume("25"), 0.25f), "'25' is 0.25");
  check(near_float(parse_volume("75%"), 0.75f), "'75%' is 0.75");
  check(near_float(parse_volume("0.1"), 0.1f), "'0.1' is 0.1");

  // Invalid values throw std::invalid_argument
  auto refused = [](const char *str) {
    try {
      parse_volume(str);
      return false;
    } catch (const std::invalid_argument &) {
      return true;
    }
  };

  check(refused("invalid"), "'invalid' is refused");
  check(refused("-1"), "'-1' is refused");
  check(refused("150"), "'150' is refused");
  check(refused("50foo"), "'50foo' is refused");
  check(refused("abc%"), "'abc%' is refused");
}

void test_apply_vblank() {
  using namespace gt4::bonnet_wind;

  std::vector<std::uint8_t> ram(0x700000, 0);
  const Addresses addr = {
      /* visual */ 0x006213E8u,
      /* visual_volume */ 0x006213DCu,
      /* external_volume */ 0x006213E0u,
      /* dive */ 0x006213ECu,
  };

  AttenuationState state;

  // Initial guest state: bonnet camera (1), original volume = 1.0f
  const std::uint32_t visual_bonnet = 1;
  const std::uint32_t visual_chase = 2;
  const std::uint32_t visual_bumper = 0;

  float guest_vol = 1.0f;
  std::memcpy(ram.data() + addr.visual, &visual_bonnet, sizeof(std::uint32_t));
  std::memcpy(ram.data() + addr.visual_volume, &guest_vol, sizeof(float));

  // Case 1: Stock volume 1.0f (no attenuation)
  bool wrote = apply_vblank(ram.data(), static_cast<std::uint32_t>(ram.size()), addr, 1.0f, state);
  check(!wrote, "factor 1.0 does not write to ram");
  float read_vol = 0.0f;
  std::memcpy(&read_vol, ram.data() + addr.visual_volume, sizeof(float));
  check(near_float(read_vol, 1.0f), "ram volume remains 1.0f");

  // Case 2: Muted (factor 0.0f) in bonnet camera
  wrote = apply_vblank(ram.data(), static_cast<std::uint32_t>(ram.size()), addr, 0.0f, state);
  check(wrote, "factor 0.0 writes muted volume to ram");
  std::memcpy(&read_vol, ram.data() + addr.visual_volume, sizeof(float));
  check(near_float(read_vol, 0.0f), "ram volume is now 0.0f");
  check(state.active, "state is active");
  check(near_float(state.original_volume, 1.0f), "state preserved original volume 1.0f");

  // Frame 2: no changes, should not write again
  wrote = apply_vblank(ram.data(), static_cast<std::uint32_t>(ram.size()), addr, 0.0f, state);
  check(!wrote, "subsequent frame does not rewrite unchanged volume");

  // Case 3: Switch to chase camera (visual 2) -> restores original volume
  std::memcpy(ram.data() + addr.visual, &visual_chase, sizeof(std::uint32_t));
  wrote = apply_vblank(ram.data(), static_cast<std::uint32_t>(ram.size()), addr, 0.0f, state);
  check(wrote, "exiting bonnet camera restores volume");
  std::memcpy(&read_vol, ram.data() + addr.visual_volume, sizeof(float));
  check(near_float(read_vol, 1.0f), "ram volume restored to 1.0f");
  check(!state.active, "state is no longer active");

  // Case 4: 50% attenuation in bonnet camera
  std::memcpy(ram.data() + addr.visual, &visual_bonnet, sizeof(std::uint32_t));
  wrote = apply_vblank(ram.data(), static_cast<std::uint32_t>(ram.size()), addr, 0.5f, state);
  check(wrote, "factor 0.5 writes attenuated volume to ram");
  std::memcpy(&read_vol, ram.data() + addr.visual_volume, sizeof(float));
  check(near_float(read_vol, 0.5f), "ram volume is 0.5f");

  // Case 5: Guest re-evaluates volume to 0.8f while in bonnet cam
  guest_vol = 0.8f;
  std::memcpy(ram.data() + addr.visual_volume, &guest_vol, sizeof(float));
  wrote = apply_vblank(ram.data(), static_cast<std::uint32_t>(ram.size()), addr, 0.5f, state);
  check(wrote, "guest volume update is re-attenuated");
  std::memcpy(&read_vol, ram.data() + addr.visual_volume, sizeof(float));
  check(near_float(read_vol, 0.4f), "ram volume is 0.8 * 0.5 = 0.4f");
}

}  // namespace

int main() {
  test_parse_volume();
  test_apply_vblank();

  if (failures != 0) {
    std::fprintf(stderr, "bonnet_wind_fixture: %d failure(s)\n", failures);
    return 1;
  }
  std::printf("bonnet_wind_fixture: ok\n");
  return 0;
}
