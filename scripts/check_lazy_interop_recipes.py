#!/usr/bin/env python3
"""Verify frozen broader public-frame inputs without codec or child execution."""
from pathlib import Path
import argparse
import hashlib
import json

ROOT = Path(__file__).resolve().parents[1]
GENERATOR = ROOT / 'scripts/oracles/lazy_interop.py'
GENERATOR_SHA256 = 'db2f4e9568eff5a2508e7b65e5106404fc03c957ea8cf0afb7d3ab887b56d937'
MANIFEST = ROOT / 'tests/fixtures/reference-lazy-interop-recipes/recipes.json'
MANIFEST_SHA256 = 'd9b23abd540d4e4c2a18c1fd83795e114f59f23d7f12a1285651a77b420be1fe'


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
    require(path.is_file() and not path.is_symlink() and path.stat().st_size <= 512 << 10,
            'bounded regular manifest')
    data = path.read_bytes()
    value = decode(data)
    require(hashlib.sha256(data).hexdigest() == MANIFEST_SHA256, 'frozen recipe identity')
    require(GENERATOR.is_file() and not GENERATOR.is_symlink()
            and hashlib.sha256(GENERATOR.read_bytes()).hexdigest() == GENERATOR_SHA256,
            'generator identity before import')
    from oracles import lazy_interop as generator
    rebuilt = generator.manifest()
    require(json.dumps(value, sort_keys=True, allow_nan=False) ==
            json.dumps(rebuilt, sort_keys=True, allow_nan=False), 'independent source regeneration')
    require((value['recipe_count'], value['source_bytes'], value['classifications']) ==
            (886, 125863316, {'success': 811, 'unsupported': 43, 'resource': 32}), 'fixed envelope')
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', required=True)
    parser.add_argument('--manifest', type=Path, default=MANIFEST)
    args = parser.parse_args()
    value = check(args.manifest)
    print('Verified 886 source-only public-frame recipes / 125863316 bytes / '
          '811 admitted hypotheses / 43 unsupported / 32 resource errors / '
          f"{len(value['expected_rust_frame_equivalence'])} equivalence groups. No codec execution.")


if __name__ == '__main__':
    main()
