#!/usr/bin/env python3
"""Check frozen lazy-family input hypotheses and malformed packets without C execution."""
import argparse
import hashlib
import json
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = 'tests/fixtures/reference-lazy-tiny/recipes.json'
MANIFEST_SHA = 'f1bc0ab9185648892ae9c56f1b5ebd2c8f744edef9a1c87d160aa7e313eedd9c'
PINS = {'docs/evidence/2026-10-12-lazy-helper-review.json': '58a21fa9463f6bce62440d9d8dd0c2a810abdde8050d261eac107edeb413bea6',
 'docs/evidence/2026-10-12-lazy-transport.json': 'a40a64612296089fde473d498607085fcdd05a46e8fe892b63439c39ef6705b4',
 'docs/reference-lazy-oracle.md': 'ce404295e890520e691ff7d773c00b517b700be48c7bb019184d18a3c9823e97',
 'scripts/check_greedy_tiny_recipes.py': '8d87ccbde9e958e3866a9bf6411ba795afa5f7bc117e07be5a152e4623c9d1b0',
 'scripts/oracles/greedy_direct.py': '616248428077b4b90b873aaef1282fdf4cea5bf39388e027d882c0aa1a1f9114',
 'scripts/oracles/lazy_direct.c': 'c637a9eab81bf48af509c3cbe772ebf64b183e4a418fe11b8e12aabb6156ed1e',
 'scripts/oracles/lazy_direct.py': '9a3e1cdb77e8e3d392cf2a3e6975ca53f69c73ae6e8d70a147e5a3950843314e',
 'scripts/oracles/lazy_tiny.py': '6250504d4d87fd545e4b9dbd12b9e31e61dc132e8297da6587bfe468f205ca49',
 'tests/fixtures/reference-greedy-tiny/recipes.json': 'd617a8e8ea7c92772e8bb9961d0e1baacac07adcc9b619f968df2a1a75dd3006'}


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
    # Import only after checking the reviewed code and contract identities.
    from oracles import lazy_direct as oracle
    from oracles import lazy_tiny as families
    raw = identity(ROOT / MANIFEST, MANIFEST_SHA)
    manifest = oracle.decode_json(raw)
    cases = families.recipes()
    expected = {'schema_version': 1, 'revision': oracle.REVISION,
                'scope': '138 source-only direct Greedy/Lazy/Lazy2 input hypotheses. No new C observations or Rust Lazy outputs are present.',
                'pins': PINS, 'recipes': [c.metadata() for c in cases], 'recipe_count': 138,
                'predecessor_controls': families.predecessor_controls(),
                'geometry_hypotheses': families.geometry_hypotheses(cases),
                'source_bytes': sum(len(c.source) for c in cases),
                'packet_bytes': sum(len(c.packet()) for c in cases)}
    require(oracle.exact(manifest, expected), 'frozen input manifest differs')
    return oracle, cases, manifest


def malformed_packets(oracle, cases):
    base = cases[0].packet()
    invalid = {'magic': b'X' + base[1:], 'old-magic': b'ZGRDY01\0' + base[8:], 'extra-eof': base + b'\0',
               'empty-id': base[:8] + bytes(64) + base[72:],
               'bad-id': base[:8] + b'/' + base[9:],
               'unterminated-id': base[:8] + b'x' * 64 + base[72:]}
    for length in (0, 7, 71, 151, 152, len(base) - 1):
        invalid[f'truncated-{length}'] = base[:length]
    for label, offset, value in (
            ('branch', 72, 2), ('prefix', 76, oracle.MAX_PREFIX + 1), ('small-block', 80, 6),
            ('large-block', 80, oracle.MAX_BLOCK + 1), ('window-low', 84, 9), ('window-high', 84, 22),
            ('hash-low', 88, 5), ('hash-high', 88, 20), ('chain-low', 92, 5), ('chain-high', 92, 19),
            ('search-zero', 96, 0), ('search-chain-large', 96, 9), ('match-low', 100, 2),
            ('match-high', 100, 8), ('target-large', 104, 131073), ('floor', 108, 1),
            ('pending', 112, 1), ('repeat-zero', 116, 0), ('chain-salt', 128, 1),
            ('chain-entropy', 136, 1), ('seed-count', 140, oracle.MAX_SEEDS + 1),
            ('depth-three', 144, 3), ('depth-max', 144, 0xffffffff), ('reserved-second', 148, 1)):
        packet = bytearray(base)
        struct.pack_into('<I', packet, offset, value)
        invalid[label] = bytes(packet)
    from dataclasses import replace
    prefix = next(c for c in cases if c.name == 'd0-chain-prefix-17')
    seeded = replace(prefix, seeds=(0, 4)).packet()
    for label, offset, value in (('duplicate-seed', 156, 0), ('future-seed', 152, 32),
                                ('seed-wrap', 152, 0xffffffff)):
        packet = bytearray(seeded)
        struct.pack_into('<I', packet, offset, value)
        invalid[label] = bytes(packet)
    return invalid


def check():
    oracle, cases, manifest = inputs()
    for case in cases:
        require(oracle.decode_packet(case.packet()) == case, 'positive packet round trip')
        first, second = oracle.initial_tables(case)
        require(len(first) == 1 << case.hash_log and
                len(second) == 1 << (case.hash_log if case.branch else case.chain_log), 'seed map extent')
    rejected = 0
    for name, packet in malformed_packets(oracle, cases).items():
        try:
            oracle.decode_packet(packet)
        except ValueError:
            rejected += 1
        else:
            raise ValueError('malformed packet accepted: ' + name)
    # Closed-form boundary controls, not captured C hash observations.
    for width in (4, 5, 6):
        for bits in (1, 6, 19, 23, 32):
            require(oracle.key_hash(bytes(width), bits) == 0, 'zero hash')
            require(oracle.key_hash(bytes(width), bits, (1 << 64) - 1) == (1 << bits) - 1,
                    'salt high-bit selection')
    require(oracle.key_hash(b'\x01\0\0\0', 6) == 39, 'four-byte LE high bits')
    return {'recipes': len(cases), 'packet_bytes': manifest['packet_bytes'],
            'source_bytes': manifest['source_bytes'], 'malformed_packets_rejected': rejected,
            'closed_form_hash_controls': 31, 'reference_executions': 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', help='offline check (the only mode)')
    parser.parse_args()
    try:
        result = check()
    except (OSError, ValueError, TypeError, KeyError) as error:
        parser.exit(1, 'Lazy recipe check failed: ' + str(error) + '\n')
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
