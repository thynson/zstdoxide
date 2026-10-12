#!/usr/bin/env python3
"""Verify frozen ordinary Greedy input hypotheses without executing a codec."""

from pathlib import Path
import argparse
from dataclasses import replace
import hashlib
import importlib.util
import json
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
GENERATOR = ROOT / 'scripts/oracles/greedy_frames.py'
GENERATOR_SHA256 = '360bf5139b7e2a886e9d2359c1ea258317238bd1906cfe9d123be33df7a0ce44'
MANIFEST = ROOT / 'tests/fixtures/reference-greedy-frame-recipes/recipes.json'
MANIFEST_SHA256 = 'f833a7015aba52bf9a43fc941f1e766ab0bec5ea8c8e285eb970de1f2d702f13'


def require(valid, reason):
    if not valid:
        raise ValueError(reason)


def bounded_file(path, cap):
    require(path.is_file() and not path.is_symlink() and path.stat().st_size <= cap,
            'regular bounded input')
    return path.read_bytes()


def load_generator(path=GENERATOR):
    require(hashlib.sha256(bounded_file(path, 64 << 10)).hexdigest() == GENERATOR_SHA256,
            'frozen generator identity')
    spec = importlib.util.spec_from_file_location('zstdoxide_greedy_frame_recipes', path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def decode(data):
    def pairs(items):
        out = {}
        for name, value in items:
            require(name not in out, 'duplicate JSON key')
            out[name] = value
        return out
    def nonfinite(value):
        raise ValueError('nonfinite JSON number: ' + value)
    return json.loads(data, object_pairs_hook=pairs, parse_constant=nonfinite)


def check(path=MANIFEST):
    data = bounded_file(path, 256 << 10)
    value = decode(data)
    require(hashlib.sha256(data).hexdigest() == MANIFEST_SHA256, 'frozen input manifest')
    module = load_generator()
    rebuilt = module.manifest()
    require(json.dumps(value, sort_keys=True, allow_nan=False) == json.dumps(rebuilt, sort_keys=True, allow_nan=False),
            'source regeneration')
    require((rebuilt['recipe_count'], rebuilt['source_bytes'], rebuilt['scheduled_calls'],
             rebuilt['positive_calls'], rebuilt['whole_comparisons'], rebuilt['branches'], len(rebuilt['header_only']))
            == (126, 6959842, 355, 347, 13, {'chain': 55, 'row': 71}, 15), 'fixed input envelope')
    return rebuilt


def controls():
    """Exercise input boundaries and immutable identity controls; no C/Rust call."""
    module = load_generator()
    base = module.Recipe('control', b'abcd' * 16, (64,), 'Input validation only.')
    base.metadata()
    replace(base, window_log=14, chain_log=18, search_log=8).metadata()
    replace(base, chain_log=29, search_log=29).metadata()
    rejected = 0

    def reject(action):
        nonlocal rejected
        try:
            action()
        except (ValueError, OSError, UnicodeError):
            rejected += 1
        else:
            raise ValueError('malformed control was accepted')

    for changes in [
        {'name': ''}, {'name': 'UPPER'}, {'name': 'x' * 64},
        {'source': bytearray(base.source)}, {'source': bytes(module.MAX_SOURCE + 1)},
        {'hypothesis': ''}, {'hypothesis': 'x' * 257}, {'hypothesis': True},
        {'schedule': [64]}, {'schedule': (0,) * 65}, {'schedule': (63,)},
        {'schedule': (True, 63)}, {'schedule': (-1, 65)}, {'schedule': (65,)},
        {'window_log': 9}, {'window_log': 22}, {'window_log': True},
        {'hash_log': 5}, {'hash_log': 20}, {'hash_log': 6.0},
        {'chain_log': 5}, {'chain_log': 30}, {'search_log': 0}, {'search_log': 30},
        {'min_match': 2}, {'min_match': 8}, {'target': -1}, {'target': 131073},
        {'target': False}, {'window_log': 14, 'chain_log': 19},
        {'window_log': 14, 'search_log': 9}, {'whole_compare': 1},
        {'whole_compare': True, 'schedule': (32, 32)},
        {'source': bytes(2048), 'schedule': (2048,), 'window_log': 10},
    ]:
        reject(lambda changes=changes: replace(base, **changes).metadata())
    for data in [b'{"x":1,"x":2}', b'{"x":NaN}', b'{"x":Infinity}', b'{} trailing']:
        reject(lambda data=data: decode(data))
    with tempfile.TemporaryDirectory(prefix='zstdoxide-greedy-recipes-') as directory:
        root = Path(directory)
        altered = root / 'manifest.json'
        value = decode(MANIFEST.read_bytes())
        value['recipes'][0]['parameters'][6] = 2
        altered.write_text(json.dumps(value))
        reject(lambda: check(altered))
        altered.write_bytes(MANIFEST.read_bytes() + b' ')
        reject(lambda: check(altered))
        altered.write_bytes(bytes((256 << 10) + 1))
        reject(lambda: check(altered))
        linked = root / 'linked.json'
        linked.symlink_to(MANIFEST)
        reject(lambda: check(linked))
        changed_generator = root / 'generator.py'
        changed_generator.write_bytes(GENERATOR.read_bytes() + b'\n')
        reject(lambda: load_generator(changed_generator))
    return {'valid_metadata_controls': 3, 'rejected_controls': rejected, 'codec_executions': 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', required=True)
    parser.add_argument('--manifest', type=Path, default=MANIFEST)
    parser.add_argument('--controls', action='store_true')
    args = parser.parse_args()
    check(args.manifest)
    print('Verified 126 source-only Greedy recipes / 355 scheduled calls / 15 incomplete-header probes; no codec execution.')
    if args.controls:
        print(json.dumps(controls(), sort_keys=True))


if __name__ == '__main__':
    main()
