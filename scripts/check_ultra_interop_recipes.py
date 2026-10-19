#!/usr/bin/env python3
"""Verify frozen broader public-frame inputs without codec or child execution."""
from pathlib import Path
import argparse
import hashlib
import json

ROOT = Path(__file__).resolve().parents[1]
GENERATOR = ROOT / 'scripts/oracles/ultra_interop.py'
GENERATOR_SHA256 = 'c9d98ec9f8671d4e9a0596b4abdbda47282de93d1571718e4d74126ed6753108'
MANIFEST = ROOT / 'tests/fixtures/reference-ultra-interop-recipes/recipes.json'
MANIFEST_SHA256 = 'b498c6932e2fd85712608833dd7c745945cbea87d53bdfd009e2dae8a295fcd0'


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
    from oracles import ultra_interop as generator
    rebuilt = generator.manifest()
    require(json.dumps(value, sort_keys=True, allow_nan=False) ==
            json.dumps(rebuilt, sort_keys=True, allow_nan=False), 'independent source regeneration')
    require((value['recipe_count'], value['source_bytes'], value['classifications']) ==
            (1345, 146427068, {'success': 1222, 'unsupported': 15, 'resource': 108}), 'fixed envelope')
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', required=True)
    parser.add_argument('--manifest', type=Path, default=MANIFEST)
    args = parser.parse_args()
    value = check(args.manifest)
    print('Verified 1345 source-only public-frame recipes / 146427068 bytes / '
          '1222 admitted hypotheses / 15 unsupported / 108 resource errors / '
          f"{len(value['expected_rust_frame_equivalence'])} equivalence groups. No codec execution.")


if __name__ == '__main__':
    main()
