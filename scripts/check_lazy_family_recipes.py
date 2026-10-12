#!/usr/bin/env python3
"""Verify broader lazy-family source hypotheses without executing a codec."""
from pathlib import Path
import argparse
import hashlib
import json
import struct

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = 'tests/fixtures/reference-lazy-families/recipes.json'
MANIFEST_SHA = 'f44c9baa055d3efd0720508d78e4fd681984fefe8e7639f3d9e3be4fb574bdf5'
PINS = {'docs/evidence/2026-10-12-lazy-helper-review.json': '58a21fa9463f6bce62440d9d8dd0c2a810abdde8050d261eac107edeb413bea6',
 'docs/evidence/2026-10-12-lazy-transport.json': 'a40a64612296089fde473d498607085fcdd05a46e8fe892b63439c39ef6705b4',
 'docs/reference-lazy-oracle.md': 'ce404295e890520e691ff7d773c00b517b700be48c7bb019184d18a3c9823e97',
 'scripts/check_greedy_family_recipes.py': 'b1edd15038de0fa3b4af0d1d03d7fb15ec636f83ead060cc7578c82449c1a207',
 'scripts/check_greedy_tiny_recipes.py': '8d87ccbde9e958e3866a9bf6411ba795afa5f7bc117e07be5a152e4623c9d1b0',
 'scripts/check_lazy_tiny_capture.py': 'ba5462b9fc26f8471b5b465009c3068b673a3985172d68812ae594ce2fd2e994',
 'scripts/check_lazy_tiny_fixture.py': '835b0689b61debf480612f8778fc6b339f427c51b389bb6495bef540523d6eba',
 'scripts/check_lazy_tiny_recipes.py': '950e9c2ab5e864d8c1fce5d8e9f5e0b67a0c3b7ffe20d77b2a880920637a2069',
 'scripts/oracles/greedy_direct.py': '616248428077b4b90b873aaef1282fdf4cea5bf39388e027d882c0aa1a1f9114',
 'scripts/oracles/greedy_families.py': 'b8f5be9467988ef60d1a8b9bbdfe7007c08512bc53753cc4ad2e18818b6a0e1f',
 'scripts/oracles/lazy_direct.c': 'c637a9eab81bf48af509c3cbe772ebf64b183e4a418fe11b8e12aabb6156ed1e',
 'scripts/oracles/lazy_direct.py': '9a3e1cdb77e8e3d392cf2a3e6975ca53f69c73ae6e8d70a147e5a3950843314e',
 'scripts/oracles/lazy_families.py': 'f1c392d298ad51ff3b19894679c9dcee9a56f1fe22e6b15503a8bb14d3ca3ce3',
 'scripts/oracles/lazy_tiny.py': '6250504d4d87fd545e4b9dbd12b9e31e61dc132e8297da6587bfe468f205ca49',
 'tests/fixtures/reference-greedy-families/recipes.json': '389903b9ffcefb8fd39e144649111ce561daebba6845de06479ef6c47e3970d7',
 'tests/fixtures/reference-greedy-tiny/recipes.json': 'd617a8e8ea7c92772e8bb9961d0e1baacac07adcc9b619f968df2a1a75dd3006',
 'tests/fixtures/reference-lazy-tiny/capture.json': '5f7cf83d440e22cf581b59b7e1b592b9b2db4b2d522ac9efcc57b7753f5736dc',
 'tests/fixtures/reference-lazy-tiny/observations.ndjson': '5d25e610ba0d6bb1379d38fce19208c558873e9cfd5a1696e0e8534b1179997d',
 'tests/fixtures/reference-lazy-tiny/recipes.json': 'f1bc0ab9185648892ae9c56f1b5ebd2c8f744edef9a1c87d160aa7e313eedd9c'}
SCOPE = ('645 direct lazy-family source hypotheses frozen before their campaign. '
         'Includes 138 previously captured tiny inputs, 411 historical Greedy inputs '
         'at all depths and 96 constructed score hypotheses. No new expected output or branch-hit claim.')


def require(value, reason):
    if not value:
        raise ValueError(reason)


def identity(path, digest):
    require(path.is_file() and not path.is_symlink() and path.stat().st_size <= 1 << 20,
            'bounded regular recipe source: ' + str(path))
    raw = path.read_bytes()
    require(hashlib.sha256(raw).hexdigest() == digest, 'reviewed recipe identity: ' + str(path))
    return raw


def expected(oracle, families, cases):
    return {'schema_version': 1, 'revision': oracle.REVISION, 'scope': SCOPE, 'pins': PINS,
            'recipes': [c.metadata() for c in cases], 'recipe_count': 645,
            'branches': {name: sum(c.branch == branch for c in cases)
                         for branch, name in ((0, 'chain'), (1, 'row'))},
            'depth_counts': [sum(c.depth == d for c in cases) for d in range(3)],
            'predecessor_controls': families.predecessor_controls(),
            'score_hypotheses': families.score_hypotheses(),
            'source_bytes': sum(len(c.source) for c in cases),
            'packet_bytes': sum(len(c.packet()) for c in cases)}


def inputs():
    for name, digest in PINS.items():
        identity(ROOT / name, digest)
    from oracles import lazy_direct as oracle, lazy_families as families
    import check_greedy_family_recipes as old
    import check_lazy_tiny_fixture as tiny
    old.inputs()
    accepted, observed = tiny.fixture()
    for depth in range(3):
        rows = oracle.validate_salt(observed[f'salt-normal-{depth}'], depth)
        require(rows[1]['final_salt'] == f'{families.greedy_families.FRESH_SALT:016x}',
                'actual fresh-context salt at every depth')
    manifest = oracle.decode_json(identity(ROOT / MANIFEST, MANIFEST_SHA))
    cases = families.recipes()
    require(oracle.exact(manifest, expected(oracle, families, cases)), 'frozen broader lazy manifest')
    return oracle, cases, manifest


def malformed_packets(oracle, cases):
    import check_lazy_tiny_recipes as tiny
    _, small, _ = tiny.inputs()
    invalid = tiny.malformed_packets(oracle, small)
    row = next(c for c in cases if c.name == 'd2-row-width-16-insertions-15')
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
        invalid['row-' + label] = bytes(packet)
    invalid.update({'row-truncated-seeds': raw[:152 + 4 * len(row.seeds) - 1],
                    'row-truncated-source': raw[:-1], 'row-extra-eof': raw + b'\0'})
    return invalid


def check_score_sources(cases, hypotheses):
    by_name = {c.name: c for c in cases}
    for h in hypotheses:
        c = by_name[h['id']]
        if 'baseline_score' in h:
            depth = h['query_delta']
            repeat = h['purpose'].startswith('repeat-')
            old_bits = h['baseline_offbase'].bit_length() - 1
            new_bits = h['candidate_offbase'].bit_length() - 1
            multiplier = 3 if repeat and depth == 1 else 4
            penalty = 1 if repeat else 4 if depth == 1 else 7
            require(h['baseline_score'] == multiplier * 8 - old_bits + penalty
                    and h['candidate_score'] == multiplier * h['candidate_length'] - new_bits,
                    'closed-form placed-copy scores')
            relation = ('below' if h['candidate_score'] < h['baseline_score'] else
                        'above' if h['candidate_score'] > h['baseline_score'] else 'equal')
            require(relation == h['arithmetic_relation'], 'placed-copy threshold relation')
            placements = [(0, h['baseline_offbase'] - 3, 8, True),
                          (depth, c.repeats[0] if repeat else h['candidate_offbase'] - 3,
                           h['candidate_length'], not repeat)]
        elif 'table_length' in h:
            placements = [(0, 2045, h['table_length'], True), (1, 253, h['repeat_length'], False)]
            require(c.repeats[0] == 253, 'placed baseline repeat')
        else:
            placements = [(q, o - 3, length, True) for q, o, length in
                          zip(h['query_deltas'], h['placed_offbases'], h['placed_lengths'], strict=True)]
        seeds = []
        for delta, distance, length, is_explicit in placements:
            query = c.prefix + delta
            match = query - distance
            require(c.source[match:match + length] == c.source[query:query + length]
                    and c.source[match + length] != c.source[query + length]
                    and c.source[match - 1] != c.source[query - 1], 'exact constructed byte-copy extent')
            if is_explicit:
                seeds.append(match)
        require(tuple(sorted(seeds)) == c.seeds, 'exact constructed seed coordinates')
    require(len(hypotheses) == 96, 'constructed source inventory')


def check():
    oracle, cases, manifest = inputs()
    for c in cases:
        require(oracle.decode_packet(c.packet()) == c, 'positive broader packet round trip')
    invalid = malformed_packets(oracle, cases)
    for name, packet in invalid.items():
        try:
            oracle.decode_packet(packet)
        except ValueError:
            continue
        raise ValueError('malformed packet accepted: ' + name)
    check_score_sources(cases, manifest['score_hypotheses'])
    return {'recipes': len(cases), 'branches': manifest['branches'], 'depth_counts': manifest['depth_counts'],
            'source_bytes': manifest['source_bytes'], 'packet_bytes': manifest['packet_bytes'],
            'malformed_packets_rejected': len(invalid), 'constructed_score_sources': 96,
            'predecessor_controls': 173, 'reference_executions': 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', help='offline verification (the only mode)')
    parser.parse_args()
    try:
        result = check()
    except (ValueError, OSError, TypeError, KeyError) as error:
        parser.exit(1, 'Lazy family recipe check failed: ' + str(error) + '\n')
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
