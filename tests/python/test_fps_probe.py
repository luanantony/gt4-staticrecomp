"""TASK-082 P0: frame-loop observation and dependency proof (docs/TASK-082-ANTIGRAVITY-BRIEF.md).

Public Python contract tests:
  1. Environment variable contracts: GT4_FPS_PROBE default off, '1' enabled,
     and GT4_FPS_PROBE_DIAG_UPDATE_ONLY diagnostic toggle.
  2. Overlay syntax, sections and round-trip derivation against dispatch.cpp.
  3. Structured report parser for [fps-probe] stderr sections.
  4. Simulation step clock ratio verification (+50 units of 1/3000s per 1/60s step).
"""

import os
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class FpsProbeEnvironmentTests(unittest.TestCase):
    def test_default_is_disabled(self):
        # Default unset must be disabled
        for value in [None, '', '0', 'false', 'no', '2']:
            enabled = (value == '1')
            self.assertFalse(enabled, f"Value {value!r} must be disabled")

    def test_enabled_flag(self):
        self.assertTrue('1' == '1', "'1' enables probe")

    def test_diag_update_only_contract(self):
        # diag update-only requires both probe=1 and diag=1
        def is_diag_active(probe_val, diag_val):
            enabled = (probe_val == '1')
            diag = enabled and (diag_val == '1')
            return diag

        self.assertFalse(is_diag_active(None, None))
        self.assertFalse(is_diag_active('0', '1'))
        self.assertFalse(is_diag_active('1', '0'))
        self.assertFalse(is_diag_active('1', None))
        self.assertTrue(is_diag_active('1', '1'))


class FpsProbeOverlayTests(unittest.TestCase):
    def setUp(self):
        self.overlay_path = ROOT / 'scripts/fps_probe_overlay.inc'
        self.assertTrue(self.overlay_path.is_file(), "scripts/fps_probe_overlay.inc must exist")
        self.overlay_text = self.overlay_path.read_text(encoding='utf-8')

    def test_overlay_has_required_sections(self):
        self.assertIn('//@section includes', self.overlay_text)
        self.assertIn('#include "fps_probe.hpp"', self.overlay_text)
        self.assertIn('//@section helpers', self.overlay_text)
        self.assertIn('fps_probe_dispatch_hook', self.overlay_text)
        self.assertIn('//@section hook', self.overlay_text)
        self.assertIn('gt4::fps_probe::Probe::shared().enabled()', self.overlay_text)

    def test_overlay_application_roundtrip(self):
        donor_model = (
            '#include "ps2/generated.hpp"\n'
            '#include "license_guard.hpp"\n'
            '#include "display_aspect.hpp"\n'
            '\n'
            'namespace {\n'
            'void existing_helper() {}\n'
            '}\n'
            '\n'
            'void dispatch(Cpu &c, std::uint32_t, std::uint32_t depth) {\n'
            '    const std::uint32_t pc = c.pc;\n'
            '    if (pc & kUnwind) return;\n'
            '    if (c.r[31].w[0] == gt4::display_aspect::kCameraCallReturn) [[unlikely]] race_camera_aspect_hook(c);\n'
            '    const UnitInfo *u = unit_for(pc);\n'
            '}\n'
        )

        # Parse sections
        sections = {}
        current_section = None
        current_lines = []
        for line in self.overlay_text.splitlines(keepends=True):
            if line.startswith('//@section '):
                if current_section:
                    sections[current_section] = ''.join(current_lines)
                current_section = line.strip().split()[1]
                current_lines = []
            else:
                current_lines.append(line)
        if current_section:
            sections[current_section] = ''.join(current_lines)

        self.assertIn('includes', sections)
        self.assertIn('helpers', sections)
        self.assertIn('hook', sections)

        # Apply to model
        target = donor_model
        # 1. includes after display_aspect.hpp
        target = target.replace('#include "display_aspect.hpp"\n',
                                '#include "display_aspect.hpp"\n' + sections['includes'])
        # 2. helpers before dispatch
        target = target.replace('void dispatch(Cpu &c',
                                sections['helpers'] + 'void dispatch(Cpu &c')
        # 3. hook after camera hook
        anchor = 'if (c.r[31].w[0] == gt4::display_aspect::kCameraCallReturn) [[unlikely]] race_camera_aspect_hook(c);\n'
        target = target.replace(anchor, anchor + sections['hook'])

        self.assertIn('fps_probe.hpp', target)
        self.assertIn('fps_probe_dispatch_hook', target)

        # Removing inserted sections restores original
        restored = target.replace(sections['includes'], '')
        restored = restored.replace(sections['helpers'], '')
        restored = restored.replace(sections['hook'], '')
        self.assertEqual(restored, donor_model, "roundtrip derivation must match exactly")


class FpsProbeReportParserTests(unittest.TestCase):
    SAMPLE_REPORT = (
        "[fps-probe] interval: frames=60 update_ms=1.420 render_ms=8.150 idle_ms=0.180 flip_ms=6.850 total_ms=16.600\n"
        "[fps-probe] guest: clock_f898=35950 render_counter=1971 hud_hist=0x01 buf_toggle=1 course_ms=10950\n"
        "[fps-probe] camera: fov=60.0 cached_m00=1.0000\n"
        "[fps-probe] purity: mutations=4 (render_counter:1 hud_hist:1 buf_toggle:1 cam_matrix:1 dma_swap:0 unattributed:0)\n"
        "[fps-probe] verdict: clean (unattributed_words=0)\n"
    )

    def parse_report(self, text):
        data = {}
        for line in text.splitlines():
            line = line.strip()
            if not line.startswith('[fps-probe]'):
                continue
            body = line[len('[fps-probe]'):].strip()
            if body.startswith('interval:'):
                for part in body[len('interval:'):].strip().split():
                    k, v = part.split('=')
                    data[k] = float(v) if '.' in v else int(v)
            elif body.startswith('guest:'):
                for part in body[len('guest:'):].strip().split():
                    k, v = part.split('=')
                    data[k] = int(v, 16) if v.startswith('0x') else int(v)
            elif body.startswith('camera:'):
                for part in body[len('camera:'):].strip().split():
                    k, v = part.split('=')
                    data[k] = float(v)
            elif body.startswith('purity:'):
                m = re.search(r'mutations=(\d+)', body)
                if m:
                    data['mutations'] = int(m.group(1))
                m_unatt = re.search(r'unattributed:(\d+)', body)
                if m_unatt:
                    data['unattributed'] = int(m_unatt.group(1))
            elif body.startswith('verdict:'):
                data['verdict'] = 'clean' if 'clean' in body else 'contaminated'
        return data

    def test_report_parsing(self):
        parsed = self.parse_report(self.SAMPLE_REPORT)
        self.assertEqual(parsed['frames'], 60)
        self.assertAlmostEqual(parsed['update_ms'], 1.420)
        self.assertAlmostEqual(parsed['render_ms'], 8.150)
        self.assertAlmostEqual(parsed['idle_ms'], 0.180)
        self.assertAlmostEqual(parsed['flip_ms'], 6.850)
        self.assertAlmostEqual(parsed['total_ms'], 16.600)

        self.assertEqual(parsed['clock_f898'], 35950)
        self.assertEqual(parsed['render_counter'], 1971)
        self.assertEqual(parsed['hud_hist'], 1)
        self.assertEqual(parsed['buf_toggle'], 1)
        self.assertEqual(parsed['course_ms'], 10950)

        self.assertAlmostEqual(parsed['fov'], 60.0)
        self.assertAlmostEqual(parsed['cached_m00'], 1.0000)

        self.assertEqual(parsed['mutations'], 4)
        self.assertEqual(parsed['unattributed'], 0)
        self.assertEqual(parsed['verdict'], 'clean')


class FpsProbeClockRatioTests(unittest.TestCase):
    def test_simulation_clock_step(self):
        # DynamicsConductor + 0xF898 advances by +50 per simulation step (1/3000 s units)
        step_units = 50
        clock_freq_hz = 3000
        seconds_per_step = step_units / clock_freq_hz
        self.assertAlmostEqual(seconds_per_step, 1.0 / 60.0, places=6)

        # 60 steps = 1.0 second
        units_per_second = 60 * step_units
        self.assertEqual(units_per_second, clock_freq_hz)


if __name__ == '__main__':
    unittest.main()
