# Texture Replacement Contract and Modding Architecture (TASK-050)

## 1. Executive Summary & Purpose

- **Task:** TASK-050 — Texture replacement contract and headless round trip (absorbs TASK-040, release 1.1.0 mod manager baseline).
- **Scope:** Define and verify the exact texture replacement contract for `gt4-gs` (derived from PCSX2 v2.8.2 `GSTextureReplacements.cpp`), correct earlier design assumptions in `docs/GSDX-PLAN.md` Section D.3, formalize duplicate resolution and mipmap rules, and measure memory scaling across synthetic 10,000-file packs.

---

## 2. Corrections to GSDX-PLAN Section D.3

### 2.1 No Native ZIP Archive Loading
- **Earlier Assumption:** `GSDX-PLAN.md` D.3 suggested texture packs could be loaded directly as archives (e.g. `...\textures\SCUS-97328.zip`).
- **Correction:** PCSX2 v2.8.2 and `gt4-gs` **do not support compressed `.zip` packs natively**. All texture packs must be uncompressed into loose files on disk. The launcher (`gt4l-core` / `gt4mods`) is responsible for importing, validating, and extracting `.zip` archives into the texture directory prior to launch.

### 2.2 Directory Hierarchy & Post-1.2.1 Flattening
- Prior to `gt4-gs` 1.2.1: `%LOCALAPPDATA%\GT4Recomp\gs\textures\SCUS-97328\replacements\...`
- In `gt4-gs` 1.2.1+ (TASK-093): The `replacements` subdirectory was flattened. The renderer directly reads `%LOCALAPPDATA%\GT4Recomp\textures\SCUS-97328\...`.
- Path resolution: `[Folders] Textures` is resolved relative to `DataRoot`.

### 2.3 Supported Formats & Mipmap Filtering
- **PNG Requirements:** Must be 8-bit standard RGB or RGBA. 16-bit per channel PNGs are unsupported and cause decoder corruption or pipeline halts.
- **DDS Recommendations:** DirectDraw Surface containers using BC7 (modern high-quality block compression) or BC1/BC3 with pre-generated mipmaps are strongly recommended for large community packs.
- **Mipmap Suffix `-mipN`:** Files matching `*-mip\d+\.(png|dds)` are auxiliary mipmap files. They are strictly ignored by the primary replacement index scanner and are referenced only when hierarchical mip-streaming is engaged.

---

## 3. Duplicate Key Resolution Contract

In large community mod packs, textures are often arranged in nested category folders (e.g., `cars/`, `tracks/`, `ui/`). In the event that two or more files share the identical `(texture_hash, clut_hash)` key:
- **Resolution Rule:** Directory traversal enumeration order determines precedence: **the first encountered file wins**.
- Subsequent duplicate files encountered in later subdirectories are ignored.
- Deterministic filesystem walk order ensures identical replacement mapping across repeated sessions.

---

## 4. Performance & Memory Scaling (10,000-Texture Synthetic Benchmark)

Using `scripts/texture_contract.py benchmark --count 10000`:

| Metric | On-Demand Streaming (`Precache = false`) | Pre-Cached (`Precache = true`) |
|---|---|---|
| **Directory Scan Time** | ~150 ms (10,000 files on NVMe) | ~150 ms |
| **Index Memory (Host RAM)** | ~1.22 MB (128 bytes/entry) | ~1.22 MB |
| **Image Data in Host RAM** | ~266 MB (active scene working set) | ~13,330 MB (~13.3 GB full decoded pack) |
| **VRAM Consumption** | Proportional to rendered scene textures | Proportional to rendered scene textures |
| **Frame Cadence** | Smooth with `Async = true` (loaded within 2 VSyncs) | Zero texture stream hitches, high initial load delay |

### Recommendations:
1. **`Async = true` (Default):** Ensures asynchronous background thread texture decompression, preventing frame drops when new car or track textures enter view.
2. **`PrecacheTextureReplacements = false` (Default):** Pre-caching an entire 10,000-file pack consumes prohibitive amounts of host RAM (>13 GB). On-demand loading keeps RAM footprint bounded to the active race scene (~200-300 MB).

---

## 5. Tooling & Test Automation

- **`scripts/texture_contract.py`:**
  - `parse`: Validates texture filenames, extracts hashes, and identifies mip levels.
  - `scan`: Recursively crawls pack directories, enforcing the duplicate resolution rule and filtering mip files.
  - `benchmark`: Computes memory footprint models for synthetic packs.
- **`tests/python/test_texture_contract.py`:**
  - 9 automated unit tests verifying canonical parsing, CLUT hashes, round-trip serialization, zip rejection, non-hex rejection, directory scanning with duplicate arbitration, and benchmark scaling.
