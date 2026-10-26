"""Checks the derived SPU2 envelope fix against the pinned donor spu2.cpp (TASK-125, Issue #104).

Needs the private donor checkout: set GT4_ASSETS_ROOT to the checkout that holds
private/references/ps2-web-recomp (default: this repository). Skipped when it is absent.
"""

import importlib.util
import os
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ASSETS_ROOT = Path(os.environ.get('GT4_ASSETS_ROOT', ROOT)).resolve()
DONOR_SPU2 = ASSETS_ROOT / 'private/references/ps2-web-recomp/runtime/src/iop/spu2.cpp'

spec = importlib.util.spec_from_file_location('prepare_spu2_envelope', ROOT / 'scripts/prepare_spu2_envelope.py')
spu2_prep = importlib.util.module_from_spec(spec)
spec.loader.exec_module(spu2_prep)

try:
    import unittest
    TestCase = unittest.TestCase
    skip_unless = unittest.skipUnless
except ImportError:
    class TestCase:
        def assertIn(self, a, b):
            assert a in b, f"{a!r} not in {b!r}"
        def assertNotIn(self, a, b):
            assert a not in b, f"{a!r} in {b!r}"
        def assertEqual(self, a, b):
            assert a == b, f"{a!r} != {b!r}"
        class _AssertRaisesContext:
            def __init__(self, exc): self.exc = exc
            def __enter__(self): return self
            def __exit__(self, exc_type, exc_val, exc_tb):
                assert exc_type is not None and issubclass(exc_type, self.exc), f"Expected {self.exc}, got {exc_type}"
                return True
        def assertRaises(self, exc):
            return self._AssertRaisesContext(exc)
    def skip_unless(cond, reason):
        def dec(cls): return cls
        return dec


class SPU2EnvelopeUnitOverlayTest(TestCase):
    """Synthetic unit tests that run unconditionally without requiring private donor assets."""

    def test_synthetic_roundtrip(self):
        source = f"// header\n{spu2_prep.DONOR_STEP}\n// footer\n"
        derived = spu2_prep.apply_overlay(source)
        self.assertIn("if (delta == 0 && level > 0) delta = -1;", derived)
        restored = spu2_prep.remove_overlay(derived)
        self.assertEqual(restored, source)

    def test_missing_anchor_raises_error(self):
        with self.assertRaises(ValueError):
            spu2_prep.apply_overlay("// no anchor here\n")

    def test_output_must_stay_inside_private(self):
        with tempfile.TemporaryDirectory() as scratch:
            with self.assertRaises(ValueError):
                spu2_prep.prepare(Path(scratch) / 'out', ASSETS_ROOT)


@skip_unless(DONOR_SPU2.exists(), 'donor checkout with private/references is not present')
class SPU2EnvelopeDonorOverlayTest(TestCase):
    """Full donor integration tests against private/references/ps2-web-recomp."""

    def setUp(self):
        self.source = spu2_prep.read_donor(ASSETS_ROOT)
        self.derived = spu2_prep.apply_overlay(self.source)

    def test_overlay_reverses_to_the_pinned_donor_text(self):
        self.assertEqual(spu2_prep.remove_overlay(self.derived), self.source)

    def test_overlay_contains_underflow_guard(self):
        self.assertIn('if (delta == 0 && level > 0) delta = -1;', self.derived)

    def test_derived_source_has_no_crlf(self):
        self.assertNotIn('\r', self.derived)

    def test_prepare_writes_the_derived_file(self):
        output = ASSETS_ROOT / 'private' / 'slice270-spu2-test-scratch'
        try:
            spu2_prep.prepare(output, ASSETS_ROOT)
            self.assertEqual((output / 'spu2.cpp').read_text(encoding='utf-8'), self.derived)
        finally:
            for path in sorted(output.glob('*'), reverse=True):
                path.unlink()
            if output.exists():
                output.rmdir()


if __name__ == '__main__':
    try:
        import unittest
        unittest.main()
    except ImportError:
        print('Running synthetic tests without unittest framework...')
        unit = SPU2EnvelopeUnitOverlayTest()
        unit.test_synthetic_roundtrip()
        unit.test_missing_anchor_raises_error()
        unit.test_output_must_stay_inside_private()
        print('Synthetic tests: ALL PASS')
