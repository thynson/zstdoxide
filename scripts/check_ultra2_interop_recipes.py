#!/usr/bin/env python3
"""Verify frozen broader public-frame inputs without codec or child execution."""
from pathlib import Path
import argparse
import hashlib
import json

ROOT = Path(__file__).resolve().parents[1]
GENERATOR = ROOT / 'scripts/oracles/ultra2_interop.py'
GENERATOR_SHA256 = '8ac8b792373196b449a2d9fa0d4eaf636c65d46dc5c01790d91fc2b5077319c4'
MANIFEST = ROOT / 'tests/fixtures/reference-ultra2-interop-recipes/recipes.json'
MANIFEST_SHA256 = '0ae6c7cfb5ab3530c52976139326514e9919bcba321d9e6516621ffd129f4dd5'


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
    require(path.is_file() and not path.is_symlink() and path.stat().st_size <= 2 << 20,
            'bounded regular manifest')
    data = path.read_bytes()
    value = decode(data)
    require(hashlib.sha256(data).hexdigest() == MANIFEST_SHA256, 'frozen recipe identity')
    require(GENERATOR.is_file() and not GENERATOR.is_symlink()
            and hashlib.sha256(GENERATOR.read_bytes()).hexdigest() == GENERATOR_SHA256,
            'generator identity before import')
    from oracles import ultra2_interop as generator
    rebuilt = generator.manifest()
    require(json.dumps(value, sort_keys=True, allow_nan=False) ==
            json.dumps(rebuilt, sort_keys=True, allow_nan=False), 'independent source regeneration')
    require((value['recipe_count'], value['source_bytes'], value['classifications']) ==
            (1433, 146881431, {'success': 1292, 'resource': 141}), 'fixed envelope')
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', required=True)
    parser.add_argument('--manifest', type=Path, default=MANIFEST)
    args = parser.parse_args()
    value = check(args.manifest)
    print('Verified 1433 source-only public-frame recipes / 146881431 bytes / '
          '1292 admitted hypotheses / 141 resource errors / '
          f"{len(value['expected_rust_frame_equivalence'])} equivalence groups. No codec execution.")


if __name__ == '__main__':
    main()
