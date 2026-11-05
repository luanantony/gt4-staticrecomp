#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Unit tests for the GS Stream Primitive Interpolator & High-FPS Synthesizer (TASK-045)."""

import io
import struct
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "gs_interp"))
import gs_interp as interp  # noqa: E402


class SyntheticCaptureBuilder:
    """Helper to build test .gscap version 2 streams in memory."""

    def __init__(self, serial: str = "SCUS-97328"):
        self.stream = io.BytesIO()
        hdr = struct.pack("<8sHHHHII16sQ16x", interp.V2_MAGIC, 2, 1, 0, 64, 16, 0, serial.encode("ascii").ljust(16, b"\0"), 0)
        self.stream.write(hdr)
        self.seq = 1

    def add_record(self, kind: int, payload: bytes, flags: int = 0):
        hdr = interp.V2_RECORD_HEADER.pack(kind, flags, len(payload), self.seq)
        self.stream.write(hdr)
        self.stream.write(payload)
        padding = (-len(payload)) % 16
        if padding:
            self.stream.write(b"\0" * padding)
        self.seq += 1

    def add_vsync(self, frame_num: int, field_bit: int = 0):
        payload = struct.pack("<QB7x", frame_num, field_bit)
        self.add_record(interp.REC_VSYNC, payload)

    def add_gif_packed_triangles(self, vertices, tex0: int = 0x1000):
        # First A+D to set TEX0 and PRIM
        buf = bytearray()
        # Tag for A+D (1 QWORD)
        tag_ad_low = 1 | (1 << 15) | (0 << 58) | (1 << 60)
        tag_ad_high = interp.GS_REG_AD
        buf += struct.pack("<QQ", tag_ad_low, tag_ad_high)
        buf += struct.pack("<QQ", tex0, interp.GS_REG_TEX0_1)

        # Tag for Triangles
        nloop = len(vertices)
        tag_tri_low = (nloop & 0x7FFF) | (1 << 15) | (1 << 46) | (interp.PRIM_TRI << 47) | (0 << 58) | (3 << 60)
        tag_tri_high = interp.GS_REG_ST | (interp.GS_REG_RGBAQ << 4) | (interp.GS_REG_XYZF2 << 8)
        buf += struct.pack("<QQ", tag_tri_low, tag_tri_high)

        for v in vertices:
            # ST
            buf += struct.pack("<fff4x", v["s"], v["t"], 1.0)
            # RGBAQ
            buf += struct.pack("<BBBB12x", v["r"], v["g"], v["b"], v["a"])
            # XYZF2
            buf += struct.pack("<HHHHI4x", v["x"], 0, v["y"], 0, v["z"])

        payload_hdr = struct.pack("<B3xII4x", 1, 1, len(buf))
        self.add_record(interp.REC_GIF, payload_hdr + bytes(buf))

    def add_hud_sprite(self, x: int, y: int, w: int, h: int):
        buf = bytearray()
        # Tag for Sprite
        tag_sprite_low = (2 & 0x7FFF) | (1 << 15) | (1 << 46) | (interp.PRIM_SPRITE << 47) | (0 << 58) | (2 << 60)
        tag_sprite_high = interp.GS_REG_RGBAQ | (interp.GS_REG_XYZ2 << 4)
        buf += struct.pack("<QQ", tag_sprite_low, tag_sprite_high)

        # 2 vertices defining sprite rectangle (Z = 0)
        buf += struct.pack("<BBBB12x", 255, 255, 255, 128)
        buf += struct.pack("<HHHHI4x", x, 0, y, 0, 0)
        buf += struct.pack("<BBBB12x", 255, 255, 255, 128)
        buf += struct.pack("<HHHHI4x", x + w, 0, y + h, 0, 0)

        payload_hdr = struct.pack("<B3xII4x", 1, 1, len(buf))
        self.add_record(interp.REC_GIF, payload_hdr + bytes(buf))

    def getvalue(self) -> bytes:
        return self.stream.getvalue()


class GsInterpTests(unittest.TestCase):
    def test_bad_magic_is_rejected(self):
        bad_stream = io.BytesIO(b"NOT_A_VALID_GSCAP_HEADER_1234567890123456789012345678901234567890")
        with self.assertRaises(ValueError):
            list(interp.parse_v2_records(bad_stream))

    def test_draw_batch_parsing_and_hud_detection(self):
        builder = SyntheticCaptureBuilder()
        # Add 3D triangles
        tri_verts = [
            {"x": 100, "y": 200, "z": 5000, "s": 0.0, "t": 0.0, "r": 255, "g": 0, "b": 0, "a": 255},
            {"x": 150, "y": 250, "z": 5200, "s": 1.0, "t": 0.0, "r": 0, "g": 255, "b": 0, "a": 255},
            {"x": 120, "y": 280, "z": 5100, "s": 0.5, "t": 1.0, "r": 0, "g": 0, "b": 255, "a": 255},
        ]
        builder.add_gif_packed_triangles(tri_verts)
        # Add HUD sprite
        builder.add_hud_sprite(50, 50, 100, 20)
        builder.add_vsync(0)

        data = builder.getvalue()
        frames = interp.parse_frames_from_records(interp.parse_v2_records(io.BytesIO(data)))
        self.assertEqual(len(frames), 1)
        batches = frames[0].draw_batches
        self.assertEqual(len(batches), 2)

        # Batch 0: 3D Triangles
        self.assertEqual(batches[0].prim_type, interp.PRIM_TRI)
        self.assertFalse(batches[0].is_hud)
        self.assertEqual(len(batches[0].vertices), 3)
        self.assertEqual(batches[0].vertices[0].x, 100)
        self.assertEqual(batches[0].vertices[0].z, 5000)

        # Batch 1: 2D HUD Sprite
        self.assertEqual(batches[1].prim_type, interp.PRIM_SPRITE)
        self.assertTrue(batches[1].is_hud)
        self.assertEqual(len(batches[1].vertices), 2)
        self.assertEqual(batches[1].vertices[0].z, 0)

    def test_linear_interpolation_midpoint_accuracy(self):
        batch_a = interp.DrawBatch(
            prim_type=interp.PRIM_TRI,
            context_reg=0,
            frame_buffer=0,
            z_buffer=0,
            tex0=0x1000,
            test=0,
            is_hud=False,
            vertices=[
                interp.Vertex(x=100, y=200, z=1000, rgba=(100, 100, 100, 255), st=(0.0, 0.0, 1.0)),
                interp.Vertex(x=300, y=400, z=2000, rgba=(200, 200, 200, 255), st=(1.0, 1.0, 1.0)),
            ]
        )
        batch_b = interp.DrawBatch(
            prim_type=interp.PRIM_TRI,
            context_reg=0,
            frame_buffer=0,
            z_buffer=0,
            tex0=0x1000,
            test=0,
            is_hud=False,
            vertices=[
                interp.Vertex(x=200, y=400, z=3000, rgba=(150, 150, 150, 255), st=(0.5, 0.5, 1.0)),
                interp.Vertex(x=500, y=600, z=4000, rgba=(250, 250, 250, 255), st=(1.0, 1.0, 1.0)),
            ]
        )

        mid = interp.interpolate_batch(batch_a, batch_b, t=0.5)
        self.assertEqual(len(mid.vertices), 2)

        # Midpoint of (100, 200, 1000) and (200, 400, 3000) -> (150, 300, 2000)
        self.assertEqual(mid.vertices[0].x, 150)
        self.assertEqual(mid.vertices[0].y, 300)
        self.assertEqual(mid.vertices[0].z, 2000)
        self.assertEqual(mid.vertices[0].rgba, (125, 125, 125, 255))
        self.assertAlmostEqual(mid.vertices[0].st[0], 0.25)
        self.assertAlmostEqual(mid.vertices[0].st[1], 0.25)

        # Midpoint of (300, 400, 2000) and (500, 600, 4000) -> (400, 500, 3000)
        self.assertEqual(mid.vertices[1].x, 400)
        self.assertEqual(mid.vertices[1].y, 500)
        self.assertEqual(mid.vertices[1].z, 3000)
        self.assertEqual(mid.vertices[1].rgba, (225, 225, 225, 255))

    def test_hud_elements_stay_unmodified(self):
        hud_batch = interp.DrawBatch(
            prim_type=interp.PRIM_SPRITE,
            context_reg=0,
            frame_buffer=0,
            z_buffer=0,
            tex0=0,
            test=0,
            is_hud=True,
            vertices=[
                interp.Vertex(x=50, y=50, z=0, rgba=(255, 255, 255, 255)),
                interp.Vertex(x=150, y=70, z=0, rgba=(255, 255, 255, 255)),
            ]
        )
        res = interp.interpolate_batch(hud_batch, hud_batch, t=0.5)
        self.assertTrue(res.is_hud)
        self.assertEqual(res.vertices[0].x, 50)
        self.assertEqual(res.vertices[1].x, 150)

    def test_synthesize_aux_frame_record_headers(self):
        builder = SyntheticCaptureBuilder()
        v_a = [
            {"x": 100, "y": 100, "z": 1000, "s": 0.0, "t": 0.0, "r": 200, "g": 200, "b": 200, "a": 255},
            {"x": 200, "y": 100, "z": 1000, "s": 1.0, "t": 0.0, "r": 200, "g": 200, "b": 200, "a": 255},
            {"x": 150, "y": 200, "z": 1000, "s": 0.5, "t": 1.0, "r": 200, "g": 200, "b": 200, "a": 255},
        ]
        builder.add_gif_packed_triangles(v_a)
        builder.add_vsync(0)

        v_b = [
            {"x": 110, "y": 105, "z": 1005, "s": 0.0, "t": 0.0, "r": 200, "g": 200, "b": 200, "a": 255},
            {"x": 210, "y": 105, "z": 1005, "s": 1.0, "t": 0.0, "r": 200, "g": 200, "b": 200, "a": 255},
            {"x": 160, "y": 205, "z": 1005, "s": 0.5, "t": 1.0, "r": 200, "g": 200, "b": 200, "a": 255},
        ]
        builder.add_gif_packed_triangles(v_b)
        builder.add_vsync(1)

        data = builder.getvalue()
        frames = interp.parse_frames_from_records(interp.parse_v2_records(io.BytesIO(data)))
        self.assertEqual(len(frames), 2)

        aux_records, stats = interp.synthesize_aux_frame(frames[0], frames[1], t=0.5)
        self.assertTrue(len(aux_records) >= 3)
        # Check first is AUX_FRAME_BEGIN
        self.assertEqual(aux_records[0].kind, interp.REC_AUX_FRAME_BEGIN)
        base_f, t_val, _ = struct.unpack("<QfI", aux_records[0].payload)
        self.assertEqual(base_f, 0)
        self.assertAlmostEqual(t_val, 0.5)

        # Check last is AUX_FRAME_END
        self.assertEqual(aux_records[-1].kind, interp.REC_AUX_FRAME_END)
        end_base_f, _, _ = struct.unpack("<QII", aux_records[-1].payload)
        self.assertEqual(end_base_f, 0)

        # Check metrics and gate
        self.assertEqual(stats["matched_primitives"], 3.0)
        self.assertEqual(stats["match_rate_pct"], 100.0)
        self.assertEqual(stats["gate_passed"], 1.0)

    def test_full_pipeline_multi_rate_120_and_144(self):
        builder = SyntheticCaptureBuilder()
        for f in range(3):
            verts = [
                {"x": 100 + f * 10, "y": 100, "z": 1000, "s": 0.0, "t": 0.0, "r": 100, "g": 100, "b": 100, "a": 255},
                {"x": 200 + f * 10, "y": 100, "z": 1000, "s": 1.0, "t": 0.0, "r": 100, "g": 100, "b": 100, "a": 255},
                {"x": 150 + f * 10, "y": 200, "z": 1000, "s": 0.5, "t": 1.0, "r": 100, "g": 100, "b": 100, "a": 255},
            ]
            builder.add_gif_packed_triangles(verts)
            builder.add_vsync(f)

        raw_in = builder.getvalue()

        # Test 120 Hz synthesis
        out_120 = io.BytesIO()
        res_120 = interp.process_gscap_interpolation(io.BytesIO(raw_in), out_120, target_rate=120)
        self.assertEqual(res_120["frames_in"], 3)
        self.assertEqual(res_120["aux_frames_generated"], 2)  # Between 0-1 and 1-2
        self.assertTrue(res_120["match_gate_passed"])
        self.assertEqual(res_120["overall_match_rate_pct"], 100.0)

        # Verify output stream is readable and contains synthesized aux records
        out_records = list(interp.parse_v2_records(io.BytesIO(out_120.getvalue())))
        aux_begins = [r for r in out_records if r.kind == interp.REC_AUX_FRAME_BEGIN]
        aux_ends = [r for r in out_records if r.kind == interp.REC_AUX_FRAME_END]
        self.assertEqual(len(aux_begins), 2)
        self.assertEqual(len(aux_ends), 2)

        # Test 144 Hz synthesis (2 aux frames per guest interval)
        out_144 = io.BytesIO()
        res_144 = interp.process_gscap_interpolation(io.BytesIO(raw_in), out_144, target_rate=144)
        self.assertEqual(res_144["aux_frames_generated"], 4)
        self.assertTrue(res_144["match_gate_passed"])


if __name__ == "__main__":
    unittest.main()
