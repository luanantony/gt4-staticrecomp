#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""GS Stream Primitive Interpolation Oracle & Offline High-FPS Synthesizer (TASK-045).

Analyzes PlayStation 2 Graphic Synthesizer (GS) primitive streams (.gscap version 2)
between consecutive 60 Hz guest simulation frames N and N+1. Matches 3D primitives,
linearly interpolates geometry (X, Y, Z), UV/ST coordinates, and vertex colors (RGBA)
at normalized presentation time t in (0, 1), and synthesizes auxiliary presentation
frames wrapped in aux_frame_begin / aux_frame_end records without altering 60 Hz guest
simulation timing or physics state.

2D HUD, text, speedometer and lap timers are detected (orthographic / sprite signatures)
and passed through without spatial interpolation to eliminate text ghosting and jitter.
"""

from __future__ import annotations

import argparse
import io
import json
import math
import os
import struct
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import BinaryIO, Dict, Iterator, List, Optional, Tuple, Union

# Protocol constants from native/gs_stream/gs_stream_protocol.h
V2_MAGIC = b"GSSTREAM"
V2_FILE_HEADER = struct.Struct("<8sHHHHII16sQ16x")
V2_RECORD_HEADER = struct.Struct("<HHIQ")

REC_WRAP = 0
REC_GIF = 1
REC_PRIV = 2
REC_VSYNC = 3
REC_READ_FIFO = 4
REC_RESET = 5
REC_CONTROL = 6
REC_STATE_SAVE = 7
REC_STATE_LOAD = 8
REC_AUX_FRAME_BEGIN = 9
REC_AUX_FRAME_END = 10
REC_OSD_STATS = 11

# GS Register Addresses (low 8 bits for A+D)
GS_REG_PRIM = 0x00
GS_REG_RGBAQ = 0x01
GS_REG_ST = 0x02
GS_REG_UV = 0x03
GS_REG_XYZF2 = 0x04
GS_REG_XYZ2 = 0x05
GS_REG_TEX0_1 = 0x06
GS_REG_TEX0_2 = 0x07
GS_REG_CLAMP_1 = 0x08
GS_REG_CLAMP_2 = 0x09
GS_REG_FOG = 0x0A
GS_REG_XYZF3 = 0x0C
GS_REG_XYZ3 = 0x0D
GS_REG_AD = 0x0E
GS_REG_NOP = 0x0F

GS_REG_FRAME_1 = 0x4C
GS_REG_FRAME_2 = 0x4D
GS_REG_ZBUF_1 = 0x4E
GS_REG_ZBUF_2 = 0x4F
GS_REG_TEST_1 = 0x47
GS_REG_TEST_2 = 0x48

# Primitive types
PRIM_POINT = 0
PRIM_LINE = 1
PRIM_LINESTRIP = 2
PRIM_TRI = 3
PRIM_TRISTRIP = 4
PRIM_TRIFAN = 5
PRIM_SPRITE = 6


@dataclass
class Vertex:
    x: int = 0          # 16-bit fixed point (12.4)
    y: int = 0          # 16-bit fixed point (12.4)
    z: int = 0          # 24-bit or 32-bit depth
    f: int = 0          # fog / adc
    rgba: Tuple[int, int, int, int] = (128, 128, 128, 128)
    st: Tuple[float, float, float] = (0.0, 0.0, 1.0)
    uv: Tuple[int, int] = (0, 0)
    has_st: bool = False
    has_uv: bool = False
    has_rgba: bool = False


@dataclass
class DrawBatch:
    prim_type: int
    context_reg: int
    frame_buffer: int
    z_buffer: int
    tex0: int
    test: int
    is_hud: bool
    vertices: List[Vertex] = field(default_factory=list)
    raw_tag: bytes = b""


@dataclass
class StreamRecord:
    kind: int
    flags: int = 0
    payload: bytes = b""
    sequence: int = 0


@dataclass
class Frame:
    frame_index: int
    field_bit: int = 0
    records: List[StreamRecord] = field(default_factory=list)
    draw_batches: List[DrawBatch] = field(default_factory=list)


def parse_v2_records(stream: BinaryIO) -> Iterator[StreamRecord]:
    """Reads all version 2 records from a stream."""
    head = stream.read(64)
    if len(head) < 64:
        return
    if head[:8] != V2_MAGIC:
        raise ValueError("Not a version 2 .gscap file (missing GSSTREAM magic)")

    while True:
        hdr = stream.read(16)
        if not hdr:
            break
        if len(hdr) < 16:
            raise ValueError("Truncated record header")
        kind, flags, payload_bytes, seq = V2_RECORD_HEADER.unpack(hdr)
        padded = (payload_bytes + 15) & ~15
        payload = stream.read(padded)[:payload_bytes]
        yield StreamRecord(kind=kind, flags=flags, payload=payload, sequence=seq)


def parse_frames_from_records(records: Iterator[StreamRecord]) -> List[Frame]:
    """Groups stream records into frames delimited by VSYNC."""
    frames: List[Frame] = []
    current_records: List[StreamRecord] = []
    current_batches: List[DrawBatch] = []

    # GS state tracking
    current_frame_buf = 0
    current_zbuf = 0
    current_tex0 = 0
    current_test = 0
    current_prim = 0

    frame_index = 0

    for rec in records:
        current_records.append(rec)

        if rec.kind == REC_PRIV:
            # Privileged write: address, value
            pass

        elif rec.kind == REC_GIF:
            # GIF record payload: u8 path, u8 reserved[3], u32 flags, u32 bytes, u32 reserved1, then data
            if len(rec.payload) >= 16:
                path, _, gif_bytes = struct.unpack_from("<B3xII", rec.payload, 0)
                gif_data = rec.payload[16:16 + gif_bytes]

                # Parse GIFTAGs and draw batches
                offset = 0
                while offset + 16 <= len(gif_data):
                    low, high = struct.unpack_from("<QQ", gif_data, offset)
                    offset += 16
                    nloop = low & 0x7FFF
                    pre = (low >> 46) & 1
                    prim_field = (low >> 47) & 0x7FF
                    flg = (low >> 58) & 3
                    nreg = (low >> 60) & 0xF
                    if nreg == 0:
                        nreg = 16

                    if pre:
                        current_prim = prim_field & 0x7

                    regs = [(high >> (4 * i)) & 0xF for i in range(nreg)]

                    if flg == 0:  # PACKED
                        batch_vertices: List[Vertex] = []
                        is_ad_only = all(r == GS_REG_AD for r in regs)

                        for _ in range(nloop):
                            v = Vertex()
                            for r in regs:
                                if offset + 16 > len(gif_data):
                                    break
                                qdata = gif_data[offset:offset + 16]
                                offset += 16

                                if r == GS_REG_AD:
                                    d_val, a_val = struct.unpack_from("<QQ", qdata, 0)
                                    reg_addr = a_val & 0xFF
                                    if reg_addr == GS_REG_FRAME_1:
                                        current_frame_buf = d_val
                                    elif reg_addr == GS_REG_ZBUF_1:
                                        current_zbuf = d_val
                                    elif reg_addr == GS_REG_TEX0_1:
                                        current_tex0 = d_val
                                    elif reg_addr == GS_REG_TEST_1:
                                        current_test = d_val
                                    elif reg_addr == GS_REG_PRIM:
                                        current_prim = d_val & 0x7
                                elif r == GS_REG_PRIM:
                                    current_prim = qdata[0] & 0x7
                                elif r == GS_REG_RGBAQ:
                                    v.rgba = (qdata[0], qdata[4], qdata[8], qdata[12])
                                    v.has_rgba = True
                                elif r == GS_REG_ST:
                                    s, t, q = struct.unpack_from("<fff", qdata, 0)
                                    v.st = (s, t, q)
                                    v.has_st = True
                                elif r == GS_REG_UV:
                                    u, v_coord = struct.unpack_from("<HH", qdata, 0)
                                    v.uv = (u, v_coord)
                                    v.has_uv = True
                                elif r in (GS_REG_XYZF2, GS_REG_XYZ2, GS_REG_XYZF3, GS_REG_XYZ3):
                                    vx = struct.unpack_from("<H", qdata, 0)[0]
                                    vy = struct.unpack_from("<H", qdata, 4)[0]
                                    vz = struct.unpack_from("<I", qdata, 8)[0]
                                    vf = qdata[12]
                                    v.x, v.y, v.z, v.f = vx, vy, vz, vf
                                    batch_vertices.append(v)
                                    v = Vertex()

                        if batch_vertices and not is_ad_only:
                            # HUD detection: SPRITE primitive or constant Z across vertices with no depth
                            z_vals = [v.z for v in batch_vertices]
                            is_sprite = (current_prim == PRIM_SPRITE)
                            constant_z = (len(z_vals) > 0 and max(z_vals) == min(z_vals) and min(z_vals) == 0)
                            is_hud = is_sprite or constant_z

                            batch = DrawBatch(
                                prim_type=current_prim,
                                context_reg=0,
                                frame_buffer=current_frame_buf,
                                z_buffer=current_zbuf,
                                tex0=current_tex0,
                                test=current_test,
                                is_hud=is_hud,
                                vertices=batch_vertices,
                                raw_tag=gif_data[offset - (nloop * nreg + 1) * 16:offset]
                            )
                            current_batches.append(batch)

                    elif flg == 2:  # IMAGE
                        # Skip image data QWORDs
                        offset += nloop * 16
                    else:
                        offset += nloop * nreg * 16

        elif rec.kind == REC_VSYNC:
            field_bit = 0
            if len(rec.payload) >= 9:
                _, field_bit = struct.unpack_from("<QB", rec.payload, 0)
            frame = Frame(
                frame_index=frame_index,
                field_bit=field_bit,
                records=current_records,
                draw_batches=current_batches
            )
            frames.append(frame)
            current_records = []
            current_batches = []
            frame_index += 1

    if current_records:
        frames.append(Frame(frame_index=frame_index, records=current_records, draw_batches=current_batches))

    return frames


def lerp(a: float, b: float, t: float) -> float:
    return (1.0 - t) * a + t * b


def lerp_int(a: int, b: int, t: float) -> int:
    return int(round((1.0 - t) * a + t * b))


def interpolate_batch(batch_a: DrawBatch, batch_b: DrawBatch, t: float) -> DrawBatch:
    """Linearly interpolates 3D vertices between two matching draw batches."""
    if batch_a.is_hud:
        # HUD / OSD element: do not interpolate spatially to avoid blurring / jitter
        return batch_a

    interp_verts: List[Vertex] = []
    count = min(len(batch_a.vertices), len(batch_b.vertices))

    for i in range(count):
        va = batch_a.vertices[i]
        vb = batch_b.vertices[i]

        vx = lerp_int(va.x, vb.x, t)
        vy = lerp_int(va.y, vb.y, t)
        vz = lerp_int(va.z, vb.z, t)
        vf = va.f  # keep flag / fog index

        # RGBA lerp
        r = lerp_int(va.rgba[0], vb.rgba[0], t)
        g = lerp_int(va.rgba[1], vb.rgba[1], t)
        b = lerp_int(va.rgba[2], vb.rgba[2], t)
        a = lerp_int(va.rgba[3], vb.rgba[3], t)

        # ST lerp
        s = lerp(va.st[0], vb.st[0], t)
        t_coord = lerp(va.st[1], vb.st[1], t)
        q = lerp(va.st[2], vb.st[2], t)

        u = lerp_int(va.uv[0], vb.uv[0], t)
        v_coord = lerp_int(va.uv[1], vb.uv[1], t)

        interp_verts.append(Vertex(
            x=vx, y=vy, z=vz, f=vf,
            rgba=(r, g, b, a),
            st=(s, t_coord, q),
            uv=(u, v_coord),
            has_st=va.has_st,
            has_uv=va.has_uv,
            has_rgba=va.has_rgba
        ))

    return DrawBatch(
        prim_type=batch_a.prim_type,
        context_reg=batch_a.context_reg,
        frame_buffer=batch_a.frame_buffer,
        z_buffer=batch_a.z_buffer,
        tex0=batch_a.tex0,
        test=batch_a.test,
        is_hud=False,
        vertices=interp_verts,
        raw_tag=batch_a.raw_tag
    )


def build_gif_from_batch(batch: DrawBatch) -> bytes:
    """Constructs a raw GIF packet from a draw batch."""
    if not batch.vertices:
        return b""

    # Use standard 3-register PACKED format: ST (0x02), RGBAQ (0x01), XYZF2 (0x04)
    nloop = len(batch.vertices)
    pre = 1
    prim = batch.prim_type & 0x7
    flg = 0  # PACKED
    nreg = 3
    regs = [GS_REG_ST, GS_REG_RGBAQ, GS_REG_XYZF2]

    # Tag low: nloop (15), eop (1), pre (1 at 46), prim (11 at 47), flg (2 at 58), nreg (4 at 60)
    low = (nloop & 0x7FFF) | (1 << 15) | (pre << 46) | ((prim & 0x7FF) << 47) | ((flg & 3) << 58) | ((nreg & 0xF) << 60)
    high = regs[0] | (regs[1] << 4) | (regs[2] << 8)

    buf = bytearray()
    buf += struct.pack("<QQ", low, high)

    for v in batch.vertices:
        # ST QWORD
        buf += struct.pack("<fff4x", v.st[0], v.st[1], v.st[2])
        # RGBAQ QWORD
        buf += struct.pack("<BBBB12x", v.rgba[0], v.rgba[1], v.rgba[2], v.rgba[3])
        # XYZF2 QWORD: x (u16), 0 (u16), y (u16), 0 (u16), z (u32), f (u8), 0 (3 bytes)
        buf += struct.pack("<HHHHI4x", v.x, 0, v.y, 0, v.z)

    return bytes(buf)


def synthesize_aux_frame(frame_a: Frame, frame_b: Frame, t: float = 0.5) -> Tuple[List[StreamRecord], Dict[str, float]]:
    """Synthesizes an intermediate presentation frame between frame_a and frame_b at parameter t."""
    aux_records: List[StreamRecord] = []

    # 1. Begin auxiliary frame record (type 9)
    # Payload: base_frame (u64), t (float), flags (u32)
    begin_payload = struct.pack("<QfI", frame_a.frame_index, float(t), 0)
    aux_records.append(StreamRecord(kind=REC_AUX_FRAME_BEGIN, flags=0, payload=begin_payload))

    # Match batches between Frame A and Frame B
    batches_a = frame_a.draw_batches
    batches_b = frame_b.draw_batches

    matched_count = 0
    total_prims_a = sum(len(b.vertices) for b in batches_a)
    total_prims_b = sum(len(b.vertices) for b in batches_b)
    matched_prims = 0
    hud_batches_count = 0

    interp_batches: List[DrawBatch] = []

    # Match batches in order by context signature: (prim_type, frame_buffer, tex0)
    b_idx = 0
    for b_a in batches_a:
        if b_a.is_hud:
            hud_batches_count += 1
            interp_batches.append(b_a)
            matched_prims += len(b_a.vertices)
            continue

        matched_b = None
        while b_idx < len(batches_b):
            cand = batches_b[b_idx]
            b_idx += 1
            if cand.prim_type == b_a.prim_type and cand.tex0 == b_a.tex0:
                matched_b = cand
                break

        if matched_b is not None:
            matched_count += 1
            interp = interpolate_batch(b_a, matched_b, t)
            interp_batches.append(interp)
            matched_prims += len(interp.vertices)
        else:
            # Pass through original if no match
            interp_batches.append(b_a)

    denom = max(total_prims_a, total_prims_b, 1)
    match_rate_pct = min(100.0, (matched_prims / float(denom)) * 100.0)

    # Convert interpolated batches into GIF records
    for b in interp_batches:
        gif_bytes = build_gif_from_batch(b)
        if gif_bytes:
            # GIF record payload header: path (1 = PATH1 VU1), flags, size
            payload_hdr = struct.pack("<B3xII4x", 1, 1, len(gif_bytes))
            aux_records.append(StreamRecord(kind=REC_GIF, flags=0, payload=payload_hdr + gif_bytes))

    # 2. End auxiliary frame record (type 10)
    # Payload: base_frame (u64), flags (u32), reserved (u32)
    end_payload = struct.pack("<QII", frame_a.frame_index, 0, 0)
    aux_records.append(StreamRecord(kind=REC_AUX_FRAME_END, flags=0, payload=end_payload))

    stats = {
        "base_frame": float(frame_a.frame_index),
        "target_t": float(t),
        "total_prims_a": float(total_prims_a),
        "total_prims_b": float(total_prims_b),
        "matched_primitives": float(matched_prims),
        "match_rate_pct": float(match_rate_pct),
        "hud_batches": float(hud_batches_count),
        "gate_passed": 1.0 if match_rate_pct >= 95.0 else 0.0
    }

    return aux_records, stats


def write_v2_stream(destination: BinaryIO, records: Iterator[StreamRecord], serial: str = "SCUS-97328"):
    """Writes records into a version 2 .gscap file."""
    # Write 64-byte file header
    header = struct.pack("<8sHHHHII16sQ16x", V2_MAGIC, 2, 1, 0, 64, 16, 0, serial.encode("ascii")[:16].ljust(16, b"\0"), 0)
    destination.write(header)

    seq = 1
    for rec in records:
        payload_bytes = len(rec.payload)
        hdr = V2_RECORD_HEADER.pack(rec.kind, rec.flags, payload_bytes, seq)
        destination.write(hdr)
        destination.write(rec.payload)
        padding = (-payload_bytes) % 16
        if padding:
            destination.write(b"\0" * padding)
        seq += 1


def process_gscap_interpolation(input_stream: BinaryIO, output_stream: Optional[BinaryIO] = None, target_rate: int = 120) -> Dict[str, Union[float, int, bool, List[Dict[str, float]]]]:
    """Processes an entire .gscap stream, synthesizing high-FPS intermediate frames."""
    records = list(parse_v2_records(input_stream))
    frames = parse_frames_from_records(iter(records))

    stats_list: List[Dict[str, float]] = []
    output_records: List[StreamRecord] = []

    total_matched = 0
    total_prims = 0

    for i in range(len(frames)):
        frame = frames[i]
        # Emit Frame N
        output_records.extend(frame.records)

        # If next frame exists, synthesize intermediate auxiliary frame(s)
        if i + 1 < len(frames):
            next_frame = frames[i + 1]
            if target_rate == 120:
                aux_recs, s = synthesize_aux_frame(frame, next_frame, t=0.5)
                output_records.extend(aux_recs)
                stats_list.append(s)
                total_matched += int(s["matched_primitives"])
                total_prims += int(s["total_prims_a"])
            elif target_rate == 144:
                # 144 Hz has fractional steps (2.4 subframes per guest frame)
                for t in (0.4167, 0.8333):
                    aux_recs, s = synthesize_aux_frame(frame, next_frame, t=t)
                    output_records.extend(aux_recs)
                    stats_list.append(s)
                    total_matched += int(s["matched_primitives"])
                    total_prims += int(s["total_prims_a"])

    if output_stream is not None:
        write_v2_stream(output_stream, iter(output_records))

    overall_match_rate = (total_matched / float(max(total_prims, 1))) * 100.0 if total_prims > 0 else 100.0

    return {
        "frames_in": len(frames),
        "aux_frames_generated": len(stats_list),
        "target_presentation_rate": target_rate,
        "overall_match_rate_pct": overall_match_rate,
        "match_gate_passed": overall_match_rate >= 95.0,
        "frame_metrics": stats_list
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="GS Stream Primitive Interpolator & High-FPS Synthesizer (TASK-045)")
    parser.add_argument("input", help="Path to input .gscap file")
    parser.add_argument("--output", "-o", help="Path to output interpolated .gscap file")
    parser.add_argument("--rate", type=int, choices=[60, 120, 144, 240], default=120, help="Target presentation rate")
    parser.add_argument("--stats-json", help="Path to output analysis report JSON")

    args = parser.parse_args()
    input_path = Path(args.input)
    if not input_path.exists():
        sys.stderr.write(f"Error: input file {input_path} does not exist\n")
        return 1

    with input_path.open("rb") as f_in:
        out_f = open(args.output, "wb") if args.output else None
        try:
            results = process_gscap_interpolation(f_in, out_f, target_rate=args.rate)
        finally:
            if out_f:
                out_f.close()

    if args.stats_json:
        with open(args.stats_json, "w", encoding="utf-8") as f_stats:
            json.dump(results, f_stats, indent=2)

    print(f"GS Interp: {results['frames_in']} input frames -> {results['aux_frames_generated']} aux frames generated at {args.rate} Hz")
    print(f"Overall primitive match rate: {results['overall_match_rate_pct']:.2f}% (Gate >= 95%: {'PASS' if results['match_gate_passed'] else 'FAIL'})")

    return 0 if results["match_gate_passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
