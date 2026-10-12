#!/usr/bin/env python3
"""Verify broader Greedy input recipes without executing a codec or subprocess."""
from pathlib import Path
import argparse
import hashlib
import json
import struct

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = 'tests/fixtures/reference-greedy-families/recipes.json'
MANIFEST_SHA = '389903b9ffcefb8fd39e144649111ce561daebba6845de06479ef6c47e3970d7'
PINS = {
    "scripts/oracles/greedy_families.py": "b8f5be9467988ef60d1a8b9bbdfe7007c08512bc53753cc4ad2e18818b6a0e1f",
    "scripts/oracles/greedy_direct.py": "616248428077b4b90b873aaef1282fdf4cea5bf39388e027d882c0aa1a1f9114",
    "scripts/oracles/greedy_direct.c": "51bfb1d124fbdaf72612f8802f271218f30da478358cd1610b8961f6a1748828",
    "docs/reference-greedy-oracle.md": "b563ef7714dea7ee84e5131bc21fa58a507dd784e8700280e1a4880120974046",
    "docs/reference-greedy-adapter-review.md": "8b40b4227ae9e224fe01676b073980d8a80d90c7afa04b609c459369a83444f5",
    "scripts/check_greedy_tiny_recipes.py": "8d87ccbde9e958e3866a9bf6411ba795afa5f7bc117e07be5a152e4623c9d1b0",
    "tests/fixtures/reference-greedy-tiny/recipes.json": "d617a8e8ea7c92772e8bb9961d0e1baacac07adcc9b619f968df2a1a75dd3006",
    "tests/fixtures/reference-greedy-tiny/capture.json": "18e0ff3d695627c20ef159ecda1ee027ce535d98a55e0b84c261a0ef34c0ffa4",
    "tests/fixtures/reference-greedy-tiny/observations.ndjson": "a2bac02eeeb65292b92892605d15b043097ed94651b4be494fec0427b73b6fff"
}


def require(value, reason):
    if not value:
        raise ValueError(reason)


def identity(path, digest):
    require(path.is_file() and not path.is_symlink() and path.stat().st_size <= 1 << 20,
            'bounded regular source: ' + str(path))
    data = path.read_bytes()
    require(hashlib.sha256(data).hexdigest() == digest, 'reviewed identity: ' + str(path))
    return data


def inputs():
    for name, digest in PINS.items():
        identity(ROOT / name, digest)
    from oracles import greedy_direct as oracle, greedy_families as families
    raw = identity(ROOT / MANIFEST, MANIFEST_SHA)
    manifest = oracle.decode_json(raw)
    cases = families.recipes()
    require(oracle.exact(manifest, {
        'schema_version': 1, 'revision': oracle.REVISION,
        'scope': 'Broader direct Greedy input hypotheses frozen before execution. No expected parser output or branch-hit claim.',
        'pins': PINS, 'recipes': [c.metadata() for c in cases], 'recipe_count': 137,
        'branches': {'chain': 63, 'row': 74},
        'source_bytes': sum(len(c.source) for c in cases), 'packet_bytes': sum(len(c.packet()) for c in cases),
        'salt_provenance': 'Fresh salt 835892b4bb2cae74 from the independently pinned tiny C capture; explicit zero-salt controls are distinct constructed states.'}),
        'frozen broader recipe manifest')
    tiny = oracle.decode_json((ROOT / 'tests/fixtures/reference-greedy-tiny/capture.json').read_bytes())
    pack = (ROOT / 'tests/fixtures/reference-greedy-tiny/observations.ndjson').read_bytes()
    record = tiny['records']['salt-normal']
    rows = oracle.validate_salt(pack[record['offset']:record['offset'] + record['bytes']])
    require(rows[1]['final_salt'] == f'{families.FRESH_SALT:016x}', 'observed fresh salt provenance')
    return oracle, cases, manifest


def malformed_packets(oracle, cases):
    import check_greedy_tiny_recipes as tiny
    _, small, _ = tiny.inputs()
    result = tiny.malformed_packets(oracle, small)
    row = next(c for c in cases if c.name == 'row-width-16-insertions-15')
    raw = row.packet()
    for label, offset, value in (
            ('branch', 72, 2), ('search-zero', 96, 0), ('search-large', 96, 30),
            ('window', 84, 22), ('hash', 88, 20), ('chain-envelope', 92, 19),
            ('floor', 108, row.prefix + 1), ('pending-before-seeds', 112, 0),
            ('duplicate-seed', 156, 0), ('descending-seed', 152, 40),
            ('incomplete-seed-key', 152 + 4 * (len(row.seeds) - 1), row.prefix - 3),
            ('future-seed', 152 + 4 * (len(row.seeds) - 1), row.prefix),
            ('second-repeat-zero', 120, 0)):
        packet = bytearray(raw)
        struct.pack_into('<I', packet, offset, value)
        result['row-' + label] = bytes(packet)
    result['row-truncated-seeds'] = raw[:152 + 4 * len(row.seeds) - 1]
    result['row-truncated-source'] = raw[:-1]
    result['row-extra-eof'] = raw + b'\0'
    return result


def check():
    oracle, cases, manifest = inputs()
    for case in cases:
        require(oracle.decode_packet(case.packet()) == case, 'positive broader packet round trip')
    negatives = malformed_packets(oracle, cases)
    for name, packet in negatives.items():
        try:
            oracle.decode_packet(packet)
        except ValueError:
            continue
        raise ValueError('malformed packet accepted: ' + name)
    return {'recipes': len(cases), 'branches': manifest['branches'],
            'source_bytes': manifest['source_bytes'], 'packet_bytes': manifest['packet_bytes'],
            'malformed_packets_rejected': len(negatives), 'reference_executions': 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', help='offline verification (the only mode)')
    parser.parse_args()
    try:
        result = check()
    except (ValueError, OSError, TypeError, KeyError) as error:
        parser.exit(1, 'Greedy family recipe check failed: ' + str(error) + '\n')
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
