#define _CRT_SECURE_NO_WARNINGS 1
#include "fps_probe.hpp"

#ifdef _WIN32
#include <windows.h>
#endif

#include <cassert>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <stdexcept>
#include <string>
#include <vector>

namespace {

std::FILE *create_temp_stream() {
  std::FILE *stream = nullptr;
#ifdef _WIN32
  if (tmpfile_s(&stream) == 0 && stream != nullptr) return stream;
  char temp_dir[MAX_PATH];
  if (GetTempPathA(sizeof temp_dir, temp_dir) != 0) {
    char temp_file[MAX_PATH];
    if (GetTempFileNameA(temp_dir, "gt4fps", 0, temp_file) != 0) {
      stream = std::fopen(temp_file, "w+D"); // 'D' = delete on close on Windows CRT
      if (stream) return stream;
      return std::fopen(temp_file, "w+b");
    }
  }
#endif
  return std::tmpfile();
}

std::uint64_t mock_time_us = 0;
std::uint64_t mock_clock_calls = 0;

std::uint64_t mock_clock() noexcept {
  ++mock_clock_calls;
  return mock_time_us;
}

void require(bool condition, const char *msg) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", msg);
    std::exit(1);
  }
}

bool near_f(float a, float b, float tol = 1e-4f) {
  return std::fabs(a - b) <= tol;
}

std::string read_stream(std::FILE *stream) {
  std::fflush(stream);
  std::rewind(stream);
  std::string result;
  char buf[512];
  while (std::fgets(buf, sizeof buf, stream)) {
    result += buf;
  }
  std::fseek(stream, 0, SEEK_END);
  return result;
}

// Minimal mock CPU structure matching dispatch layout
struct MockCpu {
  struct Reg {
    std::uint32_t w[2] = {};
  };
  std::uint32_t pc = 0;
  Reg r[32] = {};
};

void test_env_and_defaults() {
  gt4::fps_probe::Probe probe;

  // 1. Unset / null -> disabled
  probe.init(nullptr, nullptr);
  require(!probe.enabled(), "null env must default to disabled");
  require(!probe.diag_update_only(), "diag update-only must be false");

  // 2. Off values
  probe.init("0", nullptr);
  require(!probe.enabled(), "'0' must be disabled");
  probe.init("", nullptr);
  require(!probe.enabled(), "empty string must be disabled");
  probe.init("false", nullptr);
  require(!probe.enabled(), "'false' must be disabled");
  probe.init("2", nullptr);
  require(!probe.enabled(), "'2' must be disabled");

  // 3. Enabled
  probe.init("1", nullptr);
  require(probe.enabled(), "'1' must be enabled");
  require(!probe.diag_update_only(), "diag update-only must default off when enabled");

  // 4. Diagnostic update-only
  probe.init("1", "1");
  require(probe.enabled(), "enabled with diag");
  require(probe.diag_update_only(), "diag update-only active");

  // 5. Diag flag without probe enabled
  probe.init("0", "1");
  require(!probe.enabled(), "disabled probe ignores diag flag");
  require(!probe.diag_update_only(), "diag is inactive when probe is disabled");
}

void test_disabled_fast_path() {
  gt4::fps_probe::Probe probe;
  probe.init("0", nullptr);
  require(!probe.enabled(), "probe must be disabled");

  mock_clock_calls = 0;
  mock_time_us = 1000;
  probe.set_clock_adapter(&mock_clock);

  std::vector<std::uint8_t> ram(0x10000, 0);

  // Calling any hook method while disabled must do nothing and never query clock
  probe.on_update_enter(0x1000, ram.data(), ram.size());
  probe.on_update_leave(ram.data(), ram.size());
  probe.on_render_enter(0x1000, ram.data(), ram.size());
  probe.on_render_leave(ram.data(), ram.size());
  probe.on_idle_enter(0x1000, ram.data(), ram.size());
  probe.on_idle_leave(ram.data(), ram.size());
  probe.on_flip_wait_enter();
  probe.on_flip_wait_leave();
  probe.on_frame_boundary();

  require(mock_clock_calls == 0, "disabled probe must not invoke clock adapter");
  require(probe.stats().frame_count == 0, "no frames recorded while disabled");
  require(probe.current_phase() == gt4::fps_probe::MainLoopPhase::outside, "phase stays outside");
}

void test_state_and_phase_timings() {
  gt4::fps_probe::Probe probe;
  probe.init("1", nullptr);
  require(probe.enabled(), "probe must be enabled");

  mock_time_us = 1000000; // 1.000s
  mock_clock_calls = 0;
  probe.set_clock_adapter(&mock_clock);

  // Create a 32 MB synthetic RAM space (PS2 EE RAM)
  std::vector<std::uint8_t> ram(32u << 20, 0);

  const std::uint32_t race_obj = 0x10000;
  const std::uint32_t dynamics_ptr = 0x20000;
  std::memcpy(ram.data() + race_obj + 0x70u, &dynamics_ptr, sizeof dynamics_ptr);

  // Race clock at DynamicsConductor + 0xF898
  std::uint32_t race_clock = 32950;
  std::memcpy(ram.data() + dynamics_ptr + 0xF898u, &race_clock, sizeof race_clock);

  // Render counter at RacePS2Base + 0xE32C
  std::uint32_t render_counter = 1970;
  std::memcpy(ram.data() + race_obj + 0xE32Cu, &render_counter, sizeof render_counter);

  // HUD history at RacePS2Base + 0xCF5A
  std::uint8_t hud_hist = 0x01;
  ram[race_obj + 0xCF5Au] = hud_hist;

  // Double buffer toggle at 0x00620210
  std::uint32_t buf_toggle = 1;
  std::memcpy(ram.data() + 0x00620210u, &buf_toggle, sizeof buf_toggle);

  // CourseData ms at 0x006214D8
  std::uint32_t course_ms = 10950;
  std::memcpy(ram.data() + 0x006214D8u, &course_ms, sizeof course_ms);

  // CameraView at race_obj + 0x80 + 0x158 = 0x101D8
  const std::uint32_t cam_view = race_obj + 0x80u + 0x158u;
  const float fov = 60.0f;
  std::memcpy(ram.data() + cam_view + 0x14u, &fov, sizeof fov);

  const float pose0_angle_x = 0.123f;
  std::memcpy(ram.data() + cam_view + 0x28u, &pose0_angle_x, sizeof pose0_angle_x);

  const float matrix_m00 = 1.0f;
  std::memcpy(ram.data() + cam_view + 0x100u, &matrix_m00, sizeof matrix_m00);

  // Simulate 3 frames
  for (int f = 0; f < 3; ++f) {
    // 1. Update phase: 1,400 us
    probe.on_update_enter(race_obj, ram.data(), ram.size());
    require(probe.current_phase() == gt4::fps_probe::MainLoopPhase::update, "in update");
    mock_time_us += 1400;
    race_clock += 50; // Simulation steps +50 per step
    std::memcpy(ram.data() + dynamics_ptr + 0xF898u, &race_clock, sizeof race_clock);
    probe.on_update_leave(ram.data(), ram.size());
    require(probe.current_phase() == gt4::fps_probe::MainLoopPhase::outside, "left update");

    // 2. Render phase: 8,200 us
    probe.on_render_enter(race_obj, ram.data(), ram.size());
    require(probe.current_phase() == gt4::fps_probe::MainLoopPhase::render, "in render");
    mock_time_us += 8200;
    render_counter += 1; // Render increments frame counter
    std::memcpy(ram.data() + race_obj + 0xE32Cu, &render_counter, sizeof render_counter);
    hud_hist = static_cast<std::uint8_t>((hud_hist << 1) | 1);
    ram[race_obj + 0xCF5Au] = hud_hist;
    buf_toggle ^= 1;
    std::memcpy(ram.data() + 0x00620210u, &buf_toggle, sizeof buf_toggle);
    probe.on_render_leave(ram.data(), ram.size());
    require(probe.current_phase() == gt4::fps_probe::MainLoopPhase::outside, "left render");

    // 3. Idle phase: 180 us
    probe.on_idle_enter(race_obj, ram.data(), ram.size());
    require(probe.current_phase() == gt4::fps_probe::MainLoopPhase::idle, "in idle");
    mock_time_us += 180;
    probe.on_idle_leave(ram.data(), ram.size());
    require(probe.current_phase() == gt4::fps_probe::MainLoopPhase::outside, "left idle");

    // 4. Flip wait phase: 6,820 us (completing 16.6ms frame)
    probe.on_flip_wait_enter();
    require(probe.current_phase() == gt4::fps_probe::MainLoopPhase::flip_wait, "in flip wait");
    mock_time_us += 6820;
    probe.on_flip_wait_leave();
    require(probe.current_phase() == gt4::fps_probe::MainLoopPhase::outside, "left flip wait");

    // 5. Frame boundary
    probe.on_frame_boundary();
  }

  const auto &s = probe.stats();
  require(s.frame_count == 3, "3 frames counted");
  require(s.update_count == 3, "3 updates");
  require(s.render_count == 3, "3 renders");
  require(s.idle_count == 3, "3 idles");
  require(s.flip_wait_count == 3, "3 flip waits");

  require(s.update_time_us == 3 * 1400, "update timing accumulated");
  require(s.render_time_us == 3 * 8200, "render timing accumulated");
  require(s.idle_time_us == 3 * 180, "idle timing accumulated");
  require(s.flip_wait_time_us == 3 * 6820, "flip wait timing accumulated");

  require(s.race_clock_f898 == 32950 + 150, "race clock advanced by +150");
  require(s.render_frame_counter == 1970 + 3, "render counter advanced by +3");
  require(s.hud_history_byte == 0x0F, "hud history shifted");
  require(near_f(s.camera_fov, 60.0f), "camera fov observed");
  require(near_f(s.camera_pose0.angles[0].x, 0.123f), "camera pose0 angle x observed");
  require(near_f(s.camera_cached_matrix[0], 1.0f), "camera cached matrix m00 observed");
}

void test_render_purity_diff_attribution() {
  gt4::fps_probe::Probe probe;
  probe.init("1", nullptr);
  probe.set_clock_adapter(&mock_clock);

  std::vector<std::uint8_t> ram(32u << 20, 0);

  const std::uint32_t race_obj = 0x10000;
  const std::uint32_t dynamics_ptr = 0x20000;
  std::memcpy(ram.data() + race_obj + 0x70u, &dynamics_ptr, sizeof dynamics_ptr);

  // Set up explicit watched spans
  probe.clear_watched_spans();
  probe.add_watched_span(race_obj + 0xE320u, 0x20u, "render_counter_span");
  probe.add_watched_span(race_obj + 0xCF50u, 0x20u, "hud_history_span");
  probe.add_watched_span(0x00620200u, 0x20u, "buf_toggle_span");
  probe.add_watched_span(dynamics_ptr + 0xF880u, 0x40u, "dynamics_sim_state");

  // Enter render (sets active_race_obj and takes snapshot)
  probe.on_render_enter(race_obj, ram.data(), ram.size());

  // Mutate known render-only fields
  std::uint32_t new_counter = 100;
  std::memcpy(ram.data() + race_obj + 0xE32Cu, &new_counter, sizeof new_counter);

  std::uint8_t new_hud = 0x02;
  ram[race_obj + 0xCF5Au] = new_hud;

  std::uint32_t new_buf = 1;
  std::memcpy(ram.data() + 0x00620210u, &new_buf, sizeof new_buf);

  // Run purity diff
  probe.diff_render_purity(ram.data(), ram.size(), /*is_idle=*/false);

  const auto &p1 = probe.stats().purity;
  require(p1.mutated_words == 3, "3 legitimate words mutated");
  require(p1.render_counter_mutations == 1, "1 render counter mutation attributed");
  require(p1.hud_history_mutations == 1, "1 hud history mutation attributed");
  require(p1.double_buffer_mutations == 1, "1 double buffer mutation attributed");
  require(p1.unattributed_mutations == 0, "0 unattributed mutations");
  require(p1.simulation_clean(), "simulation state is clean");

  // Now introduce an illegal unattributed mutation inside simulation state (DynamicsConductor)
  std::uint32_t illegal_val = 0x1337;
  std::memcpy(ram.data() + dynamics_ptr + 0xF884u, &illegal_val, sizeof illegal_val);

  probe.diff_render_purity(ram.data(), ram.size(), /*is_idle=*/false);

  const auto &p2 = probe.stats().purity;
  require(p2.unattributed_mutations == 1, "1 illegal mutation caught");
  require(!p2.simulation_clean(), "simulation purity must report contaminated");
  require(!probe.recent_mutations().empty(), "recent mutations list has recorded the fault");
  require(probe.recent_mutations().front().address == dynamics_ptr + 0xF884u, "exact fault address recorded");
  require(probe.recent_mutations().front().new_value == 0x1337, "exact fault value recorded");
}

void test_report_formatting() {
  gt4::fps_probe::Probe probe;

  // 1. Report when disabled: writes nothing
  probe.init("0", nullptr);
  std::FILE *stream = create_temp_stream();
  require(stream != nullptr, "temp stream created");
  probe.format_report(stream);
  require(read_stream(stream).empty(), "disabled report must be silent");

  // 2. Report when enabled
  probe.init("1", nullptr);
  mock_time_us = 1000000;
  probe.set_clock_adapter(&mock_clock);

  std::vector<std::uint8_t> ram(32u << 20, 0);
  const std::uint32_t race_obj = 0x10000;
  probe.on_update_enter(race_obj, ram.data(), ram.size());
  mock_time_us += 1500;
  probe.on_update_leave(ram.data(), ram.size());

  probe.on_render_enter(race_obj, ram.data(), ram.size());
  mock_time_us += 8000;
  probe.on_render_leave(ram.data(), ram.size());

  probe.on_idle_enter(race_obj, ram.data(), ram.size());
  mock_time_us += 200;
  probe.on_idle_leave(ram.data(), ram.size());

  probe.on_flip_wait_enter();
  mock_time_us += 6900;
  probe.on_flip_wait_leave();

  probe.on_frame_boundary();

  std::rewind(stream);
  probe.format_report(stream);
  const std::string text = read_stream(stream);

  require(text.find("[fps-probe] interval: frames=1") != std::string::npos, "interval header");
  require(text.find("update_ms=1.500") != std::string::npos, "update ms formatted");
  require(text.find("render_ms=8.000") != std::string::npos, "render ms formatted");
  require(text.find("idle_ms=0.200") != std::string::npos, "idle ms formatted");
  require(text.find("flip_ms=6.900") != std::string::npos, "flip ms formatted");
  require(text.find("[fps-probe] guest: clock_f898=") != std::string::npos, "guest state line");
  require(text.find("[fps-probe] camera: fov=") != std::string::npos, "camera line");
  require(text.find("[fps-probe] purity: mutations=0") != std::string::npos, "purity line");
  require(text.find("[fps-probe] verdict: clean (unattributed_words=0)") != std::string::npos, "verdict clean");

  std::fclose(stream);
}

void test_dispatch_helper() {
  gt4::fps_probe::Probe &probe = gt4::fps_probe::Probe::shared();
  probe.init("1", nullptr);
  mock_time_us = 2000000;
  probe.set_clock_adapter(&mock_clock);

  std::vector<std::uint8_t> ram(32u << 20, 0);
  const std::uint32_t race_obj = 0x10000;

  MockCpu cpu;
  cpu.r[4].w[0] = race_obj; // $a0

  const auto &hooks = gt4::target::kV200Hooks;

  // 1. Dispatch enters update functor (0x00100EE0)
  cpu.pc = hooks.main_update_functor;
  cpu.r[31].w[0] = 0x1000; // arbitrary return
  gt4::fps_probe::on_dispatch(cpu, cpu.pc, ram.data(), ram.size());
  require(probe.current_phase() == gt4::fps_probe::MainLoopPhase::update, "on_dispatch update entered");

  // 2. Dispatch returns from update functor (ra == 0x001095C0)
  cpu.pc = 0x1000;
  cpu.r[31].w[0] = hooks.functor_return;
  gt4::fps_probe::on_dispatch(cpu, cpu.pc, ram.data(), ram.size());
  require(probe.current_phase() == gt4::fps_probe::MainLoopPhase::outside, "on_dispatch update left");

  // 3. Dispatch enters render functor (0x00100F50)
  cpu.pc = hooks.main_render_functor;
  cpu.r[31].w[0] = 0x1000;
  gt4::fps_probe::on_dispatch(cpu, cpu.pc, ram.data(), ram.size());
  require(probe.current_phase() == gt4::fps_probe::MainLoopPhase::render, "on_dispatch render entered");

  // 4. Dispatch returns from render functor (ra == 0x001095C0)
  cpu.pc = 0x1000;
  cpu.r[31].w[0] = hooks.functor_return;
  gt4::fps_probe::on_dispatch(cpu, cpu.pc, ram.data(), ram.size());
  require(probe.current_phase() == gt4::fps_probe::MainLoopPhase::outside, "on_dispatch render left");

  // 5. Dispatch enters idle functor (0x00100FC0)
  cpu.pc = hooks.main_idle_functor;
  cpu.r[31].w[0] = 0x1000;
  gt4::fps_probe::on_dispatch(cpu, cpu.pc, ram.data(), ram.size());
  require(probe.current_phase() == gt4::fps_probe::MainLoopPhase::idle, "on_dispatch idle entered");

  // 6. Dispatch returns from idle functor (ra == 0x001095C0)
  cpu.pc = 0x1000;
  cpu.r[31].w[0] = hooks.functor_return;
  gt4::fps_probe::on_dispatch(cpu, cpu.pc, ram.data(), ram.size());
  require(probe.current_phase() == gt4::fps_probe::MainLoopPhase::outside, "on_dispatch idle left");

  // 7. Dispatch enters flip wait (0x00107878)
  cpu.pc = hooks.main_flip_wait;
  cpu.r[31].w[0] = hooks.flip_wait_return;
  gt4::fps_probe::on_dispatch(cpu, cpu.pc, ram.data(), ram.size());
  require(probe.current_phase() == gt4::fps_probe::MainLoopPhase::flip_wait, "on_dispatch flip wait entered");

  // 8. Dispatch returns from flip wait (ra == 0x00101818)
  cpu.pc = hooks.flip_wait_return;
  cpu.r[31].w[0] = hooks.flip_wait_return;
  gt4::fps_probe::on_dispatch(cpu, cpu.pc, ram.data(), ram.size());
  require(probe.current_phase() == gt4::fps_probe::MainLoopPhase::outside, "on_dispatch flip wait left");
}

}  // namespace

int main() {
  test_env_and_defaults();
  test_disabled_fast_path();
  test_state_and_phase_timings();
  test_render_purity_diff_attribution();
  test_report_formatting();
  test_dispatch_helper();

  std::printf("fps_probe_fixture: ok\n");
  return 0;
}
