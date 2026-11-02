"""Unit tests for PCSX2/gt4-gs texture replacement contract and scanner (TASK-050)."""

from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import texture_contract as tc


class TextureFilenameParsingTests(unittest.TestCase):
    def test_canonical_filename_with_clut(self):
        name = "0123456789abcdef-fedcba98.png"
        meta = tc.parse_texture_filename(name)
        self.assertTrue(meta.is_valid)
        self.assertIsNotNone(meta.key)
        self.assertEqual(meta.key.texture_hash, 0x0123456789ABCDEF)
        self.assertEqual(meta.key.clut_hash, 0xFEDCBA98)
        self.assertFalse(meta.is_mip)
        self.assertEqual(meta.extension, ".png")
        self.assertEqual(meta.key.canonical_stem, "0123456789abcdef-fedcba98")

    def test_filename_without_clut(self):
        name = "0000000012345678.dds"
        meta = tc.parse_texture_filename(name)
        self.assertTrue(meta.is_valid)
        self.assertIsNotNone(meta.key)
        self.assertEqual(meta.key.texture_hash, 0x12345678)
        self.assertIsNone(meta.key.clut_hash)
        self.assertFalse(meta.is_mip)
        self.assertEqual(meta.extension, ".dds")

    def test_mipmap_filename_flagged_as_mip(self):
        name = "0123456789abcdef-fedcba98-mip1.png"
        meta = tc.parse_texture_filename(name)
        self.assertTrue(meta.is_valid)
        self.assertTrue(meta.is_mip)
        self.assertEqual(meta.mip_level, 1)

    def test_round_trip_format_and_parse(self):
        tex_hash = 0xAABBCCDDEEFF0011
        clut_hash = 0x12345678
        formatted = tc.format_texture_filename(tex_hash, clut_hash=clut_hash, ext="png")
        self.assertEqual(formatted, "aabbccddeeff0011-12345678.png")

        meta = tc.parse_texture_filename(formatted)
        self.assertTrue(meta.is_valid)
        self.assertEqual(meta.key.texture_hash, tex_hash)
        self.assertEqual(meta.key.clut_hash, clut_hash)

    def test_zip_packs_are_refused(self):
        name = "SCUS-97328.zip"
        meta = tc.parse_texture_filename(name)
        self.assertFalse(meta.is_valid)
        self.assertIn("Zip packs are not supported", meta.error)

    def test_unsupported_extensions_refused(self):
        for bad_name in ("texture.jpg", "notes.txt", "data.bin"):
            meta = tc.parse_texture_filename(bad_name)
            self.assertFalse(meta.is_valid)
            self.assertIn("Unsupported texture extension", meta.error)

    def test_non_hex_filename_refused(self):
        meta = tc.parse_texture_filename("not_a_hex_hash.png")
        self.assertFalse(meta.is_valid)
        self.assertIn("does not match hex hash pattern", meta.error)


class DirectoryScanningAndDuplicateResolutionTests(unittest.TestCase):
    def test_scanner_resolves_duplicates_by_first_seen_rule(self):
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            dir_a = root / "01_high_priority"
            dir_b = root / "02_low_priority"
            dir_a.mkdir()
            dir_b.mkdir()

            tex_name = "0123456789abcdef-00000001.png"
            file_a = dir_a / tex_name
            file_b = dir_b / tex_name
            mip_file = dir_a / "0123456789abcdef-00000001-mip1.png"
            invalid_file = dir_a / "unsupported.txt"

            file_a.write_bytes(b"texture_a")
            file_b.write_bytes(b"texture_b")
            mip_file.write_bytes(b"mip_data")
            invalid_file.write_text("hello", encoding="utf-8")

            report = tc.scan_replacement_directory(root)
            self.assertEqual(report.total_files_examined, 4)
            self.assertEqual(len(report.valid_replacements), 1)

            key = tc.TextureKey(texture_hash=0x0123456789ABCDEF, clut_hash=1)
            self.assertIn(key, report.valid_replacements)
            # First encountered file (dir_a) must win
            self.assertEqual(report.valid_replacements[key], file_a)

            # Duplicate file_b recorded
            self.assertEqual(len(report.duplicates), 1)
            dup_key, dup_path, orig_path = report.duplicates[0]
            self.assertEqual(dup_key, key)
            self.assertEqual(dup_path, file_b)
            self.assertEqual(orig_path, file_a)

            # Mip file ignored
            self.assertEqual(len(report.ignored_mips), 1)
            self.assertEqual(report.ignored_mips[0], mip_file)

            # Invalid file recorded
            self.assertEqual(len(report.invalid_files), 1)
            self.assertEqual(report.invalid_files[0][0], invalid_file)


class SyntheticBenchmarkTests(unittest.TestCase):
    def test_benchmark_scaling_estimates(self):
        stats_streaming = tc.estimate_synthetic_pack_footprint(
            file_count=10000, precache=False
        )
        stats_precache = tc.estimate_synthetic_pack_footprint(
            file_count=10000, precache=True
        )

        self.assertEqual(stats_streaming["file_count"], 10000)
        self.assertGreater(stats_streaming["total_pack_size_mb"], 10000)
        # On-demand streaming keeps active host RAM low (~250-300 MB)
        self.assertLess(stats_streaming["host_ram_mb"], 500)
        # Precache loads the entire 10,000 files into RAM (>10 GB)
        self.assertGreater(stats_precache["host_ram_mb"], 10000)


if __name__ == "__main__":
    unittest.main()
