// CTest native_hshifter_mod: tests for the TASK-215 H-shifter and clutch gameplay mod logic and guest memory application.
#include "hshifter_mod.hpp"
#include "hshifter_guest.hpp"

#include <cmath>
#include <cstdio>
#include <cstring>
#include <stdexcept>
#include <string_view>
#include <vector>

namespace {

using namespace gt4::hshifter;

int failures = 0;

void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "hshifter_mod_fixture: FAILED %s\n", what);
    ++failures;
  }
}

void check_switch() {
  check(!parse_switch(nullptr), "switch: nullptr is off (default)");
  check(!parse_switch(""), "switch: empty string is off");
  check(!parse_switch("0"), "switch: '0' is off");
  check(parse_switch("1"), "switch: '1' is on");

  for (const char *bad_val : {"2", "true", "on", "yes", "-1", " 1", "1 "}) {
    bool threw = false;
    try {
      parse_switch(bad_val);
    } catch (const std::invalid_argument &) {
      threw = true;
    }
    check(threw, "switch: invalid value throws std::invalid_argument");
  }

  check(kSwitch == "GT4_MOD_HSHIFTER", "kSwitch name");
  check(kIniKey == "mod_hshifter", "kIniKey name");
}

void check_initial_state() {
  HShifterStateMachine sm;
  check(sm.active_gate() == Gate::neutral, "initial active gate is neutral");
  check(sm.current_clutch_state() == ClutchState::open, "initial clutch state is open");

  DrivetrainInput input;
  input.gate = Gate::neutral;
  input.pedal_position = 0.0f;
  input.speed_mps = 0.0f;
  input.is_manual_transmission = true;

  auto out = sm.update(input, 16);
  check(out.target_gear == 0, "initial target gear is 0 (neutral)");
  check(out.clutch_state == ClutchState::open, "initial output clutch is open");
  check(out.clutch_engagement == 0.0f, "initial engagement is 0.0f in neutral");
  check(out.hud_glyph == 'N', "initial HUD glyph is 'N'");
  check(out.rejection == ShiftRejection::none, "initial rejection is none");
}

void check_standard_shift_flow() {
  HShifterStateMachine sm;
  DrivetrainInput input;
  input.gate = Gate::neutral;
  input.pedal_position = 0.0f;
  input.forward_gears = 6;
  input.is_manual_transmission = true;

  // Press clutch down to 1.0f and select 1st gear.
  input.pedal_position = 1.0f;
  input.gate = Gate::gear_1;

  // 16 ms elapsed: debounce window (50 ms) not yet passed.
  auto out = sm.update(input, 16);
  check(sm.active_gate() == Gate::neutral, "gate does not change before debounce elapsed");

  // Advance by another 40 ms (total 56 ms > 50 ms debounce).
  out = sm.update(input, 40);
  check(sm.active_gate() == Gate::gear_1, "gate transitions to gear_1 after debounce");
  check(out.target_gear == 1, "target gear is 1");
  check(out.clutch_state == ClutchState::open, "clutch remains open while pedal is pressed");
  check(out.hud_glyph == '\0', "hud glyph is null for forward gear");

  // Driver releases pedal to 0.0f: should transition through syncing, then engaged.
  input.pedal_position = 0.0f;
  out = sm.update(input, 16);
  check(out.clutch_state == ClutchState::syncing, "clutch enters syncing state on pedal release");

  // Another 30 ms (total 46 ms > 35 ms sync window): should become fully engaged.
  out = sm.update(input, 30);
  check(out.clutch_state == ClutchState::engaged, "clutch is fully engaged");
  check(out.clutch_engagement == 1.0f, "clutch engagement factor is 1.0f when engaged");
}

void check_direct_skip_shifting() {
  HShifterStateMachine sm;
  DrivetrainInput input;
  input.gate = Gate::gear_2;
  input.pedal_position = 1.0f;
  input.forward_gears = 6;
  input.is_manual_transmission = true;

  // Establish active gear 2
  sm.update(input, 60);
  input.pedal_position = 0.0f;
  sm.update(input, 50);
  check(sm.active_gate() == Gate::gear_2, "car cruising in gear 2");

  // Skip shift directly from 2nd to 5th gear!
  input.pedal_position = 1.0f;
  input.gate = Gate::gear_5;
  auto out = sm.update(input, 60);

  check(sm.active_gate() == Gate::gear_5, "direct skip-shift to gear 5 accepted immediately without stepping");
  check(out.target_gear == 5, "target gear is directly 5");
  check(out.suppress_guest_shifts, "guest sequential shifts suppressed");
}

void check_shift_without_clutch() {
  HShifterStateMachine sm;
  DrivetrainInput input;
  input.gate = Gate::neutral;
  input.pedal_position = 0.0f; // Foot off clutch
  input.forward_gears = 6;
  input.is_manual_transmission = true;

  // Attempt to slam into 1st gear without depressing clutch
  input.gate = Gate::gear_1;
  auto out = sm.update(input, 60);

  check(out.rejection == ShiftRejection::clutch_not_depressed, "shift rejected when clutch not depressed");
  check(sm.active_gate() == Gate::neutral, "active gate remains neutral on rejection");
  check(out.target_gear == 0, "target gear remains 0");

  // But with auto-clutch assist enabled in config, it should succeed
  ModConfig cfg;
  cfg.auto_clutch_assist = true;
  HShifterStateMachine assisted_sm(cfg);

  out = assisted_sm.update(input, 60);
  check(out.rejection == ShiftRejection::none, "shift accepted with auto_clutch_assist enabled");
  check(assisted_sm.active_gate() == Gate::gear_1, "active gate transitions to gear_1 with assist");
}

void check_reverse_safety_lock() {
  HShifterStateMachine sm;
  DrivetrainInput input;
  input.pedal_position = 1.0f;
  input.forward_gears = 6;
  input.is_manual_transmission = true;

  // Moving forward at 20 m/s (~72 km/h)
  input.speed_mps = 20.0f;
  input.gate = Gate::reverse;

  auto out = sm.update(input, 60);
  check(out.rejection == ShiftRejection::speed_too_high_for_reverse, "reverse rejected at speed");
  check(sm.active_gate() == Gate::neutral, "active gate forced to neutral");
  check(out.target_gear == 0, "target gear is 0");
  check(out.hud_glyph == 'N', "HUD displays 'N' when reverse is rejected at speed");

  // Come to a near complete stop (< 0.2778 m/s = 1 km/h)
  input.speed_mps = 0.10f;
  out = sm.update(input, 60);
  check(out.rejection == ShiftRejection::none, "reverse accepted when vehicle is stopped");
  check(sm.active_gate() == Gate::reverse, "active gate becomes reverse");
  check(out.hud_glyph == 'R', "HUD displays 'R' when reverse is engaged");
}

void check_gear_count_clamp() {
  HShifterStateMachine sm;
  DrivetrainInput input;
  input.pedal_position = 1.0f;
  input.forward_gears = 5; // 5-speed vehicle
  input.is_manual_transmission = true;

  // Attempt to select 6th gear on 5-speed vehicle
  input.gate = Gate::gear_6;
  auto out = sm.update(input, 60);
  check(out.rejection == ShiftRejection::gear_exceeds_car_max, "6th gear rejected on 5-speed vehicle");
  check(sm.active_gate() == Gate::neutral, "gate held in neutral on exceeding gear count");

  // Select 5th gear on 5-speed vehicle
  input.gate = Gate::gear_5;
  out = sm.update(input, 60);
  check(out.rejection == ShiftRejection::none, "5th gear accepted on 5-speed vehicle");
  check(sm.active_gate() == Gate::gear_5, "active gate becomes gear_5");
  check(out.target_gear == 5, "target gear is 5");
}

void check_automatic_transmission_refusal() {
  HShifterStateMachine sm;
  DrivetrainInput input;
  input.gate = Gate::gear_3;
  input.pedal_position = 1.0f;
  input.forward_gears = 6;
  input.is_manual_transmission = false; // AT car

  auto out = sm.update(input, 60);
  check(out.rejection == ShiftRejection::automatic_transmission_blocked, "AT vehicle blocks mod intervention");
  check(!out.suppress_guest_shifts, "does not suppress guest shifts for AT car");
}

void check_guest_memory_interaction() {
  constexpr std::size_t kRamSize = 32 * 1024 * 1024;
  std::vector<std::uint8_t> ram(kRamSize, 0);

  constexpr std::uint32_t kAutoAddr = 0x01200000;
  constexpr std::uint32_t kEngAddr  = 0x01250000;

  // Setup fake Engine block: max_rpm = 9200, forward_gears = 6
  std::uint16_t max_rpm = 9200;
  std::uint8_t forward_gears = 6;
  std::memcpy(ram.data() + kEngAddr + kEngineMaxRpmOffset, &max_rpm, sizeof(max_rpm));
  std::memcpy(ram.data() + kEngAddr + kEngineGearCountOffset, &forward_gears, sizeof(forward_gears));

  // Setup fake Automobile: control_type = 6 (MT), engine_ptr = kEngAddr, speed = 25.5f, rpm = 4500.0f
  std::uint8_t ctrl_type = 6;
  float speed = 25.5f;
  float rpm = 4500.0f;
  std::memcpy(ram.data() + kAutoAddr + kAutoEnginePointerOffset, &kEngAddr, sizeof(kEngAddr));
  std::memcpy(ram.data() + kAutoAddr + kAutoControlTypeOffset, &ctrl_type, sizeof(ctrl_type));
  std::memcpy(ram.data() + kAutoAddr + kAutoSpeedMpsOffset, &speed, sizeof(speed));
  std::memcpy(ram.data() + kAutoAddr + kAutoEngineRpmOffset, &rpm, sizeof(rpm));

  GuestPlayerCar car;
  car.automobile_address = kAutoAddr;

  check(read_guest_player_car(ram.data(), kRamSize, car), "read_guest_player_car succeeded");
  check(car.is_manual_transmission, "detected manual transmission");
  check(car.max_rpm == 9200, "read correct max_rpm");
  check(car.forward_gears == 6, "read correct forward_gears");
  check(car.speed_mps == 25.5f, "read correct speed_mps");
  check(car.engine_rpm == 4500.0f, "read correct engine_rpm");

  // Apply a ModOutput (direct gear 4, clutch engaged, suppress shifts)
  ModOutput out;
  out.target_gear = 4;
  out.clutch_state = ClutchState::engaged;
  out.clutch_engagement = 1.0f;
  out.hud_glyph = '\0';
  out.suppress_guest_shifts = true;

  check(apply_hshifter_to_guest(ram.data(), kRamSize, car, out), "apply_hshifter_to_guest succeeded");

  // Verify guest Automobile fields
  std::uint8_t written_gear = ram[kAutoAddr + kAutoGearOffset];
  std::uint8_t written_clutch = ram[kAutoAddr + kAutoClutchStateOffset];
  float written_engage = 0.0f;
  std::memcpy(&written_engage, ram.data() + kAutoAddr + kAutoClutchEngageOffset, sizeof(float));
  std::int8_t written_step = static_cast<std::int8_t>(ram[kAutoAddr + kAutoShiftStepOffset]);

  check(written_gear == 4, "guest Automobile gear written to 4");
  check(written_clutch == 1, "guest Automobile clutch state written to 1 (engaged)");
  check(written_engage == 1.0f, "guest Automobile clutch engagement written to 1.0f");
  check(written_step == 0, "guest Automobile shift step cleared to 0");

  // Verify host parameter block (0x7F10..0x7F16)
  std::uint8_t block_clutch = ram[kHostBlockClutchState];
  std::int32_t block_gear = 0;
  std::memcpy(&block_gear, ram.data() + kHostBlockTargetGear, sizeof(block_gear));

  check(block_clutch == 1, "host block clutch state is 1");
  check(block_gear == 4, "host block target gear is 4");
}

void check_guest_discovery_chain() {
  constexpr std::size_t kRamSize = 32 * 1024 * 1024;
  std::vector<std::uint8_t> ram(kRamSize, 0);

  constexpr std::uint32_t kRaceDisplayAddr = 0x01A00000;
  constexpr std::uint32_t kOrgAddr         = 0x01B00000;
  constexpr std::uint32_t kCarsArrayAddr   = 0x01C00000;
  constexpr std::uint32_t kCarRecordAddr   = 0x01D00000;
  constexpr std::uint32_t kAutoAddr        = 0x01E00000;
  constexpr std::uint32_t kEngAddr         = 0x01F00000;

  // Setup RaceDisplay:
  // +0x04 -> RaceOrganization pointer
  // +0x2E -> player car index (0)
  std::uint32_t org_ptr = kOrgAddr;
  std::uint8_t player_idx = 0;
  std::memcpy(ram.data() + kRaceDisplayAddr + 0x04, &org_ptr, sizeof(org_ptr));
  ram[kRaceDisplayAddr + 0x2E] = player_idx;

  // Setup RaceOrganization:
  // +0x60 -> count (1)
  // +0x68 -> cars array pointer
  std::int32_t cars_count = 1;
  std::uint32_t cars_array_ptr = kCarsArrayAddr;
  std::memcpy(ram.data() + kOrgAddr + 0x60, &cars_count, sizeof(cars_count));
  std::memcpy(ram.data() + kOrgAddr + 0x68, &cars_array_ptr, sizeof(cars_array_ptr));

  // Setup Cars array item 0: pointer to CarRecord
  std::uint32_t car_rec_ptr = kCarRecordAddr;
  std::memcpy(ram.data() + kCarsArrayAddr, &car_rec_ptr, sizeof(car_rec_ptr));

  // Setup CarRecord:
  // +0x18 -> Automobile pointer
  std::uint32_t auto_ptr = kAutoAddr;
  std::memcpy(ram.data() + kCarRecordAddr + 0x18, &auto_ptr, sizeof(auto_ptr));

  // Setup Automobile:
  // +0x010 -> Engine pointer
  // +0x546 -> control_type = 6 (manual)
  // +0x600 -> rpm = 3500.0f
  // +0x640 -> speed = 15.0f
  std::uint32_t eng_ptr = kEngAddr;
  std::uint8_t ctrl_type = 6;
  float rpm = 3500.0f;
  float speed = 15.0f;
  std::memcpy(ram.data() + kAutoAddr + kAutoEnginePointerOffset, &eng_ptr, sizeof(eng_ptr));
  ram[kAutoAddr + kAutoControlTypeOffset] = ctrl_type;
  std::memcpy(ram.data() + kAutoAddr + kAutoEngineRpmOffset, &rpm, sizeof(rpm));
  std::memcpy(ram.data() + kAutoAddr + kAutoSpeedMpsOffset, &speed, sizeof(speed));

  // Setup Engine:
  // +0x200 -> max_rpm = 8200
  // +0x236 -> forward_gears = 5
  std::uint16_t max_rpm = 8200;
  std::uint8_t forward_gears = 5;
  std::memcpy(ram.data() + kEngAddr + kEngineMaxRpmOffset, &max_rpm, sizeof(max_rpm));
  ram[kEngAddr + kEngineGearCountOffset] = forward_gears;

  // Record race display
  note_race_display(kRaceDisplayAddr);
  check(active_race_display() == kRaceDisplayAddr, "active_race_display matches recorded address");

  // Read guest player car with address initially unknown (0)
  GuestPlayerCar car{};
  check(read_guest_player_car(ram.data(), kRamSize, car), "read_guest_player_car resolved through chain");
  check(car.automobile_address == kAutoAddr, "discovered correct player Automobile address");
  check(car.is_manual_transmission, "discovered manual transmission");
  check(car.forward_gears == 5, "discovered 5 forward gears");
  check(car.max_rpm == 8200, "discovered 8200 max rpm");
  check(car.speed_mps == 15.0f, "discovered 15 m/s speed");
  check(car.engine_rpm == 3500.0f, "discovered 3500 rpm");
}

} // namespace

int main() {
  check_switch();
  check_initial_state();
  check_standard_shift_flow();
  check_direct_skip_shifting();
  check_shift_without_clutch();
  check_reverse_safety_lock();
  check_gear_count_clamp();
  check_automatic_transmission_refusal();
  check_guest_memory_interaction();
  check_guest_discovery_chain();

  if (failures != 0) {
    std::fprintf(stderr, "hshifter_mod_fixture: %d test(s) failed\n", failures);
    return 1;
  }
  std::printf("hshifter_mod_fixture: all tests passed\n");
  return 0;
}
