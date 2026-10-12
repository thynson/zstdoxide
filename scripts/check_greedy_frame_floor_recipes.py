#!/usr/bin/env python3
"""Check additive ordinary tiny-window-floor inputs without codec execution."""

from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
FOUNDATION = ROOT / 'scripts/check_greedy_frame_recipes.py'
FOUNDATION_SHA256 = '9f743ed73771914f1d2e44cdc811fb9a05a59dc44ce40e19f09bff5489a8a700'
GENERATOR = ROOT / 'scripts/oracles/greedy_frame_floor_recipes.py'
GENERATOR_SHA256 = 'e30cb2c60762b333d6e4b510e9c9a5384c43a20e93209e89d52716f099bae2c4'
MANIFEST = ROOT / 'tests/fixtures/reference-greedy-frame-floor-recipes/recipes.json'
MANIFEST_SHA256 = '8414ea9f5f9eee715b22f473023a299331df5f8abe11f974202bda03af58cfbd'


def require(valid, reason):
    if not valid:
        raise ValueError(reason)


def load(path, digest, name):
    require(path.is_file() and not path.is_symlink() and path.stat().st_size <= 64 << 10,
            'regular bounded Python input')
    require(hashlib.sha256(path.read_bytes()).hexdigest() == digest, 'frozen source identity')
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def load_generator():
    return load(GENERATOR, GENERATOR_SHA256, 'zstdoxide_greedy_floor_recipes')


def check(path=MANIFEST):
    foundation = load(FOUNDATION, FOUNDATION_SHA256, 'zstdoxide_greedy_frame_checker')
    data = foundation.bounded_file(path, 64 << 10)
    value = foundation.decode(data)
    require(hashlib.sha256(data).hexdigest() == MANIFEST_SHA256, 'frozen additive manifest')
    module = load_generator()
    rebuilt = module.manifest()
    require(json.dumps(value, sort_keys=True, allow_nan=False) == json.dumps(rebuilt, sort_keys=True, allow_nan=False),
            'independent source regeneration')
    require((rebuilt['recipe_count'], rebuilt['source_bytes'], rebuilt['scheduled_calls'], rebuilt['positive_calls'], rebuilt['branches'])
            == (12, 405942, 48, 48, {'chain': 6, 'row': 6}), 'fixed additive envelope')
    foundation.check()
    original = {r.name for r in foundation.load_generator().recipes()}
    cases = module.recipes()
    require(not original.intersection(r.name for r in cases), 'distinct additive case identities')
    for index, case in enumerate(cases):
        tiny = index % 6 + 1
        size = 1 << case.window_log
        require(case.schedule == (size, size, tiny, 33) and case.source[:2 * size + tiny] == b'Z' * (2 * size + tiny),
                'uniform windows and bounded tiny boundary')
    return rebuilt


def controls():
    rejected = 0
    with tempfile.TemporaryDirectory(prefix='zstdoxide-greedy-floor-inputs-') as directory:
        root = Path(directory)
        original = json.loads(MANIFEST.read_bytes())
        for field, value in [('recipe_count', True), ('recipe_count', 11), ('source_bytes', 0),
                             ('scheduled_calls', 47), ('positive_calls', 47), ('branches', {'chain': 7, 'row': 5})]:
            changed = dict(original); changed[field] = value
            path = root / (field + '.json'); path.write_text(json.dumps(changed))
            try:
                check(path)
            except ValueError:
                rejected += 1
            else:
                raise ValueError('changed metadata accepted')
        path = root / 'linked.json'; path.symlink_to(MANIFEST)
        try:
            check(path)
        except ValueError:
            rejected += 1
        else:
            raise ValueError('symlink accepted')
        path = root / 'generator.py'; path.write_bytes(GENERATOR.read_bytes() + b'\n')
        try:
            load(path, GENERATOR_SHA256, 'must_not_execute')
        except ValueError:
            rejected += 1
        else:
            raise ValueError('changed generator accepted')
    require(rejected == 8, 'all additive rejection controls')
    return {'rejected_controls': rejected, 'codec_executions': 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', required=True)
    parser.add_argument('--manifest', type=Path, default=MANIFEST)
    parser.add_argument('--controls', action='store_true')
    args = parser.parse_args()
    check(args.manifest)
    print('Verified 12 additive floor recipes / 48 positive calls; original 126 recipes unchanged; no codec execution.')
    if args.controls:
        print(json.dumps(controls(), sort_keys=True))


if __name__ == '__main__':
    main()
