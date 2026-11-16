"""TASK-215: H-shifter, direct gear application, and clutch pedal gameplay mod.
Specification: docs/H-SHIFTER-CLUTCH-MOD-STUDY.md and docs/decisions/0073-mod-support-in-bundle.md.
"""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]


class HShifterModTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.header = (ROOT / 'native/hshifter_mod.hpp').read_text(encoding='utf-8')
        cls.source = (ROOT / 'native/hshifter_mod.cpp').read_text(encoding='utf-8')
        cls.guest_header = (ROOT / 'native/hshifter_guest.hpp').read_text(encoding='utf-8')
        cls.guest_source = (ROOT / 'native/hshifter_guest.cpp').read_text(encoding='utf-8')
        cls.wheel_header = (ROOT / 'native/wheel.hpp').read_text(encoding='utf-8')
        cls.wheel_source = (ROOT / 'native/wheel.cpp').read_text(encoding='utf-8')
        cls.window_source = (ROOT / 'native/window.cpp').read_text(encoding='utf-8')
        cls.display_aspect = (ROOT / 'native/display_aspect.cpp').read_text(encoding='utf-8')
        cls.cmake = (ROOT / 'native/CMakeLists.txt').read_text(encoding='utf-8')

    def test_switch_names_and_ini_keys(self):
        self.assertIn('kSwitch = "GT4_MOD_HSHIFTER"', self.header)
        self.assertIn('kIniKey = "mod_hshifter"', self.header)

    def test_mod_is_opt_in_off_by_default(self):
        # Decision 0073: gameplay mods are opt-in and off by default.
        match = re.search(r'if\s*\(\s*value\s*==\s*nullptr\s*\|\|\s*\*value\s*==\s*\'\\0\'\s*\|\|\s*std::strcmp\(value,\s*"0"\)\s*==\s*0\s*\)\s*\{\s*return\s+false;', self.source)
        self.assertIsNotNone(match, 'parse_switch must return false for nullptr, empty, and "0"')

    def test_gate_and_clutch_constants(self):
        self.assertIn('enum class Gate : std::uint8_t', self.header)
        self.assertIn('reverse = 0', self.header)
        self.assertIn('neutral = 1', self.header)
        self.assertIn('gear_1  = 2', self.header)
        self.assertIn('enum class ClutchState : std::uint8_t', self.header)
        self.assertIn('open     = 0', self.header)
        self.assertIn('engaged  = 1', self.header)
        self.assertIn('syncing  = 3', self.header)

    def test_safety_invariants(self):
        # Max reverse speed threshold (~1 km/h = 0.2778 m/s)
        self.assertIn('0.2778f', self.header)
        # Reverse lock check
        self.assertIn('speed_too_high_for_reverse', self.source)
        # Gear clamp check
        self.assertIn('gear_exceeds_car_max', self.source)
        # Clutch requirement
        self.assertIn('clutch_not_depressed', self.source)
        # Automatic transmission lockout
        self.assertIn('automatic_transmission_blocked', self.source)

    def test_guest_memory_offsets(self):
        # Host parameter block at 0x7F10..0x7F16
        self.assertIn('kHostBlockClutchState = 0x7F10', self.guest_header)
        self.assertIn('kHostBlockTargetGear  = 0x7F14', self.guest_header)
        self.assertIn('kHostBlockHudChar     = 0x7F16', self.guest_header)
        # Automobile offsets
        self.assertIn('kAutoGearOffset          = 0x624', self.guest_header)
        self.assertIn('kAutoClutchStateOffset   = 0x626', self.guest_header)
        self.assertIn('kAutoClutchEngageOffset  = 0x618', self.guest_header)
        self.assertIn('kAutoShiftStepOffset     = 0x567', self.guest_header)

    def test_cmake_fixture_registration(self):
        self.assertIn('hshifter_mod.cpp hshifter_guest.cpp', self.cmake)
        self.assertIn('add_executable(hshifter_mod_fixture hshifter_mod_fixture.cpp)', self.cmake)
        self.assertIn('add_test(NAME native_hshifter_mod COMMAND $<TARGET_FILE:hshifter_mod_fixture>)', self.cmake)

    def test_wheel_shifter_and_clutch_integration(self):
        # WheelConfig contains ShifterConfig
        self.assertIn('struct ShifterConfig', self.wheel_header)
        self.assertIn('ShifterConfig shifter', self.wheel_header)
        self.assertIn('evaluate_shifter_gate', self.wheel_header)
        # WheelInput exposes clutch pedal and gate getters
        self.assertIn('clutch_pedal() const', self.wheel_header)
        self.assertIn('shifter_gate() const', self.wheel_header)
        # wheel.cpp parses [shifter] table
        self.assertIn('current_section == "shifter"', self.wheel_source)
        self.assertIn('evaluate_shifter_gate', self.wheel_source)

    def test_window_vblank_integration(self):
        # window.cpp includes mod headers and checks GT4_MOD_HSHIFTER
        self.assertIn('#include "hshifter_mod.hpp"', self.window_source)
        self.assertIn('#include "hshifter_guest.hpp"', self.window_source)
        self.assertIn('GT4_MOD_HSHIFTER', self.window_source)
        # on_vblank runs read_guest_player_car and apply_hshifter_to_guest
        self.assertIn('read_guest_player_car', self.window_source)
        self.assertIn('apply_hshifter_to_guest', self.window_source)

    def test_display_aspect_hook_records_race_display(self):
        # display_aspect.cpp notifies hshifter of live RaceDisplay
        self.assertIn('gt4::hshifter::note_race_display(self)', self.display_aspect)


if __name__ == '__main__':
    unittest.main()
