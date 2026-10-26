"""Derive the SPU2 envelope fix from the pinned MIT donor IOP SPU2 source (TASK-125, Issue #104).

The donor IOP SPU2 implementation calculates exponential decrease envelope steps via integer
multiplication and shift:
    delta = delta * level / 0x8000;
When level is small (e.g. below 32-4096 depending on release rate), delta truncates to 0.
As a result, level stops decreasing and never reaches 0, preventing voices in kRelease phase
from reaching kOff (if (e.level <= 0) v.phase = kOff). Looping ADPCM samples (such as the GT
Auto oil change or car wash sound effects) therefore repeat indefinitely at residual volume.

This script derives runtime/src/iop/spu2.cpp with an overlay enforcing that delta decrements
by at least 1 when level > 0:
    if (delta == 0 && level > 0) delta = -1;

Removing the overlay restores the exact pinned donor source byte-for-byte.
"""

import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DONOR_SPU2 = 'runtime/src/iop/spu2.cpp'
DONOR_SPU2_SHA256 = '84809a6aa1892da83caa3858b3e7a344b447210d049ab3c2a264696217caa4d9'

DONOR_STEP = '''    void step(int rate, bool decrease, bool exponential) {
        const int shift = rate >> 2;
        int cycles = 1 << std::max(0, shift - 11);
        if (exponential && !decrease && level > 0x6000) cycles *= 4;
        if (++counter < cycles) return;
        counter = 0;
        int delta = (decrease ? -8 + (rate & 3) : 7 - (rate & 3)) << std::max(0, 11 - shift);
        if (exponential && decrease) delta = delta * level / 0x8000;
        level = std::clamp(level + delta, 0, 0x7FFF);
    }'''

DERIVED_STEP = '''    // TASK-125: SPU2 exponential decrease underflow fix (Issue #104).
    // When integer division delta * level / 0x8000 truncates to 0 while level > 0,
    // force delta = -1 so the envelope decays to 0 and transitions to kOff.
    // Prevents looping SFX (e.g. GT Auto oil change / car wash audio) from persisting forever.
    void step(int rate, bool decrease, bool exponential) {
        const int shift = rate >> 2;
        int cycles = 1 << std::max(0, shift - 11);
        if (exponential && !decrease && level > 0x6000) cycles *= 4;
        if (++counter < cycles) return;
        counter = 0;
        int delta = (decrease ? -8 + (rate & 3) : 7 - (rate & 3)) << std::max(0, 11 - shift);
        if (exponential && decrease) {
            delta = delta * level / 0x8000;
            if (delta == 0 && level > 0) delta = -1;
        }
        level = std::clamp(level + delta, 0, 0x7FFF);
    }'''


def apply_overlay(source):
    """Returns the derived spu2.cpp text (LF line endings)."""
    if source.count(DONOR_STEP) != 1:
        raise ValueError('Donor Envelope::step anchor must appear exactly once in the donor source')
    return source.replace(DONOR_STEP, DERIVED_STEP, 1)


def remove_overlay(source):
    """Reverses apply_overlay so the input can be checked byte for byte."""
    if source.count(DERIVED_STEP) != 1:
        raise ValueError('Derived source lacks the SPU2 envelope overlay')
    return source.replace(DERIVED_STEP, DONOR_STEP, 1)


def read_donor(assets_root):
    donor_path = assets_root / 'private/references/ps2-web-recomp' / DONOR_SPU2
    raw = donor_path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != DONOR_SPU2_SHA256:
        raise ValueError(f'Donor source differs from the pinned version: {DONOR_SPU2}')
    return raw.decode('utf-8').replace('\r\n', '\n')


def prepare(output, assets_root=None):
    assets_root = Path(assets_root or ROOT).resolve()
    output = output.resolve()
    private = assets_root / 'private'
    if not output.is_relative_to(private) or output == private:
        raise ValueError('Derived donor code must stay inside the assets checkout private/')
    source = read_donor(assets_root)
    derived = apply_overlay(source)
    if remove_overlay(derived) != source:
        raise ValueError('SPU2 envelope overlay does not reverse to the donor input')
    output.mkdir(parents=True, exist_ok=True)
    (output / 'spu2.cpp').write_text(derived, encoding='utf-8', newline='\n')
    manifest = {
        'schema_version': 1,
        'donor_source': DONOR_SPU2,
        'donor_sha256': DONOR_SPU2_SHA256,
        'spu2_sha256': hashlib.sha256(derived.encode('utf-8')).hexdigest(),
        'overlay': 'TASK-125: SPU2 exponential decrease envelope underflow fix (#104)',
    }
    (output / 'runtime.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8', newline='\n')
    return derived


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--assets-root', type=Path)
    options = parser.parse_args()
    prepare(options.output, options.assets_root)
