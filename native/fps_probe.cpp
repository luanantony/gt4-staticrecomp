#define _CRT_SECURE_NO_WARNINGS 1
#include "fps_probe.hpp"

#include <chrono>
#include <cstdlib>
#include <cstring>

namespace gt4::fps_probe {

namespace {

inline std::uint32_t read_u32(const std::uint8_t *ram, std::size_t ram_size, std::uint32_t addr) noexcept {
  if (addr + 4u > ram_size) return 0;
  std::uint32_t v = 0;
  std::memcpy(&v, ram + addr, sizeof v);
  return v;
}

inline float read_f32(const std::uint8_t *ram, std::size_t ram_size, std::uint32_t addr) noexcept {
  if (addr + 4u > ram_size) return 0.0f;
  float v = 0.0f;
  std::memcpy(&v, ram + addr, sizeof v);
  return v;
}

inline std::uint8_t read_u8(const std::uint8_t *ram, std::size_t ram_size, std::uint32_t addr) noexcept {
  if (addr >= ram_size) return 0;
  return ram[addr];
}

std::uint64_t default_steady_clock_us() noexcept {
  using namespace std::chrono;
  return duration_cast<microseconds>(steady_clock::now().time_since_epoch()).count();
}

}  // namespace

MutationBucket classify_mutation(std::uint32_t addr, std::uint32_t race_obj,
                                 std::uint32_t camera_addr, std::uint32_t coursedata_addr) noexcept {
  if (race_obj != 0) {
    if (addr == race_obj + 0xE32Cu) return MutationBucket::render_frame_counter;
    if (addr >= race_obj + 0xCF58u && addr <= race_obj + 0xCF5Cu) return MutationBucket::hud_history_byte;
    if (addr >= race_obj + 0xCF38u && addr <= race_obj + 0xCF70u) return MutationBucket::hud_history_byte;
  }
  if (camera_addr != 0) {
    if (addr >= camera_addr + 0x100u && addr < camera_addr + 0x140u) return MutationBucket::camera_cached_matrix;
  }
  if (addr >= 0x0062020Cu && addr <= 0x00620214u) return MutationBucket::double_buffer_toggle;
  if (coursedata_addr != 0 && (addr >= coursedata_addr - 4u && addr <= coursedata_addr + 4u)) {
    return MutationBucket::coursedata_ms;
  }
  if (addr >= 0x006214D4u && addr <= 0x006214DCu) return MutationBucket::coursedata_ms;

  return MutationBucket::unattributed;
}

Probe::Probe() noexcept {
  init();
}

Probe &Probe::shared() noexcept {
  static Probe instance;
  return instance;
}

void Probe::init(const char *env_fps_probe, const char *env_diag_update_only) noexcept {
  const char *probe_val = env_fps_probe ? env_fps_probe : std::getenv("GT4_FPS_PROBE");
  enabled_ = (probe_val != nullptr && std::strcmp(probe_val, "1") == 0);

  const char *diag_val = env_diag_update_only ? env_diag_update_only : std::getenv("GT4_FPS_PROBE_DIAG_UPDATE_ONLY");
  diag_update_only_ = enabled_ && (diag_val != nullptr && std::strcmp(diag_val, "1") == 0);

  clock_us_ = &default_steady_clock_us;
  reset();
}

void Probe::reset() noexcept {
  phase_ = MainLoopPhase::outside;
  phase_start_us_ = 0;
  frame_start_us_ = 0;
  active_race_obj_ = 0;
  active_camera_view_ = 0;
  active_dynamics_ = 0;

  stats_ = FrameStats{};
  watched_spans_.clear();
  snapshot_buffer_.clear();
  recent_mutations_.clear();
}

void Probe::set_clock_adapter(ClockFn clock_us) noexcept {
  clock_us_ = clock_us ? clock_us : &default_steady_clock_us;
}

std::uint64_t Probe::now_us() const noexcept {
  return clock_us_ ? clock_us_() : 0;
}

void Probe::clear_watched_spans() noexcept {
  watched_spans_.clear();
  snapshot_buffer_.clear();
}

void Probe::add_watched_span(std::uint32_t guest_addr, std::uint32_t size_bytes, const char *name) noexcept {
  watched_spans_.push_back({guest_addr, size_bytes, name});
}

void Probe::setup_default_spans(std::uint32_t race_obj) noexcept {
  if (!watched_spans_.empty() || race_obj == 0) return;

  // RacePS2Base header and tails
  add_watched_span(race_obj + 0xE280u, 0x100u, "RacePS2Base_tail_render_counter");
  add_watched_span(race_obj + 0xCF30u, 0x60u,  "RacePS2Base_hud_history");

  // CameraManager (race_obj + 0x80) and CameraView 0
  const std::uint32_t cam_mgr = race_obj + 0x80u;
  active_camera_view_ = cam_mgr + 0x158u;
  add_watched_span(active_camera_view_ + 0x20u, 0x140u, "CameraView_poses_matrix");

  // DynamicsConductor simulation state (race_obj + 0x70 -> pointer)
  active_dynamics_ = race_obj + 0x70u;
  add_watched_span(active_dynamics_ + 0xF880u, 0x40u, "DynamicsConductor_clock");

  // CourseData ms and buffer toggle
  add_watched_span(0x006214C0u, 0x40u, "CourseData_clock");
  add_watched_span(0x00620200u, 0x20u, "DoubleBuffer_toggle");
}

void Probe::on_update_enter(std::uint32_t obj_ptr, const std::uint8_t *ram, std::size_t ram_size) noexcept {
  if (!enabled_) return;

  if (active_race_obj_ == 0 && obj_ptr != 0) {
    active_race_obj_ = obj_ptr;
    setup_default_spans(active_race_obj_);
  }

  phase_ = MainLoopPhase::update;
  phase_start_us_ = now_us();
  if (frame_start_us_ == 0) frame_start_us_ = phase_start_us_;

  ++stats_.update_count;

  // Read guest race clock if accessible
  if (ram != nullptr && active_race_obj_ != 0) {
    const std::uint32_t dynamics_ptr = read_u32(ram, ram_size, active_race_obj_ + 0x70u);
    if (dynamics_ptr != 0) {
      stats_.race_clock_f898 = read_u32(ram, ram_size, dynamics_ptr + 0xF898u);
    }
  }
}

void Probe::on_update_leave(const std::uint8_t *ram, std::size_t ram_size) noexcept {
  if (!enabled_ || phase_ != MainLoopPhase::update) return;

  const std::uint64_t now = now_us();
  stats_.update_time_us += (now - phase_start_us_);
  phase_ = MainLoopPhase::outside;

  if (ram != nullptr && active_race_obj_ != 0) {
    const std::uint32_t dynamics_ptr = read_u32(ram, ram_size, active_race_obj_ + 0x70u);
    if (dynamics_ptr != 0) {
      stats_.race_clock_f898 = read_u32(ram, ram_size, dynamics_ptr + 0xF898u);
    }
  }
}

void Probe::on_render_enter(std::uint32_t obj_ptr, const std::uint8_t *ram, std::size_t ram_size) noexcept {
  if (!enabled_) return;

  if (active_race_obj_ == 0 && obj_ptr != 0) {
    active_race_obj_ = obj_ptr;
    setup_default_spans(active_race_obj_);
  }

  phase_ = MainLoopPhase::render;
  phase_start_us_ = now_us();
  if (frame_start_us_ == 0) frame_start_us_ = phase_start_us_;

  ++stats_.render_count;

  // Snapshot memory before render pass
  take_render_snapshot(ram, ram_size);
}

void Probe::on_render_leave(const std::uint8_t *ram, std::size_t ram_size) noexcept {
  if (!enabled_ || phase_ != MainLoopPhase::render) return;

  const std::uint64_t now = now_us();
  stats_.render_time_us += (now - phase_start_us_);
  phase_ = MainLoopPhase::outside;

  // Diff memory altered during render
  diff_render_purity(ram, ram_size, /*is_idle_pass=*/false);

  // Read guest state post-render
  if (ram != nullptr && active_race_obj_ != 0) {
    stats_.render_frame_counter = read_u32(ram, ram_size, active_race_obj_ + 0xE32Cu);
    stats_.hud_history_byte     = read_u8(ram, ram_size, active_race_obj_ + 0xCF5Au);
    stats_.double_buffer_toggle = read_u32(ram, ram_size, 0x00620210u);
    stats_.coursedata_ms        = read_u32(ram, ram_size, 0x006214D8u);

    // Read CameraView poses and matrix
    if (active_camera_view_ != 0) {
      stats_.camera_fov = read_f32(ram, ram_size, active_camera_view_ + 0x14u);
      for (int i = 0; i < 3; ++i) {
        stats_.camera_pose0.angles[i].x = read_f32(ram, ram_size, active_camera_view_ + 0x28u + i * 12u);
        stats_.camera_pose0.angles[i].y = read_f32(ram, ram_size, active_camera_view_ + 0x2Cu + i * 12u);
        stats_.camera_pose0.angles[i].z = read_f32(ram, ram_size, active_camera_view_ + 0x30u + i * 12u);

        stats_.camera_pose0.translations[i].x = read_f32(ram, ram_size, active_camera_view_ + 0x4Cu + i * 12u);
        stats_.camera_pose0.translations[i].y = read_f32(ram, ram_size, active_camera_view_ + 0x50u + i * 12u);
        stats_.camera_pose0.translations[i].z = read_f32(ram, ram_size, active_camera_view_ + 0x54u + i * 12u);

        stats_.camera_pose1.angles[i].x = read_f32(ram, ram_size, active_camera_view_ + 0x94u + i * 12u);
        stats_.camera_pose1.angles[i].y = read_f32(ram, ram_size, active_camera_view_ + 0x98u + i * 12u);
        stats_.camera_pose1.angles[i].z = read_f32(ram, ram_size, active_camera_view_ + 0x9Cu + i * 12u);

        stats_.camera_pose1.translations[i].x = read_f32(ram, ram_size, active_camera_view_ + 0xB8u + i * 12u);
        stats_.camera_pose1.translations[i].y = read_f32(ram, ram_size, active_camera_view_ + 0xBCu + i * 12u);
        stats_.camera_pose1.translations[i].z = read_f32(ram, ram_size, active_camera_view_ + 0xC0u + i * 12u);
      }
      for (int m = 0; m < 16; ++m) {
        stats_.camera_cached_matrix[m] = read_f32(ram, ram_size, active_camera_view_ + 0x100u + m * 4u);
      }
    }
  }
}

void Probe::on_idle_enter(std::uint32_t obj_ptr, const std::uint8_t *ram, std::size_t ram_size) noexcept {
  (void)obj_ptr; (void)ram; (void)ram_size;
  if (!enabled_) return;

  phase_ = MainLoopPhase::idle;
  phase_start_us_ = now_us();
  ++stats_.idle_count;
}

void Probe::on_idle_leave(const std::uint8_t *ram, std::size_t ram_size) noexcept {
  if (!enabled_ || phase_ != MainLoopPhase::idle) return;

  const std::uint64_t now = now_us();
  stats_.idle_time_us += (now - phase_start_us_);
  phase_ = MainLoopPhase::outside;

  // Diff memory altered during idle (DMAsafe swaps)
  diff_render_purity(ram, ram_size, /*is_idle_pass=*/true);
}

void Probe::on_flip_wait_enter() noexcept {
  if (!enabled_) return;

  phase_ = MainLoopPhase::flip_wait;
  phase_start_us_ = now_us();
  ++stats_.flip_wait_count;
}

void Probe::on_flip_wait_leave() noexcept {
  if (!enabled_ || phase_ != MainLoopPhase::flip_wait) return;

  const std::uint64_t now = now_us();
  stats_.flip_wait_time_us += (now - phase_start_us_);
  phase_ = MainLoopPhase::outside;
}

void Probe::on_frame_boundary() noexcept {
  if (!enabled_) return;

  if (phase_ == MainLoopPhase::flip_wait) {
    on_flip_wait_leave();
  }

  const std::uint64_t now = now_us();
  if (frame_start_us_ != 0) {
    stats_.total_frame_time_us += (now - frame_start_us_);
  }
  frame_start_us_ = now;
  ++stats_.frame_count;
}

void Probe::take_render_snapshot(const std::uint8_t *ram, std::size_t ram_size) noexcept {
  if (ram == nullptr || watched_spans_.empty()) return;

  std::size_t total_bytes = 0;
  for (const auto &span : watched_spans_) {
    total_bytes += span.size_bytes;
  }
  snapshot_buffer_.resize(total_bytes);

  std::size_t offset = 0;
  for (const auto &span : watched_spans_) {
    if (span.guest_addr + span.size_bytes <= ram_size) {
      std::memcpy(snapshot_buffer_.data() + offset, ram + span.guest_addr, span.size_bytes);
    } else {
      std::memset(snapshot_buffer_.data() + offset, 0, span.size_bytes);
    }
    offset += span.size_bytes;
  }
}

void Probe::diff_render_purity(const std::uint8_t *ram, std::size_t ram_size, bool is_idle_pass) noexcept {
  if (ram == nullptr || snapshot_buffer_.empty()) return;

  std::size_t offset = 0;
  for (const auto &span : watched_spans_) {
    if (span.guest_addr + span.size_bytes > ram_size) {
      offset += span.size_bytes;
      continue;
    }

    const std::size_t words = span.size_bytes / 4u;
    stats_.purity.total_inspected_words += words;

    for (std::size_t w = 0; w < words; ++w) {
      const std::uint32_t word_addr = span.guest_addr + static_cast<std::uint32_t>(w * 4u);
      std::uint32_t old_val = 0;
      std::uint32_t new_val = 0;

      std::memcpy(&old_val, snapshot_buffer_.data() + offset + w * 4u, sizeof old_val);
      std::memcpy(&new_val, ram + word_addr, sizeof new_val);

      if (old_val != new_val) {
        ++stats_.purity.mutated_words;
        // Update snapshot so subsequent checks on this pass don't re-trigger
        std::memcpy(snapshot_buffer_.data() + offset + w * 4u, &new_val, sizeof new_val);

        MutationBucket bucket = classify_mutation(word_addr, active_race_obj_, active_camera_view_, 0x006214D8u);
        if (is_idle_pass && bucket == MutationBucket::unattributed) {
          // If in an idle span and classified as buffer swap
          bucket = MutationBucket::dma_safe_idle_swap;
        }

        switch (bucket) {
          case MutationBucket::render_frame_counter:
            ++stats_.purity.render_counter_mutations;
            break;
          case MutationBucket::hud_history_byte:
            ++stats_.purity.hud_history_mutations;
            break;
          case MutationBucket::double_buffer_toggle:
            ++stats_.purity.double_buffer_mutations;
            break;
          case MutationBucket::coursedata_ms:
            ++stats_.purity.coursedata_mutations;
            break;
          case MutationBucket::camera_cached_matrix:
            ++stats_.purity.camera_matrix_mutations;
            break;
          case MutationBucket::dma_safe_idle_swap:
            ++stats_.purity.dma_swap_mutations;
            break;
          case MutationBucket::unattributed:
          default:
            ++stats_.purity.unattributed_mutations;
            if (recent_mutations_.size() < 16) {
              recent_mutations_.push_back({word_addr, old_val, new_val, MutationBucket::unattributed});
            }
            break;
        }
      }
    }
    offset += span.size_bytes;
  }
}

void Probe::format_report(std::FILE *stream) const noexcept {
  if (!enabled_ || stream == nullptr) return;

  const double f = (stats_.frame_count > 0) ? static_cast<double>(stats_.frame_count) : 1.0;
  const double update_ms = (static_cast<double>(stats_.update_time_us) / 1000.0) / f;
  const double render_ms = (static_cast<double>(stats_.render_time_us) / 1000.0) / f;
  const double idle_ms   = (static_cast<double>(stats_.idle_time_us) / 1000.0) / f;
  const double flip_ms   = (static_cast<double>(stats_.flip_wait_time_us) / 1000.0) / f;
  const double total_ms  = (static_cast<double>(stats_.total_frame_time_us) / 1000.0) / f;

  std::fprintf(stream,
               "[fps-probe] interval: frames=%llu update_ms=%.3f render_ms=%.3f idle_ms=%.3f flip_ms=%.3f total_ms=%.3f\n",
               static_cast<unsigned long long>(stats_.frame_count),
               update_ms, render_ms, idle_ms, flip_ms, total_ms);

  std::fprintf(stream,
               "[fps-probe] guest: clock_f898=%u render_counter=%u hud_hist=0x%02X buf_toggle=%u course_ms=%u\n",
               stats_.race_clock_f898,
               stats_.render_frame_counter,
               static_cast<unsigned>(stats_.hud_history_byte),
               stats_.double_buffer_toggle,
               stats_.coursedata_ms);

  std::fprintf(stream,
               "[fps-probe] camera: fov=%.1f cached_m00=%.4f\n",
               stats_.camera_fov,
               stats_.camera_cached_matrix[0]);

  std::fprintf(stream,
               "[fps-probe] purity: mutations=%llu (render_counter:%llu hud_hist:%llu buf_toggle:%llu cam_matrix:%llu dma_swap:%llu unattributed:%llu)\n",
               static_cast<unsigned long long>(stats_.purity.mutated_words),
               static_cast<unsigned long long>(stats_.purity.render_counter_mutations),
               static_cast<unsigned long long>(stats_.purity.hud_history_mutations),
               static_cast<unsigned long long>(stats_.purity.double_buffer_mutations),
               static_cast<unsigned long long>(stats_.purity.camera_matrix_mutations),
               static_cast<unsigned long long>(stats_.purity.dma_swap_mutations),
               static_cast<unsigned long long>(stats_.purity.unattributed_mutations));

  if (stats_.purity.simulation_clean()) {
    std::fprintf(stream, "[fps-probe] verdict: clean (unattributed_words=0)\n");
  } else {
    std::fprintf(stream, "[fps-probe] verdict: CONTAMINATED (unattributed_words=%llu)\n",
                 static_cast<unsigned long long>(stats_.purity.unattributed_mutations));
    for (std::size_t i = 0; i < recent_mutations_.size(); ++i) {
      const auto &m = recent_mutations_[i];
      std::fprintf(stream, "[fps-probe] mutation[%zu]: addr=0x%08X old=0x%08X new=0x%08X (unattributed)\n",
                   i, m.address, m.old_value, m.new_value);
    }
  }

  if (diag_update_only_) {
    std::fprintf(stream, "[fps-probe] diagnostic: UPDATE_ONLY_ACTIVE (render and idle bypassed for EE budgeting)\n");
  }
}

void Probe::finish(std::FILE *stream) noexcept {
  if (!enabled_ || stream == nullptr) return;
  format_report(stream);
}

}  // namespace gt4::fps_probe
