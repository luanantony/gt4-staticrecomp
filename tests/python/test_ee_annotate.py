"""Unit tests for EE decompilation name annotation and symbol resolution (TASK-020)."""

from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import ee_annotate as annotate


class SymbolResolutionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.names = annotate.load_name_map(ROOT / 'docs' / 'data' / 'ee-names.csv')

    def test_load_name_map_contains_both_int_and_hex_keys(self):
        self.assertIn('0x001010E0', self.names)
        self.assertIn(0x001010E0, self.names)
        entry = self.names['0x001010E0']
        self.assertEqual(entry['address'], '0x001010E0')
        self.assertEqual(entry['quality'], 'named')
        # Ensure named 'GranTurismo4__GameObjectPS2__start' was selected over placeholder 'CarIconMaker__virtual_01'
        self.assertEqual(entry['primary'], 'GranTurismo4__GameObjectPS2__start')
        self.assertIn('CarIconMaker__virtual_01', entry['aliases'])

    def test_lookup_symbol_formats_varied_address_representations(self):
        res1 = annotate.lookup_symbol('0x001010e0', name_map=self.names)
        res2 = annotate.lookup_symbol('001010E0', name_map=self.names)
        res3 = annotate.lookup_symbol(0x1010E0, name_map=self.names)
        self.assertIsNotNone(res1)
        self.assertEqual(res1, res2)
        self.assertEqual(res1, res3)

    def test_lookup_unknown_address_returns_none(self):
        res = annotate.lookup_symbol('0x00000000', name_map=self.names)
        self.assertIsNone(res)

    def test_adhoc_symbols_resolved(self):
        # 0x0011EDB8 is MLoggerControl::setTotalTime
        res = annotate.lookup_symbol('0x0011EDB8', name_map=self.names)
        self.assertIsNotNone(res)
        self.assertEqual(res['origin'], 'adhoc')
        self.assertEqual(res['primary'], 'MLoggerControl::setTotalTime')


class SourceAnnotationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.names = annotate.load_name_map(ROOT / 'docs' / 'data' / 'ee-names.csv')

    def test_annotate_case_label_and_goto_target(self):
        sample = (
            "void unit_0040(Cpu& cpu, uint32_t entry, uint32_t depth) {\n"
            "    switch (entry) {\n"
            "        case 1u: goto L_001010E0;\n"
            "        case 2u: goto L_00999999;\n"
            "    }\n"
            "L_001010E0:\n"
            "    cpu.r[1] = 0;\n"
            "L_00999999:\n"
            "    return;\n"
            "}\n"
        )
        annotated = annotate.annotate_source(sample, self.names)
        self.assertIn("        case 1u: goto L_001010E0; // GranTurismo4__GameObjectPS2__start\n", annotated)
        self.assertIn("L_001010E0: // GranTurismo4__GameObjectPS2__start\n", annotated)
        # Unmapped address 0x00999999 remains unannotated
        self.assertIn("        case 2u: goto L_00999999;\n", annotated)
        self.assertIn("L_00999999:\n", annotated)

    def test_round_trip_comment_stripping_invariance_lf(self):
        sample = (
            "void unit_0040(Cpu& cpu, uint32_t entry, uint32_t depth) {\n"
            "    switch (entry) {\n"
            "        case 1u: goto L_001010E0;\n"
            "    }\n"
            "L_001010E0:\n"
            "    return;\n"
            "}\n"
        )
        annotated = annotate.annotate_source(sample, self.names)
        self.assertNotEqual(annotated, sample)
        stripped = annotate.strip_annotations(annotated)
        self.assertEqual(stripped, sample)

    def test_round_trip_comment_stripping_invariance_crlf(self):
        sample = (
            "void unit_0040(Cpu& cpu, uint32_t entry, uint32_t depth) {\r\n"
            "    switch (entry) {\r\n"
            "        case 1u: goto L_001010E0;\r\n"
            "    }\r\n"
            "L_001010E0:\r\n"
            "    return;\r\n"
            "}\r\n"
        )
        annotated = annotate.annotate_source(sample, self.names)
        self.assertIn("\r\n", annotated)
        self.assertNotIn("\r\r\n", annotated)
        stripped = annotate.strip_annotations(annotated)
        self.assertEqual(stripped, sample)

    def test_annotation_idempotency(self):
        sample = "        case 1u: goto L_001010E0;\n"
        once = annotate.annotate_source(sample, self.names)
        twice = annotate.annotate_source(once, self.names)
        self.assertEqual(once, twice)
        self.assertEqual(once.count("//"), 1)


if __name__ == '__main__':
    unittest.main()
