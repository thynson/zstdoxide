#!/usr/bin/env python3
"""Project immutable direct Greedy observations into bounded Rust test records."""
from pathlib import Path
import argparse
import hashlib
import json
import struct

ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / 'tests/fixtures/reference-greedy'
CHECKERS = {
    'scripts/check_greedy_tiny_capture.py': 'e26fef9b209b4e6cb1d0a13b513cc580923242567cdc850fe45f27e986f77dd5',
    'scripts/check_greedy_family_capture.py': '0ad7d93df6b79bd9ad11b1e2bb7f9b51aa65894199405d8a965d7f4e76762754',
}


def require(value, reason):
    if not value:
        raise ValueError(reason)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def pack_cases(cases):
    require(len(cases) == 173 and len({c.name for c, _ in cases}) == 173, 'combined case inventory')
    result = bytearray(b'ZGRST01\0' + struct.pack('<I', len(cases)))
    index = {}
    for case, row in cases:
        body = bytearray()

        def numbers(width, values):
            for value in values:
                require(type(value) is int and 0 <= value < 1 << (8 * width), 'integer width/domain')
                body.extend(value.to_bytes(width, 'little'))

        def blob(value):
            require(type(value) is bytes, 'byte blob')
            numbers(4, [len(value)])
            body.extend(value)

        name = case.name.encode('ascii')
        require(1 <= len(name) <= 63, 'bounded name')
        numbers(2, [len(name)])
        body.extend(name)
        numbers(1, [case.branch, case.window_log, case.hash_log, case.chain_log, case.search_log,
                    case.min_match, row['key_width'], row['row_log']])
        numbers(4, [case.target, case.prefix, case.floor, case.next,
                    *case.repeats, *row['parser_repeats'], *row['wire_repeats'],
                    row['trailing_literals'], row['initial_current'], row['repeat_floor'],
                    row['final_next_to_update'], row['final_lazy_skipping'],
                    case.entropy, row['final_entropy']])
        numbers(8, [case.salt, int(row['final_salt'], 16)])
        numbers(4, row['hash_cache'])
        numbers(1, [row['long_length_type']])
        numbers(4, [row['long_length_position']])
        blob(case.source)
        numbers(8, [int(row['source_fnv1a64'], 16)])
        numbers(4, [len(case.seeds), *case.seeds])
        blob(bytes.fromhex(row['stored_literals_hex']))
        numbers(4, [len(row['sequences'])])
        for sequence in row['sequences']:
            numbers(4, [sequence[k] for k in ('raw_ll', 'raw_ml_base', 'll', 'ml', 'distance', 'off_base')])
        for name in ('first_table', 'second_table'):
            table = row[name]
            numbers(1, [table['element_bytes'], table['log']])
            numbers(2, [0])
            numbers(4, [len(table['before'])])
            for values in table['before']:
                numbers(4, values)
            numbers(4, [len(table['changes'])])
            for values in table['changes']:
                numbers(4, values)
            numbers(8, [int(table['before_fnv1a64'], 16), int(table['after_fnv1a64'], 16)])
        require(len(body) <= 32 << 20, 'case body cap')
        result.extend(struct.pack('<I', len(body)))
        index[case.name] = {'offset': len(result), 'bytes': len(body), 'sha256': sha(body)}
        result.extend(body)
    require(len(result) <= 16 << 20, 'frozen collection cap')
    return bytes(result), index


def projection():
    for name, digest in CHECKERS.items():
        path = ROOT / name
        require(path.is_file() and not path.is_symlink() and path.stat().st_size <= 1 << 20,
                'bounded regular checker')
        require(sha(path.read_bytes()) == digest, 'checker identity before import')
    import check_greedy_tiny_capture as tiny, check_greedy_family_capture as families
    cases = []
    collections = []
    for checker, module in ((tiny, 'check_greedy_tiny_recipes'),
                            (families, 'check_greedy_family_recipes')):
        # fixture() verifies its recipe checker's identity before importing it.
        # Only reuse that admitted module after this boundary has passed.
        accepted, observed = checker.fixture()
        inputs = __import__(module)
        oracle, recipes, _ = inputs.inputs()
        for case in recipes:
            rows = [oracle.decode_json(line) for line in observed[case.name].splitlines()]
            cases.append((case, rows[1]))
        collections.append({'path': checker.COLLECTION, 'capture_sha256': checker.FIXTURE_SHA,
                            'observation_sha256': accepted['pack_sha256'], 'direct_cases': len(recipes)})
    data, index = pack_cases(cases)
    provenance = {'schema_version': 1, 'format': 'ZGRST01: bounded little-endian direct Greedy projection',
                  'recipes': 173, 'represented_padding_observations': 346,
                  'represented_direct_observations_across_builds': 692,
                  'upstream_revision': oracle.REVISION, 'generator': 'scripts/pack_greedy_parser_fixtures.py',
                  'generator_sha256': sha(Path(__file__).read_bytes()), 'checkers': CHECKERS,
                  'source_collections': collections, 'records': index,
                  'fixture': {'path': 'cases.bin', 'bytes': len(data), 'sha256': sha(data)},
                  'policy': 'Test-only projection of unchanged inputs and padding-equivalent C observations. No codec/process execution, refreshed expectations, ordinary frames, chronological probes, Rust parser or timing claim.'}
    return data, (json.dumps(provenance, sort_keys=True, indent=2) + '\n').encode()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument('--check', action='store_true')
    action.add_argument('--write', action='store_true')
    args = parser.parse_args()
    try:
        data, provenance = projection()
        outputs = {'cases.bin': data, 'provenance.json': provenance}
        for name, expected in outputs.items():
            path = DESTINATION / name
            if path.exists() or not args.write:
                require(path.is_file() and not path.is_symlink() and path.read_bytes() == expected,
                        'immutable projected output: ' + name)
        if args.write:
            DESTINATION.mkdir(parents=True, exist_ok=True)
            for name, expected in outputs.items():
                path = DESTINATION / name
                if not path.exists():
                    path.write_bytes(expected)
    except (ValueError, OSError, KeyError, TypeError, AttributeError) as error:
        parser.exit(1, 'Greedy projection failed: ' + str(error) + '\n')
    print(json.dumps({'records': 173, 'bytes': len(data), 'sha256': sha(data), 'reference_executions': 0}, sort_keys=True))


if __name__ == '__main__':
    main()
