"""Independent complete optLevel=0 statistics/price replay; no C or production Rust.

Pinned base distributions derive from facebook/zstd v1.5.7 zstd_opt.c. Symbol
intervals derive from the format extra-bit widths in zstd_internal.h. Attribution
is retained in NOTICE/LICENSE.zstd. All arithmetic is checked before use.
"""
import bisect
import json
from .optimal_prices_v2 import Request, REVISION, integer, require

LIMIT = (1 << 30) - 1
U32_MAX = (1 << 32) - 1
LL_WIDTHS = [0] * 16 + [1, 1, 1, 1, 2, 2, 3, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
ML_WIDTHS = [0] * 32 + [1, 1, 1, 1, 2, 2, 3, 3, 4, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
BASE_LL = [4, 2] + [1] * 34
BASE_OF = [6, 2, 1, 1, 2, 3, 4, 4, 4, 3, 2, 1] + [1] * 20
KEYS = ('lit', 'll', 'ml', 'of')


def bases(widths, first):
    result = []
    for width in widths:
        result.append(first)
        first += 1 << width
    return result


LL_START = bases(LL_WIDTHS, 0)
ML_START = bases(ML_WIDTHS, 3)


def weight(stat):
    integer(stat, 0, U32_MAX - 1, 'weight domain before increment')
    return ((stat + 1).bit_length() - 1) * 256


def finite(value):
    return integer(value, 0, LIMIT, 'finite price')


def fnv(source):
    value = 0xcbf29ce484222325
    for byte in source:
        value = ((value ^ byte) * 0x100000001b3) & ((1 << 64) - 1)
    return f'{value:016x}'


class Model:
    def __init__(self, mode):
        self.mode = mode
        self.tables = {k: [0] * n for k, n in zip(KEYS, (256, 36, 53, 32))}
        self.sums = dict.fromkeys(KEYS, 0)
        self.cache = dict.fromkeys(KEYS, 0)
        self.predefined = False

    def sum_table(self, key):
        self.sums[key] = integer(sum(self.tables[key]), 0, U32_MAX - 1, 'bounded frequency sum')

    def refresh(self):
        for key in KEYS:
            if key != 'lit' or self.mode == 0:
                self.cache[key] = weight(self.sums[key])

    def rescale(self, source):
        self.predefined = False
        if self.sums['ll'] == 0:
            self.predefined = len(source) <= 8
            if self.mode == 0:
                histogram = [0] * 256
                for byte in source:
                    histogram[byte] += 1
                self.tables['lit'] = [int(n > 0) + (n >> 8) for n in histogram]
                self.sum_table('lit')
            self.tables['ll'] = BASE_LL.copy()
            self.tables['ml'] = [1] * 53
            self.tables['of'] = BASE_OF.copy()
            for key in ('ll', 'ml', 'of'):
                self.sum_table(key)
        else:
            for key in KEYS:
                if key == 'lit' and self.mode == 1:
                    continue
                factor = sum(self.tables[key]) >> (12 if key == 'lit' else 11)
                if factor > 1:
                    shift = factor.bit_length() - 1
                    require(shift < 30, 'bounded downscale')
                    self.tables[key] = [1 + (n >> shift) for n in self.tables[key]]
                self.sum_table(key)
        self.refresh()

    def update(self, literals, offset, length):
        if self.mode == 0:
            for byte in literals:
                self.tables['lit'][byte] = integer(self.tables['lit'][byte] + 2, 0, U32_MAX - 1, 'literal increment')
            self.sums['lit'] = integer(self.sums['lit'] + 2 * len(literals), 0, U32_MAX - 1, 'literal sum increment')
        for key, code in [('ll', bisect.bisect_right(LL_START, len(literals)) - 1),
                          ('ml', bisect.bisect_right(ML_START, length) - 1),
                          ('of', offset.bit_length() - 1)]:
            require(0 <= code < len(self.tables[key]), 'symbol domain')
            self.tables[key][code] = integer(self.tables[key][code] + 1, 0, U32_MAX - 1, 'symbol increment')
            self.sums[key] = integer(self.sums[key] + 1, 0, U32_MAX - 1, 'symbol sum increment')

    def literal_price(self, literals):
        if not literals:
            return 0
        if self.mode == 1:
            return finite(len(literals) * 8 * 256)
        if self.predefined:
            return finite(len(literals) * 6 * 256)
        base = self.cache['lit']
        require(base >= 256, 'nonzero literal base')
        return finite(sum(base - min(weight(self.tables['lit'][byte]), base - 256) for byte in literals))

    def ll_price(self, length):
        if self.predefined:
            return weight(length)
        if length == 131072:
            return finite(256 + self.ll_price(length - 1))
        code = bisect.bisect_right(LL_START, length) - 1
        return finite(LL_WIDTHS[code] * 256 + self.cache['ll'] - weight(self.tables['ll'][code]))

    def match_price(self, offset, length):
        off = offset.bit_length() - 1
        if self.predefined:
            return finite(weight(length - 3) + (16 + off) * 256)
        ml = bisect.bisect_right(ML_START, length) - 1
        return finite(off * 256 + self.cache['of'] - weight(self.tables['of'][off])
                      + max(0, off - 19) * 2 * 256
                      + ML_WIDTHS[ml] * 256 + self.cache['ml'] - weight(self.tables['ml'][ml]) + 51)

    def record(self, index, tag, query):
        for key in KEYS:
            require(sum(self.tables[key]) == self.sums[key], 'table/sum agreement')
        return dict(record='operation', index=index, tag=tag,
                    price_type='predefined' if self.predefined else 'dynamic', literal_mode=self.mode,
                    frequencies={k: v.copy() for k, v in self.tables.items()}, sums=self.sums.copy(),
                    base_prices=self.cache.copy(), query=query)


def observations(request):
    require(type(request) is Request, 'bounded request')
    request.validate()
    state = Model(request.literal_mode)
    source_identity = fnv(request.source)
    yield dict(record='request', schema_version=2, revision=REVISION, source_bytes=len(request.source),
               source_fnv1a64=source_identity, operations=len(request.operations), literal_mode=request.literal_mode)
    for index, (tag, a, b, c, d, e) in enumerate(request.operations):
        query = None
        if tag in (0, 3):
            state.rescale(request.source[a:a + b])
        elif tag in (1, 5):
            for _ in range(e if tag == 5 else 1):
                state.update(request.source[a:a + b], c, d)
        elif tag == 2:
            state.refresh()
        else:
            query = [state.literal_price(request.source[a:a + b]), state.ll_price(c), state.match_price(d, e)]
        yield state.record(index, tag, query)
    yield dict(record='end', operations=len(request.operations), source_fnv1a64=source_identity)


def unique(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON key')
        result[key] = value
    return result


def invalid_constant(value):
    raise ValueError('nonfinite JSON constant: ' + value)


def validate_output(data, request):
    require(type(data) is bytes and 0 < len(data) <= 2 << 20, 'observation byte limit')
    lines = data.splitlines()
    require(len(lines) == len(request.operations) + 2, 'exact record count')
    for raw, expected in zip(lines, observations(request)):
        actual = json.loads(raw, object_pairs_hook=unique, parse_constant=invalid_constant)
        require(json.dumps(actual, sort_keys=True, separators=(',', ':')) ==
                json.dumps(expected, sort_keys=True, separators=(',', ':')), 'complete independent observation mismatch')
    return len(request.operations)
