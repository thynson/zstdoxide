#!/usr/bin/env python3
"""Freeze/check deterministic Optimal price inputs without a codec or children."""
import argparse
from dataclasses import replace
import json
from pathlib import Path
import sys
from oracles import optimal_price_recipes as price
from oracles.optimal_prices_v2 import parse_packet, require

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / 'tests/fixtures/reference-optimal-price-recipes/recipes.json'


def check(write=False):
    children = []

    def guard(event, args):
        if event in ('subprocess.Popen', 'os.system', 'os.posix_spawn', 'os.fork'):
            children.append(event)
            raise ValueError('child execution forbidden')

    sys.addaudithook(guard)
    recipes = price.recipes()
    manifest = price.manifest()
    require(manifest['recipe_count'] == 147 and manifest['source_bytes'] == 9835338
            and manifest['operation_count'] == 2018, 'frozen inventory')
    for recipe in recipes:
        require(parse_packet(recipe.request.packet()) == recipe.request, 'exact source/operation transport')
    queries = [op for recipe in recipes for op in recipe.request.operations if op[0] == 4]
    require(set(price.boundaries(price.LL_BITS, 0, 131072)) <= {q[3] for q in queries}, 'LL interval coverage')
    require(set(price.boundaries(price.ML_BITS, 3, 131072)) <= {q[5] for q in queries}, 'ML interval coverage')
    require({n for b in range(32) for n in ((1 << b) - 1, 1 << b, (1 << b) + 1)
             if 1 <= n <= 0xffffffff} <= {q[4] for q in queries}, 'offset interval coverage')
    require({r.request.literal_mode for r in recipes} == {0, 1}, 'literal modes')
    canonical = (json.dumps(manifest, sort_keys=True, indent=2) + '\n').encode()
    if write:
        require(not DEST.exists(), 'fresh recipe manifest')
        DEST.parent.mkdir(parents=True, exist_ok=True)
        DEST.write_bytes(canonical)
    require(DEST.is_file() and not DEST.is_symlink() and DEST.read_bytes() == canonical, 'exact frozen manifest')
    changes = [dict(name=''), dict(name='op_BAD'), dict(name='op_' + 'x' * 61),
               dict(name=1), dict(hypothesis=''), dict(hypothesis='x' * 257),
               dict(hypothesis=True), dict(request=None)]
    for change in changes:
        try:
            replace(recipes[0], **change).metadata()
        except ValueError:
            pass
        else:
            raise ValueError('accepted invalid metadata: ' + repr(change))
    require(not children, 'no child attempts')
    print(json.dumps({'status': 'passed', 'recipes': len(recipes), 'source_bytes': 9835338,
                      'operations': 2018, 'malformed_metadata_rejections': len(changes),
                      'child_attempts': len(children)}, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    check(parser.parse_args().write)
