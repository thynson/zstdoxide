#!/usr/bin/env python3
"""Check fixed ordinary DoubleFast input hypotheses without any C execution."""

from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import sys

ROOT = Path(__file__).resolve().parents[1]
GENERATOR = ROOT / 'scripts/oracles/double_fast_frames.py'
GENERATOR_SHA256 = 'ac10609d7fe7c0f4983899238dc8a852ca437e35aff42b317599a0286c8ea656'
MANIFEST = ROOT / 'tests/fixtures/reference-double-fast-frame-recipes/recipes.json'
MANIFEST_SHA256 = '1ac2fe6ddfe66aef7a72f387f0fb8db6e1109ef137f33c1ef63b8496f5fa0667'


def require(valid, reason):
    if not valid:
        raise ValueError(reason)


def load_generator():
    require(hashlib.sha256(GENERATOR.read_bytes()).hexdigest() == GENERATOR_SHA256, 'frozen generator identity')
    spec = importlib.util.spec_from_file_location('zstdoxide_double_fast_frame_recipes', GENERATOR)
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


def check(path):
    require(path.is_file() and not path.is_symlink() and path.stat().st_size <= 256 << 10, 'regular bounded manifest')
    data = path.read_bytes()
    value = decode(data)
    require(hashlib.sha256(data).hexdigest() == MANIFEST_SHA256, 'frozen input manifest')
    module = load_generator()
    rebuilt = module.manifest()
    require(json.dumps(value, sort_keys=True, allow_nan=False) == json.dumps(rebuilt, sort_keys=True, allow_nan=False),
            'independent source regeneration')
    require((rebuilt['recipe_count'], rebuilt['source_bytes'], rebuilt['scheduled_calls'],
             rebuilt['positive_calls'], rebuilt['whole_comparisons'], len(rebuilt['header_only']))
            == (51, 6348944, 164, 160, 7, 13), 'fixed input envelope')
    return rebuilt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', required=True)
    parser.add_argument('--manifest', type=Path, default=MANIFEST)
    args = parser.parse_args()
    check(args.manifest)
    print('Verified 51 source-only frame recipes / 164 scheduled calls / 13 incomplete-header probes; no C execution.')


if __name__ == '__main__':
    main()
