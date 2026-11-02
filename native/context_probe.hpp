#pragma once

// TASK-077: Context-aware input mapping (host-side game context probe).
//
// Detects the game's current high-level state (Menu, Race, Pause, PhotoPause, etc.)
// by reading guest RAM at VBlank using range-checked guest loads (pattern from race_display.cpp).
//
// Candidate addresses and vtables (Confirmed from reconstructed SCUS_973.28 ELF and gt4-decomp headers):
//   Owner pointer: word at 0x622F4C (read with word_in_ram check)
//   Race vtables:
//     RaceSinglePlayer  0x682A20
//     RaceArcade        0x67BFA8
//     RaceArcadeSingle  0x67BAD8
//     RaceArcadeDemo    0x67C520 (attract demo -> treated as Menu)
//     RaceGTmode        0x684678
//     RaceFreeRun       0x680140
//     RaceLicense       0x6807A8
//     RaceSolitaire     0x683420
//     RacePhotoDevelop  0x682310
//     RacePhotoMode     0x681C08
//     RacePS2Base       0x679378
//   Pause pointer inside race object:
//     race + 0xE418u (pointer to active Pause object)
//   Pause vtables:
//     RacePause         0x681B28
//     SimplePause       0x681B60
//     PhotoPause        0x681B98 (photo mode pause, inside race)

#include <cstddef>
#include <cstdint>
#include <string_view>

namespace gt4::context {

enum class GameContext : std::uint8_t {
  Unknown = 0,
  Menu,
  Race,
  Pause,
  PhotoPause,
  Replay,
  Showroom,
  Loading,
};

inline constexpr std::size_t kContextCount = 8;

const char *context_name(GameContext context);
GameContext parse_context_name(std::string_view name);

// Guest addresses
inline constexpr std::uint32_t kOwnerPointerAddress = 0x622F4Cu;
inline constexpr std::uint32_t kRacePausePointerOffset = 0xE418u;
inline constexpr std::uint32_t kPauseObjectVtableOffset = 0x8u;

// Vtable addresses
inline constexpr std::uint32_t kVtableRaceSinglePlayer = 0x682A20u;
inline constexpr std::uint32_t kVtableRaceArcade = 0x67BFA8u;
inline constexpr std::uint32_t kVtableRaceArcadeSingle = 0x67BAD8u;
inline constexpr std::uint32_t kVtableRaceArcadeDemo = 0x67C520u; // Attract demo
inline constexpr std::uint32_t kVtableRaceGTmode = 0x684678u;
inline constexpr std::uint32_t kVtableRaceFreeRun = 0x680140u;
inline constexpr std::uint32_t kVtableRaceLicense = 0x6807A8u;
inline constexpr std::uint32_t kVtableRaceSolitaire = 0x683420u;
inline constexpr std::uint32_t kVtableRacePhotoDevelop = 0x682310u;
inline constexpr std::uint32_t kVtableRacePhotoMode = 0x681C08u;
inline constexpr std::uint32_t kVtableRacePS2Base = 0x679378u;

inline constexpr std::uint32_t kVtableRacePause = 0x681B28u;
inline constexpr std::uint32_t kVtableSimplePause = 0x681B60u;
inline constexpr std::uint32_t kVtablePhotoPause = 0x681B98u;

// Pure recognition predicates
bool is_race_vtable(std::uint32_t vptr);
bool is_attract_demo_vtable(std::uint32_t vptr);
bool is_pause_vtable(std::uint32_t vptr);
bool is_photo_pause_vtable(std::uint32_t vptr);

// Bounds and safety
bool word_in_ram(std::uint64_t address, std::uint32_t ram_size);
std::uint32_t read_le32(const std::uint8_t *bytes);

// Finds the race object address in RAM by checking a cached offset first, then performing
// a bounded scan of the owner structure. Returns guest address of race object, or 0 if not found.
std::uint32_t find_race_object(const std::uint8_t *ram, std::uint32_t ram_size,
                              std::uint32_t owner_pointer, std::uint32_t &cached_offset);

// Instantaneous raw context detection from guest RAM (pure).
GameContext detect_context_raw(const std::uint8_t *ram, std::uint32_t ram_size,
                              std::uint32_t &cached_race_offset);

struct ContextProbeState {
  GameContext raw_context = GameContext::Unknown;
  GameContext active_context = GameContext::Menu;
  std::uint32_t stable_count = 0;
  std::uint32_t cached_race_offset = 0;
  std::uint32_t debounce_threshold = 3; // 3 consecutive VBlanks required to switch
  std::uint64_t transitions = 0;
  bool logging_enabled = false;
};

// Advances the probe by one VBlank sample with debounce filtering.
// Transitions are logged only when `logging_enabled` is true.
GameContext update_context_at_vblank(const std::uint8_t *ram, std::uint32_t ram_size,
                                    ContextProbeState &state);

} // namespace gt4::context
