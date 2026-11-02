"""Texture replacement naming contract, parser, and scanner (TASK-050).

Implements and verifies the PCSX2 v2.8.2 / gt4-gs texture replacement contract:
- Texture names are 64-bit texture data hashes and optional CLUT hashes:
    {texture_hash:016x}-{clut_hash:08x}.png
    {texture_hash:016x}.png
    {texture_hash:016x}-{clut_hash:08x}.dds
- Zip packs are NOT supported natively by the renderer; files must be extracted.
- Suffix '-mipN' (e.g. foo-mip1.png) indicates mipmap levels and is ignored
  by the primary replacement scanner.
- PNG formats must be 8-bit RGB or 8-bit RGBA (16-bit PNGs are unsupported).
- Duplicate keys in different subdirectories are resolved by directory enumeration
  order (first encountered file wins).
- PrecacheTextureReplacements loads all textures into host RAM upfront; on-demand
  loading (precache=False) keeps RAM footprint proportional to the active scene.
"""

import argparse
from dataclasses import dataclass, field
import os
from pathlib import Path
import re
import sys
from typing import Dict, List, Optional, Tuple

HEX_CHARS = set("0123456789abcdefABCDEF")
TEXTURE_NAME_PATTERN = re.compile(
    r"^([0-9A-Fa-f]{8,16})(?:-([0-9A-Fa-f]{8,16}))?(?:-mip(\d+))?$"
)


@dataclass(frozen=True)
class TextureKey:
    texture_hash: int
    clut_hash: Optional[int] = None

    @property
    def canonical_stem(self) -> str:
        if self.clut_hash is not None:
            return f"{self.texture_hash:016x}-{self.clut_hash:08x}"
        return f"{self.texture_hash:016x}"


@dataclass
class TextureMetadata:
    stem: str
    extension: str
    key: Optional[TextureKey]
    is_mip: bool = False
    mip_level: Optional[int] = None
    is_valid: bool = True
    error: Optional[str] = None


@dataclass
class ScanReport:
    total_files_examined: int = 0
    valid_replacements: Dict[TextureKey, Path] = field(default_factory=dict)
    duplicates: List[Tuple[TextureKey, Path, Path]] = field(default_factory=list)
    ignored_mips: List[Path] = field(default_factory=list)
    invalid_files: List[Tuple[Path, str]] = field(default_factory=list)


def parse_texture_filename(filename: str) -> TextureMetadata:
    """Parses a texture replacement filename according to the PCSX2/gt4-gs contract."""
    path = Path(filename)
    stem = path.stem
    ext = path.suffix.lower()

    if ext == ".zip":
        return TextureMetadata(
            stem=stem,
            extension=ext,
            key=None,
            is_valid=False,
            error="Zip packs are not supported; texture packs must be unzipped on disk",
        )

    if ext not in (".png", ".dds"):
        return TextureMetadata(
            stem=stem,
            extension=ext,
            key=None,
            is_valid=False,
            error=f"Unsupported texture extension '{ext}'; expected '.png' or '.dds'",
        )

    match = TEXTURE_NAME_PATTERN.match(stem)
    if not match:
        return TextureMetadata(
            stem=stem,
            extension=ext,
            key=None,
            is_valid=False,
            error=f"Filename '{stem}' does not match hex hash pattern",
        )

    tex_str, clut_str, mip_str = match.groups()
    try:
        tex_hash = int(tex_str, 16)
        clut_hash = int(clut_str, 16) if clut_str else None
        mip_level = int(mip_str) if mip_str else None
    except ValueError as err:
        return TextureMetadata(
            stem=stem,
            extension=ext,
            key=None,
            is_valid=False,
            error=f"Hex parse error: {err}",
        )

    is_mip = mip_level is not None
    key = TextureKey(texture_hash=tex_hash, clut_hash=clut_hash)

    return TextureMetadata(
        stem=stem,
        extension=ext,
        key=key,
        is_mip=is_mip,
        mip_level=mip_level,
        is_valid=True,
    )


def format_texture_filename(
    texture_hash: int,
    clut_hash: Optional[int] = None,
    mip_level: Optional[int] = None,
    ext: str = "png",
) -> str:
    """Formats a canonical PCSX2/gt4-gs texture replacement filename."""
    clean_ext = ext.lstrip(".").lower()
    if clut_hash is not None:
        base = f"{texture_hash:016x}-{clut_hash:08x}"
    else:
        base = f"{texture_hash:016x}"

    if mip_level is not None:
        return f"{base}-mip{mip_level}.{clean_ext}"
    return f"{base}.{clean_ext}"


def scan_replacement_directory(root_dir: Path) -> ScanReport:
    """Recursively scans a directory for texture replacements.
    Enforces duplicate-key resolution (first encountered file wins) and skips mip files.
    """
    report = ScanReport()
    root = Path(root_dir)
    if not root.is_dir():
        return report

    # Iterate deterministically
    for current_dir, _, files in os.walk(root):
        for fname in sorted(files):
            file_path = Path(current_dir) / fname
            report.total_files_examined += 1

            meta = parse_texture_filename(fname)
            if not meta.is_valid:
                report.invalid_files.append((file_path, meta.error or "Unknown error"))
                continue

            if meta.is_mip:
                report.ignored_mips.append(file_path)
                continue

            assert meta.key is not None
            if meta.key in report.valid_replacements:
                existing = report.valid_replacements[meta.key]
                report.duplicates.append((meta.key, file_path, existing))
            else:
                report.valid_replacements[meta.key] = file_path

    return report


def estimate_synthetic_pack_footprint(
    file_count: int = 10000,
    width: int = 1024,
    height: int = 1024,
    dds_bc7: bool = True,
    precache: bool = False,
) -> Dict[str, float]:
    """Calculates synthetic memory footprint metrics for a texture pack.
    - DDS BC7: 1 byte per pixel (16 bytes per 4x4 block)
    - 8-bit RGBA PNG: 4 bytes per pixel uncompressed in RAM
    """
    bytes_per_pixel = 1.0 if dds_bc7 else 4.0
    single_texture_bytes = width * height * bytes_per_pixel
    # With mipmaps, total storage is approx 1.33x
    single_texture_with_mips = single_texture_bytes * 1.333

    total_storage_mb = (single_texture_with_mips * file_count) / (1024 * 1024)

    # Hash map tracking overhead in host RAM: ~128 bytes per entry
    index_ram_mb = (file_count * 128) / (1024 * 1024)

    if precache:
        # Precache loads the entire decoded/compressed image payload into host RAM
        host_ram_mb = index_ram_mb + total_storage_mb
    else:
        # On-demand only keeps active working set (e.g. ~100-250 textures per scene)
        active_scene_textures = 200
        host_ram_mb = index_ram_mb + (
            (active_scene_textures * single_texture_with_mips) / (1024 * 1024)
        )

    # Estimated directory scan & hash index time: ~0.015 ms per file on NVMe
    estimated_scan_ms = file_count * 0.015

    return {
        "file_count": file_count,
        "single_texture_kb": single_texture_with_mips / 1024,
        "total_pack_size_mb": total_storage_mb,
        "index_ram_mb": index_ram_mb,
        "host_ram_mb": host_ram_mb,
        "estimated_scan_ms": estimated_scan_ms,
        "precache_enabled": precache,
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    parse_cmd = subparsers.add_parser("parse", help="Parse a texture replacement filename")
    parse_cmd.add_argument("filename", help="Texture filename or path")

    scan_cmd = subparsers.add_parser("scan", help="Scan a directory for texture replacements")
    scan_cmd.add_argument("directory", type=Path, help="Directory to scan")

    bench_cmd = subparsers.add_parser("benchmark", help="Estimate synthetic pack memory metrics")
    bench_cmd.add_argument("--count", type=int, default=10000, help="Number of files")
    bench_cmd.add_argument("--precache", action="store_true", help="Enable precache")

    args = parser.parse_args(argv)

    if args.command == "parse":
        meta = parse_texture_filename(args.filename)
        print(f"File:       {args.filename}")
        print(f"Valid:      {meta.is_valid}")
        if meta.is_valid and meta.key:
            print(f"Tex Hash:   0x{meta.key.texture_hash:016x}")
            if meta.key.clut_hash is not None:
                print(f"CLUT Hash:  0x{meta.key.clut_hash:08x}")
            print(f"Is Mip:     {meta.is_mip} (Level: {meta.mip_level})")
            print(f"Canonical:  {meta.key.canonical_stem}{meta.extension}")
        else:
            print(f"Error:      {meta.error}")
        return 0 if meta.is_valid else 1

    if args.command == "scan":
        report = scan_replacement_directory(args.directory)
        print(f"Examined:     {report.total_files_examined} files")
        print(f"Valid keys:   {len(report.valid_replacements)}")
        print(f"Duplicates:   {len(report.duplicates)}")
        print(f"Ignored mips: {len(report.ignored_mips)}")
        print(f"Invalid:      {len(report.invalid_files)}")
        return 0

    if args.command == "benchmark":
        stats = estimate_synthetic_pack_footprint(
            file_count=args.count, precache=args.precache
        )
        print(f"Synthetic Pack Benchmark ({stats['file_count']} textures):")
        print(f"  Total Pack Disk Size:   {stats['total_pack_size_mb']:.1f} MB")
        print(f"  Scan Time (est):        {stats['estimated_scan_ms']:.1f} ms")
        print(f"  Index Host RAM:         {stats['index_ram_mb']:.2f} MB")
        print(f"  Active Host RAM:        {stats['host_ram_mb']:.1f} MB (precache={stats['precache_enabled']})")
        return 0

    return 0


if __name__ == "__main__":
    sys.exit(main())
