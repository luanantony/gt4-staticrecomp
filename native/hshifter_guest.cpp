#include "hshifter_guest.hpp"

#include <atomic>
#include <cstring>

namespace gt4::hshifter {

namespace {

static std::atomic<std::uint32_t> g_active_race_display{0};

template <typename T>
inline bool can_read(std::size_t offset, std::size_t ram_size) {
  return offset + sizeof(T) <= ram_size;
}

template <typename T>
inline T read_val(const std::uint8_t *ram, std::size_t offset) {
  T val{};
  std::memcpy(&val, ram + offset, sizeof(T));
  return val;
}

template <typename T>
inline void write_val(std::uint8_t *ram, std::size_t offset, T val) {
  std::memcpy(ram + offset, &val, sizeof(T));
}

} // namespace

void note_race_display(std::uint32_t race_display_address) {
  g_active_race_display.store(race_display_address, std::memory_order_relaxed);
}

std::uint32_t active_race_display() {
  return g_active_race_display.load(std::memory_order_relaxed);
}

bool read_guest_player_car(const std::uint8_t *ram, std::size_t ram_size, GuestPlayerCar &out) {
  if (ram == nullptr || ram_size < 0x2000000) { // requires 32 MiB guest RAM
    return false;
  }

  // Player Automobile discovery:
  // If no car address is cached, resolve via RaceDisplay object chain (docs/H-SHIFTER-CLUTCH-MOD-STUDY.md sec 2.2).
  if (out.automobile_address == 0) {
    const std::uint32_t race_disp = active_race_display();
    if (race_disp != 0 && can_read<std::uint32_t>(race_disp + 0x30, ram_size)) {
      const auto org_ptr = read_val<std::uint32_t>(ram, race_disp + 0x04);
      const auto player_idx = read_val<std::uint8_t>(ram, race_disp + 0x2E);
      if (org_ptr >= 0x100000 && can_read<std::uint32_t>(org_ptr + 0x68, ram_size)) {
        const auto cars_count = read_val<std::int32_t>(ram, org_ptr + 0x60);
        const auto cars_array = read_val<std::uint32_t>(ram, org_ptr + 0x68);
        if (cars_count > 0 && player_idx < cars_count && cars_array >= 0x100000 &&
            can_read<std::uint32_t>(cars_array + player_idx * sizeof(std::uint32_t), ram_size)) {
          const auto car_record = read_val<std::uint32_t>(ram, cars_array + player_idx * sizeof(std::uint32_t));
          if (car_record >= 0x100000 && can_read<std::uint32_t>(car_record + 0x18, ram_size)) {
            const auto auto_ptr = read_val<std::uint32_t>(ram, car_record + 0x18);
            if (auto_ptr >= 0x100000 && can_read<float>(auto_ptr + kAutoSpeedMpsOffset, ram_size)) {
              out.automobile_address = auto_ptr;
            }
          }
        }
      }
    }
  }

  // If a valid automobile address is known, read and refresh its physical state.
  if (out.automobile_address != 0) {
    const auto auto_addr = out.automobile_address;
    if (!can_read<float>(auto_addr + kAutoSpeedMpsOffset, ram_size)) {
      out.automobile_address = 0;
      return false;
    }

    const auto ctrl_type = read_val<std::uint8_t>(ram, auto_addr + kAutoControlTypeOffset);
    out.is_manual_transmission = (ctrl_type == 6);
    out.speed_mps = read_val<float>(ram, auto_addr + kAutoSpeedMpsOffset);
    out.engine_rpm = read_val<float>(ram, auto_addr + kAutoEngineRpmOffset);
    out.current_gear = read_val<std::uint8_t>(ram, auto_addr + kAutoGearOffset);
    out.current_clutch_state = read_val<std::uint8_t>(ram, auto_addr + kAutoClutchStateOffset);

    // Read transmission specs from Engine block if pointer is valid.
    const auto eng_ptr = read_val<std::uint32_t>(ram, auto_addr + kAutoEnginePointerOffset);
    if (eng_ptr != 0 && can_read<std::uint8_t>(eng_ptr + kEngineGearCountOffset, ram_size)) {
      out.max_rpm = read_val<std::uint16_t>(ram, eng_ptr + kEngineMaxRpmOffset);
      const auto gears = read_val<std::uint8_t>(ram, eng_ptr + kEngineGearCountOffset);
      if (gears >= 1 && gears <= 8) {
        out.forward_gears = gears;
      }
    }
    return true;
  }

  return false;
}

bool apply_hshifter_to_guest(std::uint8_t *ram, std::size_t ram_size,
                             const GuestPlayerCar &car, const ModOutput &output) {
  if (ram == nullptr || ram_size < 0x2000000) {
    return false;
  }

  // 1. Always update the host parameter block (0x7F10..0x7F17) for derivative ELF / patch hooks.
  if (kHostBlockAddress + 0x20 <= ram_size) {
    write_val<std::uint8_t>(ram, kHostBlockClutchState, static_cast<std::uint8_t>(output.clutch_state));
    write_val<std::uint8_t>(ram, kHostBlockHudClutch, output.clutch_state == ClutchState::engaged ? 1u : 0u);
    write_val<std::int32_t>(ram, kHostBlockTargetGear, output.target_gear);
    write_val<std::uint8_t>(ram, kHostBlockHudChar, static_cast<std::uint8_t>(output.hud_glyph));
  }

  // 2. Direct VBlank write to player Automobile fields (Route A).
  if (car.automobile_address != 0 && car.is_manual_transmission) {
    const auto auto_addr = car.automobile_address;
    if (!can_read<float>(auto_addr + kAutoSpeedMpsOffset, ram_size)) {
      return false;
    }

    write_val<std::uint8_t>(ram, auto_addr + kAutoGearOffset, static_cast<std::uint8_t>(output.target_gear));
    write_val<std::uint8_t>(ram, auto_addr + kAutoClutchStateOffset, static_cast<std::uint8_t>(output.clutch_state));
    write_val<float>(ram, auto_addr + kAutoClutchEngageOffset, output.clutch_engagement);

    if (output.suppress_guest_shifts) {
      write_val<std::int8_t>(ram, auto_addr + kAutoShiftStepOffset, 0);
      write_val<std::int8_t>(ram, auto_addr + kAutoReverseReqOffset, 0);
      write_val<std::int8_t>(ram, auto_addr + kAutoShiftDelayOffset, 0);
    }
  }

  return true;
}

} // namespace gt4::hshifter
