#include "context_probe.hpp"

#include <cstdio>
#include <cstring>

namespace gt4::context {

const char *context_name(GameContext context) {
  switch (context) {
  case GameContext::Menu:
    return "menu";
  case GameContext::Race:
    return "race";
  case GameContext::Pause:
    return "pause";
  case GameContext::PhotoPause:
    return "photo_pause";
  case GameContext::Replay:
    return "replay";
  case GameContext::Showroom:
    return "showroom";
  case GameContext::Loading:
    return "loading";
  case GameContext::Unknown:
  default:
    return "unknown";
  }
}

GameContext parse_context_name(std::string_view name) {
  if (name == "menu")
    return GameContext::Menu;
  if (name == "race")
    return GameContext::Race;
  if (name == "pause")
    return GameContext::Pause;
  if (name == "photo_pause" || name == "photo")
    return GameContext::PhotoPause;
  if (name == "replay")
    return GameContext::Replay;
  if (name == "showroom")
    return GameContext::Showroom;
  if (name == "loading")
    return GameContext::Loading;
  return GameContext::Unknown;
}

bool word_in_ram(std::uint64_t address, std::uint32_t ram_size) {
  return address + 4 <= ram_size;
}

std::uint32_t read_le32(const std::uint8_t *bytes) {
  return static_cast<std::uint32_t>(bytes[0]) |
         (static_cast<std::uint32_t>(bytes[1]) << 8) |
         (static_cast<std::uint32_t>(bytes[2]) << 16) |
         (static_cast<std::uint32_t>(bytes[3]) << 24);
}

bool is_race_vtable(std::uint32_t vptr) {
  switch (vptr) {
  case kVtableRaceSinglePlayer:
  case kVtableRaceArcade:
  case kVtableRaceArcadeSingle:
  case kVtableRaceGTmode:
  case kVtableRaceFreeRun:
  case kVtableRaceLicense:
  case kVtableRaceSolitaire:
  case kVtableRacePhotoDevelop:
  case kVtableRacePhotoMode:
  case kVtableRacePS2Base:
    return true;
  default:
    return false;
  }
}

bool is_attract_demo_vtable(std::uint32_t vptr) {
  return vptr == kVtableRaceArcadeDemo;
}

bool is_pause_vtable(std::uint32_t vptr) {
  return vptr == kVtableRacePause || vptr == kVtableSimplePause;
}

bool is_photo_pause_vtable(std::uint32_t vptr) {
  return vptr == kVtablePhotoPause;
}

std::uint32_t find_race_object(const std::uint8_t *ram, std::uint32_t ram_size,
                              std::uint32_t owner_pointer, std::uint32_t &cached_offset) {
  if (owner_pointer == 0 || !word_in_ram(owner_pointer, ram_size))
    return 0;

  // 1. Fast check: see if race object is at the previously cached offset.
  if (cached_offset != 0) {
    const std::uint64_t candidate_slot = static_cast<std::uint64_t>(owner_pointer) + cached_offset;
    if (word_in_ram(candidate_slot, ram_size)) {
      const std::uint32_t obj_addr = read_le32(ram + candidate_slot);
      if (obj_addr != 0 && word_in_ram(obj_addr, ram_size)) {
        const std::uint32_t vptr = read_le32(ram + obj_addr);
        if (is_race_vtable(vptr) || is_attract_demo_vtable(vptr))
          return obj_addr;
      }
    }
    // Cached offset no longer points to a valid race object (e.g. race ended).
    cached_offset = 0;
  }

  // 2. Direct check: the owner object itself may be the race object or have a vtable.
  const std::uint32_t owner_vptr = read_le32(ram + owner_pointer);
  if (is_race_vtable(owner_vptr) || is_attract_demo_vtable(owner_vptr))
    return owner_pointer;

  // 3. Bounded scan of owner fields: search up to 2048 words (8 KiB) for valid pointers to race objects.
  constexpr std::uint32_t kMaxScanWords = 2048;
  for (std::uint32_t word_idx = 0; word_idx < kMaxScanWords; ++word_idx) {
    const std::uint64_t slot_addr = static_cast<std::uint64_t>(owner_pointer) + (word_idx * 4);
    if (!word_in_ram(slot_addr, ram_size))
      break;

    const std::uint32_t candidate_ptr = read_le32(ram + slot_addr);
    // Sanity check: must be a pointer inside guest user RAM (> 0x100000)
    if (candidate_ptr >= 0x00100000u && word_in_ram(candidate_ptr, ram_size)) {
      const std::uint32_t vptr = read_le32(ram + candidate_ptr);
      if (is_race_vtable(vptr) || is_attract_demo_vtable(vptr)) {
        cached_offset = word_idx * 4;
        return candidate_ptr;
      }
    }
  }

  return 0;
}

GameContext detect_context_raw(const std::uint8_t *ram, std::uint32_t ram_size,
                              std::uint32_t &cached_race_offset) {
  if (!word_in_ram(kOwnerPointerAddress, ram_size))
    return GameContext::Unknown;

  const std::uint32_t owner = read_le32(ram + kOwnerPointerAddress);
  if (owner == 0)
    return GameContext::Menu;

  const std::uint32_t race_obj = find_race_object(ram, ram_size, owner, cached_race_offset);
  if (race_obj == 0)
    return GameContext::Menu;

  const std::uint32_t race_vptr = read_le32(ram + race_obj);
  // Attract mode arcade demo uses race structures but is logically a menu/attract sequence.
  if (is_attract_demo_vtable(race_vptr))
    return GameContext::Menu;

  // Check pause pointer inside race object (+0xE418)
  const std::uint64_t pause_slot = static_cast<std::uint64_t>(race_obj) + kRacePausePointerOffset;
  if (word_in_ram(pause_slot, ram_size)) {
    const std::uint32_t pause_obj = read_le32(ram + pause_slot);
    if (pause_obj != 0 && word_in_ram(pause_obj + kPauseObjectVtableOffset, ram_size)) {
      const std::uint32_t pause_vptr = read_le32(ram + pause_obj + kPauseObjectVtableOffset);
      if (is_photo_pause_vtable(pause_vptr))
        return GameContext::PhotoPause;
      if (is_pause_vtable(pause_vptr))
        return GameContext::Pause;
    }
  }

  return GameContext::Race;
}

GameContext update_context_at_vblank(const std::uint8_t *ram, std::uint32_t ram_size,
                                    ContextProbeState &state) {
  const GameContext raw = detect_context_raw(ram, ram_size, state.cached_race_offset);

  if (raw == state.raw_context) {
    ++state.stable_count;
    if (state.stable_count >= state.debounce_threshold && state.active_context != raw) {
      const GameContext previous = state.active_context;
      state.active_context = raw;
      ++state.transitions;
      if (state.logging_enabled) {
        std::fprintf(stderr, "[context] transition %s -> %s (stable %u frames, total transitions %llu)\n",
                     context_name(previous), context_name(raw),
                     static_cast<unsigned>(state.stable_count),
                     static_cast<unsigned long long>(state.transitions));
      }
    }
  } else {
    state.raw_context = raw;
    state.stable_count = 1;
    // Immediate fallback: if debounce_threshold is 1, apply immediately
    if (state.debounce_threshold <= 1 && state.active_context != raw) {
      const GameContext previous = state.active_context;
      state.active_context = raw;
      ++state.transitions;
      if (state.logging_enabled) {
        std::fprintf(stderr, "[context] transition %s -> %s (immediate)\n",
                     context_name(previous), context_name(raw));
      }
    }
  }

  return state.active_context;
}

} // namespace gt4::context
