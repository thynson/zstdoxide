#!/usr/bin/env python3
"""Check independent Optimal price replay and forged observations without C."""
import copy
import json
import sys
from oracles.optimal_prices_v2 import Request, require
from oracles.optimal_price_recipes import recipes
from oracles import optimal_price_validation as model


def encode(rows):
    return b''.join((json.dumps(row, separators=(',', ':')) + '\n').encode() for row in rows)


def main():
    children = []

    def guard(event, args):
        if event in ('subprocess.Popen', 'os.system', 'os.posix_spawn', 'os.fork'):
            children.append(event)
            raise ValueError('child execution forbidden')

    sys.addaudithook(guard)
    require([model.weight(n) for n in (0, 1, 2, 3, 7, 8, 15, 16)] ==
            [0, 256, 256, 512, 768, 768, 1024, 1024], 'hand-checked whole-bit weights')
    s = model.Model(0)
    s.rescale(b'a' * 9)
    require(s.sums == dict(lit=1, ll=40, ml=53, of=53), 'hand-checked initial sums')
    require(s.literal_price(b'a' * 9) == 2304 and s.ll_price(0) == 768
            and s.ll_price(1) == 1024 and s.ll_price(131072) == 5376
            and s.match_price(1, 3) == 1843 and s.match_price(1 << 20, 131072) == 11827,
            'hand-checked dynamic scalar prices')
    s.rescale(b'')
    require(s.literal_price(b'!') == 256, 'minimum one-bit literal with absent symbol')
    for _ in range(4096):
        s.update(b'', 1, 3)
    require(s.cache['ll'] == 1280 and s.sums['ll'] == 4136, 'intentional stale cache')
    s.refresh()
    require(s.cache['ll'] == 3072, 'explicit refresh')
    s.rescale(b'')
    require(s.sums['ll'] == 2087 and s.sums['ml'] == 2101, 'hand-checked downscale base-one rule')
    s = model.Model(0)
    s.rescale(b'a' * 8)
    require(s.predefined and s.literal_price(b'a' * 8) == 12288 and s.ll_price(0) == 0
            and s.match_price(1, 3) == 4096, 'hand-checked predefined prices')
    s = model.Model(1)
    s.rescale(b'a' * 9)
    require(s.literal_price(b'a' * 9) == 18432 and s.sums['lit'] == 0
            and s.cache['lit'] == 0, 'disabled literal ownership')
    operations = 0
    for recipe in recipes():
        observed = list(model.observations(recipe.request))
        operations += model.validate_output(encode(observed), recipe.request)
    request = Request(b'a' * 9, ((0, 0, 9, 0, 0, 0), (1, 0, 9, 1, 3, 0),
                                (2, 0, 0, 0, 0, 0), (4, 0, 9, 131072, 1, 3)))
    original = list(model.observations(request))
    rejected = []

    def reject(name, data):
        try:
            model.validate_output(data, request)
        except (ValueError, TypeError, KeyError):
            rejected.append(name)
        else:
            raise ValueError('accepted forgery: ' + name)

    mutations = [
        ('literal-array', lambda r: r[1]['frequencies']['lit'].__setitem__(97, 0)),
        ('LL-array', lambda r: r[1]['frequencies']['ll'].__setitem__(0, 5)),
        ('ML-array', lambda r: r[1]['frequencies']['ml'].__setitem__(0, 2)),
        ('OF-array', lambda r: r[1]['frequencies']['of'].__setitem__(0, 7)),
        ('sum', lambda r: r[1]['sums'].__setitem__('lit', 2)),
        ('coherent-array-sum', lambda r: (r[1]['frequencies']['lit'].__setitem__(97, 2), r[1]['sums'].__setitem__('lit', 2))),
        ('early-refresh', lambda r: r[2]['base_prices'].__setitem__('lit', 1024)),
        ('missing-refresh', lambda r: r[3]['base_prices'].__setitem__('lit', 256)),
        ('raw-price', lambda r: r[4]['query'].__setitem__(0, 0)),
        ('LL-price', lambda r: r[4]['query'].__setitem__(1, 0)),
        ('match-price', lambda r: r[4]['query'].__setitem__(2, 0)),
        ('predefined', lambda r: r[1].__setitem__('price_type', 'predefined')),
        ('boolean-mode', lambda r: r[1].__setitem__('literal_mode', False)),
        ('floating-index', lambda r: r[1].__setitem__('index', 0.0)),
        ('source-identity', lambda r: r[0].__setitem__('source_fnv1a64', '0' * 16)),
        ('end-count', lambda r: r[-1].__setitem__('operations', 3)),
        ('extra-field', lambda r: r[1].__setitem__('extra', 1)),
        ('missing-field', lambda r: r[1].pop('sums')),
        ('record-order', lambda r: r.__setitem__(slice(1, 3), [r[2], r[1]])),
    ]
    for name, change in mutations:
        rows = copy.deepcopy(original)
        change(rows)
        reject(name, encode(rows))
    valid = encode(original)
    reject('duplicate-key', valid.replace(b'"index":0', b'"index":0,"index":0', 1))
    reject('extra-record', valid + b'{}\n')
    reject('missing-record', encode(original[:-1]))
    reject('nonfinite', valid.replace(b'"index":0', b'"index":NaN', 1))
    require(len(rejected) == 23 and operations == 2018 and not children, 'complete controls')
    print(json.dumps({'status': 'passed', 'recipes_replayed': 147, 'operations': operations,
                      'forgery_rejections': rejected, 'child_attempts': len(children),
                      'qualification': 'Independent model self-consistency and hand-checked scalar identities; not a C capture.'}, indent=2))


if __name__ == '__main__':
    main()
