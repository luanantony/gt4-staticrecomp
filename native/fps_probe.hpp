#pragma once

// TASK-082 P0: frame-loop observation and dependency proof (docs/NATIVE-VARIABLE-FPS-STUDY.md).
//
// Behind GT4_FPS_PROBE=1 (default off):
//   - H0: measures update, render, idle (DMAsafe) and flip-wait phase timings per frame.
//   - H1: observes guest race clock (DynamicsConductor + 0xF898, +50 per simulation step),
//         render frame counter (RacePS2Base + 0xE32C), HUD history byte (RacePS2Base + 0xCF5A),
//         double-buffer toggle (0x00620210), CourseData ms (0x006214D8), and camera poses/matrix.
//   - H2: bounded render-purity watcher: compares memory outside packet buffers across render/idle,
//         classifies mutated words into known attributed buckets, and verifies unattributed == 0
//         for simulation state.
//
// Fast path when GT4_FPS_PROBE is unset / 0:
//   Every hook method returns immediately; zero clock calls, zero RAM reads, zero behavior change.
//   Probe off preserves baseline output; probe on also preserves baseline output and simulated state.

#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <functional>
#include <string>
#include <vector>

#include "game_target.hpp"

namespace gt4::fps_probe {

struct Vec3 {
  float x = 0.0f;
  float y = 0.0f;
  float z = 0.0f;
};

struct CameraPose {
  Vec3 angles[3];
  Vec3 translations[3];
};

enum class MutationBucket {
  render_frame_counter,   // RacePS2Base + 0xE32C
  hud_history_byte,       // RacePS2Base + 0xCF5A
  double_buffer_toggle,   // 0x00620210
  coursedata_ms,          // 0x006214D8
  camera_cached_matrix,   // CameraView + 0x100..0x13F
  dma_safe_idle_swap,     // per-car / course DMA buffer pointers in DMAsafe
  unattributed            // MUST be 0 for pure simulation state
};

struct MutationRecord {
  std::uint32_t address = 0;
  std::uint32_t old_value = 0;
  std::uint32_t new_value = 0;
  MutationBucket bucket = MutationBucket::unattributed;
};

struct PurityReport {
  std::uint64_t total_inspected_words = 0;
  std::uint64_t mutated_words = 0;
  std::uint64_t render_counter_mutations = 0;
  std::uint64_t hud_history_mutations = 0;
  std::uint64_t double_buffer_mutations = 0;
  std::uint64_t coursedata_mutations = 0;
  std::uint64_t camera_matrix_mutations = 0;
  std::uint64_t dma_swap_mutations = 0;
  std::uint64_t unattributed_mutations = 0;

  bool simulation_clean() const noexcept {
    return unattributed_mutations == 0;
  }
};

struct FrameStats {
  std::uint64_t update_count = 0;
  std::uint64_t render_count = 0;
  std::uint64_t idle_count = 0;
  std::uint64_t flip_wait_count = 0;
  std::uint64_t frame_count = 0;

  std::uint64_t update_time_us = 0;
  std::uint64_t render_time_us = 0;
  std::uint64_t idle_time_us = 0;
  std::uint64_t flip_wait_time_us = 0;
  std::uint64_t total_frame_time_us = 0;

  // Guest observed state
  std::uint32_t race_clock_f898 = 0;
  std::uint32_t render_frame_counter = 0;
  std::uint8_t  hud_history_byte = 0;
  std::uint32_t double_buffer_toggle = 0;
  std::uint32_t coursedata_ms = 0;

  CameraPose camera_pose0{};
  CameraPose camera_pose1{};
  float camera_cached_matrix[16] = {};
  float camera_fov = 0.0f;

  PurityReport purity{};
};

enum class MainLoopPhase {
  outside,
  update,
  render,
  idle,
  flip_wait
};

struct WatchedSpan {
  std::uint32_t guest_addr = 0;
  std::uint32_t size_bytes = 0;
  const char *name = nullptr;
};

class Probe {
public:
  Probe() noexcept;
  static Probe &shared() noexcept;

  // Environment and control
  void init(const char *env_fps_probe = nullptr, const char *env_diag_update_only = nullptr) noexcept;
  bool enabled() const noexcept { return enabled_; }
  bool diag_update_only() const noexcept { return diag_update_only_; }
  void reset() noexcept;

  // Clock adapter for testing
  using ClockFn = std::uint64_t (*)();
  void set_clock_adapter(ClockFn clock_us) noexcept;

  // Watched memory configuration
  void clear_watched_spans() noexcept;
  void add_watched_span(std::uint32_t guest_addr, std::uint32_t size_bytes, const char *name) noexcept;

  // Main-loop phase hooks
  void on_update_enter(std::uint32_t obj_ptr, const std::uint8_t *ram, std::size_t ram_size) noexcept;
  void on_update_leave(const std::uint8_t *ram, std::size_t ram_size) noexcept;

  void on_render_enter(std::uint32_t obj_ptr, const std::uint8_t *ram, std::size_t ram_size) noexcept;
  void on_render_leave(const std::uint8_t *ram, std::size_t ram_size) noexcept;

  void on_idle_enter(std::uint32_t obj_ptr, const std::uint8_t *ram, std::size_t ram_size) noexcept;
  void on_idle_leave(const std::uint8_t *ram, std::size_t ram_size) noexcept;

  void on_flip_wait_enter() noexcept;
  void on_flip_wait_leave() noexcept;

  void on_frame_boundary() noexcept;

  // Purity memory snapshot and diff
  void take_render_snapshot(const std::uint8_t *ram, std::size_t ram_size) noexcept;
  void diff_render_purity(const std::uint8_t *ram, std::size_t ram_size, bool is_idle_pass) noexcept;

  // Reporting
  void format_report(std::FILE *stream) const noexcept;
  void finish(std::FILE *stream = stderr) noexcept;

  const FrameStats &stats() const noexcept { return stats_; }
  MainLoopPhase current_phase() const noexcept { return phase_; }
  const std::vector<MutationRecord> &recent_mutations() const noexcept { return recent_mutations_; }

private:
  std::uint64_t now_us() const noexcept;
  void setup_default_spans(std::uint32_t race_obj) noexcept;

  bool enabled_ = false;
  bool diag_update_only_ = false;
  ClockFn clock_us_ = nullptr;

  MainLoopPhase phase_ = MainLoopPhase::outside;
  std::uint64_t phase_start_us_ = 0;
  std::uint64_t frame_start_us_ = 0;

  std::uint32_t active_race_obj_ = 0;
  std::uint32_t active_camera_view_ = 0;
  std::uint32_t active_dynamics_ = 0;

  FrameStats stats_{};

  std::vector<WatchedSpan> watched_spans_;
  std::vector<std::uint8_t> snapshot_buffer_;
  std::vector<MutationRecord> recent_mutations_;
};

// Pure classifier for mutated words
MutationBucket classify_mutation(std::uint32_t guest_addr, std::uint32_t race_obj,
                                 std::uint32_t camera_view_addr, std::uint32_t coursedata_addr) noexcept;

// Helper to hook ps2::dispatch when enabled
template <typename CpuType>
inline void on_dispatch(CpuType &c, std::uint32_t pc, std::uint8_t *ram, std::size_t ram_size) {
  Probe &probe = Probe::shared();
  if (!probe.enabled()) return;

  const auto &hooks = ::gt4::target::active().hooks;
  const std::uint32_t ra = c.r[31].w[0];

  // 1. Check functor return
  if (hooks.functor_return && ra == hooks.functor_return) {
    if (probe.current_phase() == MainLoopPhase::update) {
      probe.on_update_leave(ram, ram_size);
    } else if (probe.current_phase() == MainLoopPhase::render) {
      probe.on_render_leave(ram, ram_size);
    } else if (probe.current_phase() == MainLoopPhase::idle) {
      probe.on_idle_leave(ram, ram_size);
    }
  }

  // 2. Check flip wait return
  if (hooks.flip_wait_return && ra == hooks.flip_wait_return) {
    if (probe.current_phase() == MainLoopPhase::flip_wait) {
      probe.on_flip_wait_leave();
    }
  }

  // 3. Check functor entries
  if (hooks.main_update_functor && pc == hooks.main_update_functor) {
    probe.on_update_enter(c.r[4].w[0], ram, ram_size);
  } else if (hooks.main_render_functor && pc == hooks.main_render_functor) {
    probe.on_render_enter(c.r[4].w[0], ram, ram_size);
  } else if (hooks.main_idle_functor && pc == hooks.main_idle_functor) {
    probe.on_idle_enter(c.r[4].w[0], ram, ram_size);
  } else if (hooks.main_flip_wait && pc == hooks.main_flip_wait) {
    probe.on_flip_wait_enter();
  }
}

}  // namespace gt4::fps_probe
