#!/usr/bin/env python3
"""Project frozen ordinary DoubleFast trajectories; no C execution or codec calls."""
from pathlib import Path
import argparse
import hashlib
import json
import struct

ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / 'tests/fixtures/reference-double-fast-frames'
CHECKER = 'scripts/check_double_fast_frames.py'
CHECKER_SHA256 = '45947d2a2b94332d83af07d4e04ac631d881a493dfb9d5253627a479442f5731'


def require(value, reason):
    if not value:
        raise ValueError(reason)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def blob(value):
    return struct.pack('<I', len(value)) + value


def projection():
    require(sha(ROOT / CHECKER) == CHECKER_SHA256, 'fixed checker identity before import')
    import check_double_fast_frames as checker
    checker.check(checker.COLLECTION)
    validator, generator = checker.load()
    cases = generator.recipes()
    result = bytearray(b'ZDFTR01\0' + struct.pack('<II', 51, 160))
    frames = ['// Generated from the fixed C collection; run pack_double_fast_frame_trajectories.py.',
              '#[rustfmt::skip]', 'const FRAMES: [FrameFiles; 51] = [']
    for case in cases:
        rows = [validator.parse_json(line) for line in
                (checker.COLLECTION / (case.name + '.stdout')).read_bytes().splitlines()]
        begin = rows[0]
        blocks = [r for r in rows if r['type'] == 'block']
        complete = next(r for r in rows if r['type'] == 'complete')
        whole = next((r for r in rows if r['type'] == 'whole_compare'), None)
        name = case.name.encode('ascii')
        body = bytearray(struct.pack('<H', len(name)) + name)
        body.extend(struct.pack('<8BI', case.window_log, case.long_log, case.short_log,
                                case.search_log, case.min_match, 0, 0, 0, case.target))
        body.extend(blob(case.source))
        body.extend(struct.pack('<3Q', int(begin['source_fnv64'], 16), int(complete['frame_fnv64'], 16),
                                int(whole['frame_fnv64'], 16) if whole else 0))
        body.extend(struct.pack('<I', len(case.schedule)))
        for size in case.schedule:
            body.extend(struct.pack('<I', size))
        body.extend(struct.pack('<I', len(blocks)))
        for block in blocks:
            candidate = block['candidate']
            record = bytearray(struct.pack('<4IBBH', block['call'], block['source_start'], block['source_bytes'],
                                           block['window']['dict_limit'] - 2, block['kind'], int(candidate['fresh']), 0))
            for repeats in (block['semantic_before'], block['semantic_after'],
                            block['before']['raw_repeats'], block['after']['raw_repeats']):
                record.extend(struct.pack('<3I', *repeats))
            for key in ('long_table', 'short_table'):
                table = block[key]
                record.extend(struct.pack('<I', len(table['changed'])))
                for change in table['changed']:
                    record.extend(struct.pack('<3I', *change))
                record.extend(struct.pack('<2Q', int(table['before_fnv64'], 16), int(table['after_fnv64'], 16)))
            if candidate['fresh']:
                record.extend(struct.pack('<3I', *block['candidate_state']['raw_repeats']))
                record.extend(struct.pack('<3I', *candidate['semantic_repeats']))
                record.extend(struct.pack('<I', candidate['trailing']))
                record.extend(blob(bytes.fromhex(candidate['literal_bytes'])))
                record.extend(struct.pack('<I', len(candidate['sequences'])))
                for ll, ml, offset, distance in candidate['sequences']:
                    record.extend(struct.pack('<4I', ll, ml, distance, offset))
            body.extend(blob(record))
        result.extend(blob(body))
        prefix = '../../upstream/double-fast-frames/' + case.name
        optional = f'Some(include_bytes!("{prefix}.whole.zst"))' if whole else 'None'
        frames.append(f'    ("{case.name}", include_bytes!("{prefix}.zst"), {optional}),')
    frames.extend(['];', ''])
    data, frame_index = bytes(result), '\n'.join(frames).encode()
    require(len(data) <= 32 << 20, 'packed file cap')
    provenance = {
        'schema_version': 1, 'binary_format': 'ZDFTR01: checked little-endian ordinary DoubleFast trajectories',
        'upstream_revision': generator.REVISION, 'cases': 51, 'positive_blocks': 160, 'source_bytes': 6348944,
        'complete_frames': 58, 'excluded_partial_headers': 13,
        'generator': str(Path(__file__).relative_to(ROOT)), 'generator_sha256': sha(Path(__file__)),
        'checker': CHECKER, 'checker_sha256': CHECKER_SHA256,
        'source_collection': str(checker.COLLECTION.relative_to(ROOT)),
        'source_catalog_sha256': checker.CATALOG_SHA256,
        'source_manifest_sha256': sha(checker.COLLECTION / 'manifest.json'),
        'fixtures': [{'path': n, 'bytes': len(b), 'sha256': hashlib.sha256(b).hexdigest()}
                     for n, b in [('cases.bin', data), ('frames.rs', frame_index)]],
        'policy': 'Test-only source/candidate/two-table/publication projection. Complete frames borrow the original collection. Full entropy and owner observations remain in original NDJSON. No C execution or production parser use.',
    }
    return {'cases.bin': data, 'frames.rs': frame_index,
            'provenance.json': (json.dumps(provenance, sort_keys=True, indent=2) + '\n').encode()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument('--write', action='store_true')
    action.add_argument('--check', action='store_true')
    args = parser.parse_args()
    files = projection()
    if args.write:
        DESTINATION.mkdir(parents=True, exist_ok=True)
        for name, data in files.items():
            (DESTINATION / name).write_bytes(data)
    else:
        for name, data in files.items():
            require((DESTINATION / name).read_bytes() == data, 'frozen projection: ' + name)
    print(f"{'Wrote' if args.write else 'Verified'} 51 ordinary DoubleFast trajectories / 160 blocks / 58 original frames ({len(files['cases.bin'])} packed bytes).")


if __name__ == '__main__':
    main()
