#!/usr/bin/env python3
"""Verify frozen broader public-frame inputs without codec or child execution."""
from pathlib import Path
import argparse
import hashlib
import json

ROOT = Path(__file__).resolve().parents[1]
GENERATOR = ROOT / 'scripts/oracles/double_fast_interop.py'
GENERATOR_SHA256 = '05eefcf492ce7dca0741d3348d165aca8a48e402aa63710d1c043775125705f3'
MANIFEST = ROOT / 'tests/fixtures/reference-double-fast-interop-recipes/recipes.json'
MANIFEST_SHA256 = '81f39a8303c41232b5d4993168f986f967aa2b1034efc77b5f86c4b401373add'


def require(value, reason):
    if not value:
        raise ValueError(reason)


def decode(data):
    def pairs(items):
        result = {}
        for key, value in items:
            require(key not in result, 'duplicate JSON key')
            result[key] = value
        return result
    return json.loads(data, object_pairs_hook=pairs,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError('nonfinite number')))


def check(path=MANIFEST):
    require(path.is_file() and not path.is_symlink() and path.stat().st_size <= 256 << 10,
            'bounded regular manifest')
    data = path.read_bytes()
    value = decode(data)
    require(hashlib.sha256(data).hexdigest() == MANIFEST_SHA256, 'frozen recipe identity')
    require(GENERATOR.is_file() and not GENERATOR.is_symlink()
            and hashlib.sha256(GENERATOR.read_bytes()).hexdigest() == GENERATOR_SHA256,
            'generator identity before import')
    from oracles import double_fast_interop as generator
    rebuilt = generator.manifest()
    require(json.dumps(value, sort_keys=True, allow_nan=False) ==
            json.dumps(rebuilt, sort_keys=True, allow_nan=False), 'independent source regeneration')
    require((value['recipe_count'], value['source_bytes'], value['classifications']) ==
            (159, 38855519, {'success': 113, 'unsupported': 42, 'resource': 4}), 'fixed envelope')
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', required=True)
    parser.add_argument('--manifest', type=Path, default=MANIFEST)
    args = parser.parse_args()
    value = check(args.manifest)
    print('Verified 159 source-only public-frame recipes / 38855519 bytes / '
          '113 admitted hypotheses / 42 unsupported / 4 resource errors / '
          f"{len(value['expected_rust_frame_equivalence'])} equivalence groups. No codec execution.")


if __name__ == '__main__':
    main()
