#!/usr/bin/env python3
"""Check frozen Greedy input hypotheses and malformed packets without C execution."""
import argparse
import hashlib
import json
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = 'tests/fixtures/reference-greedy-tiny/recipes.json'
MANIFEST_SHA = 'd617a8e8ea7c92772e8bb9961d0e1baacac07adcc9b619f968df2a1a75dd3006'
PINS = {
    'scripts/oracles/greedy_direct.c': '51bfb1d124fbdaf72612f8802f271218f30da478358cd1610b8961f6a1748828',
    'scripts/oracles/greedy_direct.py': '616248428077b4b90b873aaef1282fdf4cea5bf39388e027d882c0aa1a1f9114',
    'docs/reference-greedy-oracle.md': 'b563ef7714dea7ee84e5131bc21fa58a507dd784e8700280e1a4880120974046',
    'docs/reference-greedy-adapter-review.md': '8b40b4227ae9e224fe01676b073980d8a80d90c7afa04b609c459369a83444f5',
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
    # Import only after checking the reviewed code and contract identities.
    from oracles import greedy_direct as oracle
    raw = identity(ROOT / MANIFEST, MANIFEST_SHA)
    manifest = oracle.decode_json(raw)
    cases = oracle.recipes()
    expected = {'schema_version': 1, 'revision': oracle.REVISION,
                'scope': 'Thirty-six source-only direct Greedy input hypotheses. No C observations or Rust Greedy outputs are present.',
                'pins': PINS, 'recipes': [c.metadata() for c in cases], 'recipe_count': 36,
                'source_bytes': sum(len(c.source) for c in cases),
                'packet_bytes': sum(len(c.packet()) for c in cases)}
    require(oracle.exact(manifest, expected), 'frozen input manifest differs')
    return oracle, cases, manifest


def malformed_packets(oracle, cases):
    base = cases[0].packet()
    invalid = {'magic': b'X' + base[1:], 'extra-eof': base + b'\0',
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
            ('reserved-first', 144, 1), ('reserved-second', 148, 1)):
        packet = bytearray(base)
        struct.pack_into('<I', packet, offset, value)
        invalid[label] = bytes(packet)
    from dataclasses import replace
    prefix = next(c for c in cases if c.name == 'chain-prefix-17')
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
        parser.exit(1, 'Greedy recipe check failed: ' + str(error) + '\n')
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
