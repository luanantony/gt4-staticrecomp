"""Annotate generated EE C++ units with decompilation function names (TASK-020).

Reads docs/data/ee-names.csv (built by scripts/ee_symbols.py) and annotates
dispatch labels in generated unit_XXXX.cpp files:
  case 12u: goto L_001010E0; // GranTurismo4__GameObjectPS2__start
  L_001010E0: // GranTurismo4__GameObjectPS2__start

Key contract:
  strip_annotations(annotate_source(source, name_map)) == source
Stripping annotations reproduces the original unannotated source byte-for-byte,
guaranteeing identical AST, compilation semantics, and object code.
"""

import argparse
import csv
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_NAMES_CSV = ROOT / 'docs' / 'data' / 'ee-names.csv'

CASE_LABEL = re.compile(r'^(\s*case\s+\d+u:\s+goto\s+L_([0-9A-Fa-f]{8});)(?:\s*//.*)?$')
GOTO_TARGET = re.compile(r'^(\s*L_([0-9A-Fa-f]{8}):)(?:\s*//.*)?$')
ANNOTATION_COMMENT = re.compile(r'^(\s*(?:case\s+\d+u:\s+goto\s+L_[0-9A-Fa-f]{8};|L_[0-9A-Fa-f]{8}:))\s*//.*$')


def format_address(address):
    """Formats address as standard 0xXXXXXXXX (lowercase 0x, uppercase hex digits)."""
    if isinstance(address, int):
        return f"0x{address:08X}"
    addr_str = str(address).strip()
    if addr_str.lower().startswith("0x"):
        addr_str = addr_str[2:]
    try:
        val = int(addr_str, 16)
        return f"0x{val:08X}"
    except ValueError:
        return None


def score_entry(row):
    """Scores a name row for canonical preference.
    Higher score wins:
      - named quality (+100) vs placeholder (0)
      - decomp / adhoc (+20) vs gthd (+10)
    """
    quality_score = 100 if row.get('quality') == 'named' else 0
    origin = row.get('origin', '')
    if origin in ('decomp', 'adhoc'):
        origin_score = 20
    elif origin == 'gthd':
        origin_score = 10
    else:
        origin_score = 0
    return (quality_score + origin_score, -len(row.get('name', '')))


def load_name_map(csv_path=DEFAULT_NAMES_CSV):
    """Loads CSV and builds a dictionary:
    address (int and '0xXXXXXXXX' string) -> {
        'primary': str,
        'quality': str,
        'kind': str,
        'origin': str,
        'aliases': list of str,
    }
    """
    path = Path(csv_path)
    if not path.is_file():
        raise FileNotFoundError(f"Name table not found: {path}")

    by_addr = {}
    with open(path, mode='r', encoding='utf-8', newline='') as handle:
        reader = csv.DictReader(handle)
        for row in reader:
            normalized = format_address(row['address'])
            if normalized:
                by_addr.setdefault(normalized, []).append(row)

    name_map = {}
    for addr_str, rows in by_addr.items():
        sorted_rows = sorted(rows, key=lambda r: (score_entry(r), r['name']), reverse=True)
        primary_row = sorted_rows[0]
        aliases = [r['name'] for r in sorted_rows if r['name'] != primary_row['name']]

        entry = {
            'address': addr_str,
            'primary': primary_row['name'],
            'quality': primary_row['quality'],
            'kind': primary_row['kind'],
            'origin': primary_row['origin'],
            'aliases': aliases,
        }
        addr_int = int(addr_str, 16)
        name_map[addr_str] = entry
        name_map[addr_int] = entry

    return name_map


def lookup_symbol(address, name_map=None, csv_path=DEFAULT_NAMES_CSV):
    """Looks up address in name map. Returns entry dict or None."""
    if name_map is None:
        name_map = load_name_map(csv_path)
    formatted = format_address(address)
    if not formatted:
        return None
    return name_map.get(formatted)


def annotate_source(source_text, name_map):
    """Annotates source code lines containing case labels or goto targets with decomp names."""
    lines = source_text.splitlines(keepends=True)
    out_lines = []

    for line in lines:
        line_ending = '\r\n' if line.endswith('\r\n') else ('\n' if line.endswith('\n') else '')
        stripped = line[:-len(line_ending)] if line_ending else line

        case_match = CASE_LABEL.match(stripped)
        if case_match:
            base_stmt, hex_addr = case_match.group(1), case_match.group(2)
            symbol = name_map.get(f"0x{hex_addr.upper()}")
            if symbol:
                out_lines.append(f"{base_stmt} // {symbol['primary']}{line_ending}")
            else:
                out_lines.append(f"{base_stmt}{line_ending}")
            continue

        target_match = GOTO_TARGET.match(stripped)
        if target_match:
            base_stmt, hex_addr = target_match.group(1), target_match.group(2)
            symbol = name_map.get(f"0x{hex_addr.upper()}")
            if symbol:
                out_lines.append(f"{base_stmt} // {symbol['primary']}{line_ending}")
            else:
                out_lines.append(f"{base_stmt}{line_ending}")
            continue

        out_lines.append(line)

    return ''.join(out_lines)


def strip_annotations(annotated_text):
    """Removes added comments from case labels and goto targets, restoring byte identity."""
    lines = annotated_text.splitlines(keepends=True)
    out_lines = []

    for line in lines:
        line_ending = '\r\n' if line.endswith('\r\n') else ('\n' if line.endswith('\n') else '')
        stripped = line[:-len(line_ending)] if line_ending else line

        annotated_match = ANNOTATION_COMMENT.match(stripped)
        if annotated_match:
            base_stmt = annotated_match.group(1)
            out_lines.append(f"{base_stmt}{line_ending}")
        else:
            out_lines.append(line)

    return ''.join(out_lines)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest='command', required=True)

    lookup_parser = subparsers.add_parser('lookup', help='Lookup function name for an EE address')
    lookup_parser.add_argument('address', help='Hex address (e.g. 0x001010E0)')
    lookup_parser.add_argument('--names', default=DEFAULT_NAMES_CSV, type=Path, help='Path to ee-names.csv')

    annotate_parser = subparsers.add_parser('annotate', help='Annotate a file or directory with decomp names')
    annotate_parser.add_argument('target', type=Path, help='Target C++ source file or folder')
    annotate_parser.add_argument('--out', type=Path, help='Output path or directory')
    annotate_parser.add_argument('--names', default=DEFAULT_NAMES_CSV, type=Path, help='Path to ee-names.csv')

    verify_parser = subparsers.add_parser('verify-invariance', help='Verify round-trip comment stripping invariance')
    verify_parser.add_argument('target', type=Path, help='Target C++ source file')
    verify_parser.add_argument('--names', default=DEFAULT_NAMES_CSV, type=Path, help='Path to ee-names.csv')

    args = parser.parse_args(argv)

    if args.command == 'lookup':
        name_map = load_name_map(args.names)
        entry = lookup_symbol(args.address, name_map=name_map)
        if entry:
            print(f"Address:  {entry['address']}")
            print(f"Primary:  {entry['primary']}")
            print(f"Quality:  {entry['quality']}")
            print(f"Kind:     {entry['kind']}")
            print(f"Origin:   {entry['origin']}")
            if entry['aliases']:
                print(f"Aliases:  {', '.join(entry['aliases'])}")
        else:
            print(f"No decomp symbol found for {args.address}")
            return 1
        return 0

    if args.command == 'annotate':
        name_map = load_name_map(args.names)
        if args.target.is_file():
            source = args.target.read_text(encoding='utf-8')
            annotated = annotate_source(source, name_map)
            out_path = args.out if args.out else args.target
            out_path.write_text(annotated, encoding='utf-8')
            print(f"Annotated {args.target} -> {out_path}")
        elif args.target.is_dir():
            out_dir = args.out if args.out else args.target
            out_dir.mkdir(parents=True, exist_ok=True)
            count = 0
            for file_path in args.target.glob('unit_*.cpp'):
                source = file_path.read_text(encoding='utf-8')
                annotated = annotate_source(source, name_map)
                (out_dir / file_path.name).write_text(annotated, encoding='utf-8')
                count += 1
            print(f"Annotated {count} units in {out_dir}")
        else:
            raise FileNotFoundError(f"Target not found: {args.target}")
        return 0

    if args.command == 'verify-invariance':
        name_map = load_name_map(args.names)
        source = args.target.read_text(encoding='utf-8')
        annotated = annotate_source(source, name_map)
        stripped = strip_annotations(annotated)
        if stripped != source:
            raise ValueError(f"Invariance violated on {args.target}! Stripped text differs from original.")
        print(f"Round-trip invariance verified: {args.target} is byte-identical after comment strip.")
        return 0

    return 0


if __name__ == '__main__':
    sys.exit(main())
