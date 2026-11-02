# TASK-052: Texture Pack Importer and Active View Manager

## 1. Executive Summary & Purpose

- **Task:** TASK-052 — Active view, priority, conflicts, and mod importer (completing the core pipeline of TASK-051 / TASK-052 in `gt4l-core`).
- **Scope:** Implement the production-grade mod management and texture pack import subsystem within `launcher/crates/gt4l-core/src/mods.rs`:
  - Safe import from `.zip` archives and loose folders.
  - Security hardening against Zip-Slip, Zip-Bombs, and Windows device name collisions.
  - Automatic detection and normalization of legacy PCSX2 folder hierarchies.
  - Generation of `gt4mod.toml` manifests and `files.lock.json` SHA-256 integrity digests.
  - Atomic staging and commit into user storage (`mods/store/<id>/`).
  - Active view generation (`mods/active/textures/SCUS-97328/`) using zero-copy NTFS hardlinks with copy fallback.
  - Conflict resolution and reporting based on priority order in `mods.toml`.

---

## 2. Security & Validation Architecture

To protect users against untrusted or malicious third-party mod archives downloaded from the web, the importer enforces strict validation before any file is extracted:

### 2.1 Zip-Slip Defenses (`check_zip_path_safety`)
- **Parent Directory Traversal:** Any component containing `..` or leading slashes (`/`, `\`) is immediately refused.
- **Drive Letters & Absolute Paths:** Disallows `C:`, `D:`, etc.
- **NTFS Alternate Data Streams (ADS):** Rejects any entry containing colons (`:`), preventing hidden executable payload injection.
- **Windows Reserved Device Names:** Blacklists DOS/Windows device names regardless of file extension:
  `CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9`.

### 2.2 Zip-Bomb Mitigations
- **Entry Count Cap:** Maximum 200,000 files per archive.
- **Single File Ceiling:** Maximum 256 MiB uncompressed per individual texture/file.
- **Aggregate Pack Ceiling:** Maximum 20 GiB total uncompressed size.
- **Compression Ratio Check:** Rejects archives with an uncompressed-to-compressed ratio exceeding 200:1.

### 2.3 Executable Stripping
- Prohibits executable and script extensions: `.exe`, `.bat`, `.cmd`, `.ps1`, `.vbs`, `.dll`, `.so`, `.sh`, `.com`, `.scr`, `.pif`. Any such files inside packs are dropped with logged warnings.

---

## 3. PCSX2 Layout Normalization (`normalize_texture_relative_path`)

Community texture packs typically follow varying internal folder conventions. The importer automatically detects and normalizes these structures:
1. `SCUS-97328/replacements/<category>/<file>` $\rightarrow$ `<category>/<file>`
2. `textures/SCUS-97328/replacements/<file>` $\rightarrow$ `<file>`
3. `replacements/<file>` $\rightarrow$ `<file>`
4. Flat directory containing valid texture hash filenames $\rightarrow$ `<file>`
5. `dumps/` subfolders within packs are automatically ignored.

---

## 4. Active View Construction & Conflict Resolution

### 4.1 Priority Resolution
The active texture view is assembled in `mods/active/textures/SCUS-97328/` based on the ordered list in `mods.toml`:
- Enabled mods at the top of the registry take precedence over lower-ranked mods.
- When two enabled mods provide the same texture hash key, the higher-priority mod wins.
- Overridden textures are tracked and returned in `ActiveViewReport.conflicts`.

### 4.2 Storage Efficiency (Hardlinks with Copy Fallback)
- On standard NTFS volumes, textures are linked via `fs::hard_link`, creating new directory entries without consuming additional disk space.
- If hardlink creation fails (e.g., cross-volume configurations, virtual drives, or permission boundaries), `link_or_copy` gracefully falls back to a standard file copy.

---

## 5. Automated Verification

- **Unit Tests (`launcher/crates/gt4l-core/src/mods.rs`):**
  - `zip_path_safety_rejects_parent_traversals_and_colons`: Verifies traversal, ADS, and device name rejection.
  - `normalize_texture_relative_path_handles_pcsx2_layouts`: Tests extraction from nested PCSX2 structures.
  - `import_zip_with_manifest_and_files`: Tests full end-to-end zip extraction, staging, atomic commit, and registry updating.
  - `active_view_builder_resolves_conflicts_by_priority`: Tests active view assembly, priority arbitration, and conflict reporting.
- Full test suite: 109 unit tests and 16 launch contract tests passing.
