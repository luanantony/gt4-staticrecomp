#pragma once
// Guest RAM discovery and VBlank injection for the TASK-215 H-shifter and clutch mod.
// Specification: docs/H-SHIFTER-CLUTCH-MOD-STUDY.md sections 2, 3 and 5.3.

#include <cstddef>
#include <cstdint>
#include "hshifter_mod.hpp"

namespace gt4::hshifter {

// Discovered live player car pointers and physical state.
struct GuestPlayerCar {
  std::uint32_t automobile_address = 0;
  bool is_manual_transmission = false;
  std::uint8_t forward_gears = 6;
  std::uint16_t max_rpm = 8000;
  float speed_mps = 0.0f;
  float engine_rpm = 0.0f;
  std::uint8_t current_gear = 0;
  std::uint8_t current_clutch_state = 0;
};

// Host parameter block base address in guest RAM (the TASK-080 parameter block space).
inline constexpr std::uint32_t kHostBlockAddress = 0x7F00;
inline constexpr std::uint32_t kHostBlockClutchState = 0x7F10; // u8
inline constexpr std::uint32_t kHostBlockHudClutch   = 0x7F12; // u8
inline constexpr std::uint32_t kHostBlockTargetGear  = 0x7F14; // s32
inline constexpr std::uint32_t kHostBlockHudChar     = 0x7F16; // u8 ('R' or 'N')

// Offsets inside Automobile object (from H-SHIFTER-CLUTCH-MOD-STUDY.md section 2.1).
inline constexpr std::uint32_t kAutoEnginePointerOffset = 0x010; // Engine*
inline constexpr std::uint32_t kAutoControlTypeOffset   = 0x546; // u8, 6 = manual
inline constexpr std::uint32_t kAutoReverseReqOffset    = 0x566; // s8
inline constexpr std::uint32_t kAutoShiftStepOffset     = 0x567; // s8
inline constexpr std::uint32_t kAutoEngineRpmOffset     = 0x600; // f32
inline constexpr std::uint32_t kAutoClutchEngageOffset  = 0x618; // f32
inline constexpr std::uint32_t kAutoGearOffset          = 0x624; // u8 (0 = R, 1..count)
inline constexpr std::uint32_t kAutoClutchStateOffset   = 0x626; // u8 (0 open, 1 engaged, 3 sync)
inline constexpr std::uint32_t kAutoShiftDelayOffset    = 0x62A; // s8
inline constexpr std::uint32_t kAutoSpeedMpsOffset      = 0x640; // f32

// Offsets inside Engine object.
inline constexpr std::uint32_t kEngineMaxRpmOffset     = 0x200; // u16
inline constexpr std::uint32_t kEngineGearCountOffset  = 0x236; // u8

// Records the active RaceDisplay guest address (passed from on_race_display_render hook).
void note_race_display(std::uint32_t race_display_address);
std::uint32_t active_race_display();

// Reads player car state from guest RAM. Returns true when player Automobile is found. Pure.
bool read_guest_player_car(const std::uint8_t *ram, std::size_t ram_size, GuestPlayerCar &out);

// Injects the H-shifter outputs into guest RAM at VBlank (Route A writes + 0x7F10 parameter block). Pure.
bool apply_hshifter_to_guest(std::uint8_t *ram, std::size_t ram_size,
                             const GuestPlayerCar &car, const ModOutput &output);

} // namespace gt4::hshifter
