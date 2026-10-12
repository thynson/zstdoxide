#!/usr/bin/env python3
"""Source-only ordinary DoubleFast frame recipes; never run or predict a codec."""

from dataclasses import dataclass, replace
import hashlib
import re

REVISION = "f8745da6ff1ad1e7bab384bd1f9d742439278e99"
MAX_SOURCE = 4 << 20
MAX_BLOCK = 128 << 10
MAX_CALLS = 64
HEADERS = tuple((n, 21) for n in (255, 256, 65791, 65792, (1 << 32) - 2, (1 << 32) - 1, 1 << 32))
HEADERS += ((1024, 10), (1025, 10), (1 << 20, 20), ((1 << 20) + 1, 20),
            (1 << 21, 21), ((1 << 21) + 1, 21))


def require(valid, reason):
    if not valid:
        raise ValueError(reason)


def noise(seed, count):
    """Fixed xorshift32 bytes, matching the previously attributed Fast scaffold."""
    out = bytearray()
    for _ in range(count):
        seed ^= (seed << 13) & 0xffffffff
        seed ^= seed >> 17
        seed ^= (seed << 5) & 0xffffffff
        seed &= 0xffffffff
        out.append(seed & 255)
    return bytes(out)


def repeated(seed, period, count):
    unit = noise(seed, period)
    return (unit * ((count + period - 1) // period))[:count]


@dataclass(frozen=True)
class Recipe:
    name: str
    source: bytes
    schedule: tuple
    hypothesis: str
    window_log: int = 19
    long_log: int = 14
    short_log: int = 12
    search_log: int = 1
    min_match: int = 4
    target: int = 0
    whole_compare: bool = False

    def metadata(self):
        require(type(self.name) is str and re.fullmatch('[a-z][a-z0-9_]{0,62}', self.name), 'recipe name')
        require(type(self.source) is bytes and len(self.source) <= MAX_SOURCE, 'source cap/type')
        require(type(self.hypothesis) is str and 0 < len(self.hypothesis) <= 256, 'hypothesis')
        require(type(self.schedule) is tuple and len(self.schedule) <= MAX_CALLS, 'schedule type/count')
        for value, lower, upper in [(self.window_log, 10, 21), (self.long_log, 6, 18),
                                     (self.short_log, 6, 18), (self.search_log, 1, 29),
                                     (self.min_match, 3, 7), (self.target, 0, 131072)]:
            require(type(value) is int and lower <= value <= upper, 'parameter domain/type')
        block_limit = min(MAX_BLOCK, 1 << self.window_log, max(1, len(self.source)))
        require(all(type(n) is int and 0 <= n <= block_limit for n in self.schedule), 'scheduled block extent/type')
        require(sum(self.schedule) == len(self.source), 'schedule source coverage')
        require(type(self.whole_compare) is bool, 'whole comparison flag')
        positive = [n for n in self.schedule if n]
        if self.whole_compare:
            require(positive and all(n == block_limit for n in positive[:-1])
                    and 0 < positive[-1] <= block_limit, 'whole comparison block alignment')
        return {'id': self.name, 'source_bytes': len(self.source),
                'source_sha256': hashlib.sha256(self.source).hexdigest(), 'schedule': list(self.schedule),
                'parameters': [self.window_log, self.short_log, self.long_log, self.search_log,
                               self.min_match, self.target, 2],
                'whole_compare': self.whole_compare, 'hypothesis': self.hypothesis}


def recipes():
    periodic = repeated(0x13579bdf, 257, 4096)
    a, b = noise(0x2468ace1, MAX_BLOCK), noise(0xfedcba91, MAX_BLOCK)
    literals = bytes(x & 7 for x in noise(0x55667789, 4096)) + noise(0x19283745, 1024)
    literals += bytes(x & 7 for x in noise(0x89abcdef, 4096))
    rows = []

    def add(name, source, schedule, hypothesis, **params):
        row = Recipe(name, source, tuple(schedule), hypothesis, **params)
        row.metadata()
        rows.append(row)

    add('empty', b'', [], 'Complete empty frame; no scheduled candidate.')
    add('first_uniform', b'Z' * 4096, [4096], 'First uniform block must not use the later-block RLE conversion.', whole_compare=True)
    add('zero_uniform', b'Z' * 4096, [0, 4096, 0, 0], 'Empty calls before and after first publication preserve candidate state.')
    add('compressed_uniform_repeat', periodic + b'\xa6' * 4096 + periodic, [4096] * 3,
        'Uniform middle-block fallback and later copy with committed repeats.')
    add('raw_then_copy', a + a, [MAX_BLOCK] * 2, 'Incompressible first block retains sampled history for its copy.',
        long_log=18, short_log=16, whole_compare=True)
    add('mixed_fallback', periodic + noise(0x7654321f, 4096) + b'7' * 4096 + periodic, [4096] * 4,
        'Compressed/raw/RLE hypotheses in one ordinary context.')
    pieces = [bytes([0x90 + n]) * n for n in range(1, 11)]
    add('tiny_stale', periodic + b''.join(pieces) + periodic, [4096, *range(1, 11), 0, 4096],
        'Lengths1..6 stale scratch versus7..10 fresh candidates, then history reuse.')
    for name, target, search in [('literal_tree', 0, 1), ('literal_target4', 4, 1),
                                  ('literal_target_max', 131072, 1), ('literal_search29', 0, 29)]:
        add(name, literals, [4096, 1024, 4096], 'Positive target/search fields must not impose Fast-only raw-literal acceleration.',
            min_match=7, target=target, search_log=search)
    for window, name in [(17, 'window_expiry17'), (19, 'window_retained19')]:
        add(name, a + b + a, [MAX_BLOCK] * 3, 'A/B/A history visibility across the selected window.',
            window_log=window, long_log=18, short_log=16, whole_compare=True)
    medium = a + noise(0x76543210, 1 << 20) + a
    for window, name in [(20, 'window_expiry20'), (21, 'window_retained21')]:
        add(name, medium, [MAX_BLOCK] * (len(medium) // MAX_BLOCK), 'A/1MiB-noise/A distinguishes window20 and21 history.',
            window_log=window, long_log=18, short_log=16, whole_compare=True)
    large = a + noise(0x89abcdef, 2 << 20) + a
    add('window_expiry21', large, [MAX_BLOCK] * (len(large) // MAX_BLOCK), 'A/2MiB-noise/A advances even the window21 prefix floor.',
        window_log=21, long_log=18, short_log=16, whole_compare=True)
    for length in range(1, 11):
        add(f'fresh_length{length}', b'abcdefghij'[:length], [length], 'Fresh ordinary tiny entry without a prior prefix.',
            window_log=10, long_log=6, short_log=6)
    for length in range(7, 11):
        add(f'prefix_length{length}', periodic + periodic[:length], [4096, length],
            'Fresh short parser call with a retained prefix and committed repeats.')
    mixed = periodic + noise(0x7654321f, 4096) + periodic
    for width in range(3, 8):
        add(f'short_width{width}', mixed, [4096] * 3, 'Short-key dispatch, with minMatch3/4 equivalence as an input hypothesis.',
            min_match=width)
    for long_log, short_log in [(6, 18), (18, 6), (6, 7), (7, 6)]:
        add(f'table_l{long_log}_s{short_log}', mixed, [4096] * 3, 'Independent table dimensions and collision domains.',
            long_log=long_log, short_log=short_log)
    for length in (4, 8, 12):
        for distance in (128, 192, 256, 320):
            middle = bytearray(noise(0xabcdef01, 1024))
            middle[distance:distance + length] = middle[32:32 + length]
            add(f'raw_candidate_k{length}_d{distance}', periodic + middle + periodic, [4096, 1024, 4096],
                'Potential raw candidate with changed pending repeats; retain non-witnesses.', long_log=18, short_log=16)
    require(len(rows) == len({row.name for row in rows}) == 51, 'unique recipe cardinality')
    return rows


def manifest():
    rows = [row.metadata() for row in recipes()]
    return {'schema_version': 1, 'upstream_revision': REVISION, 'status': 'source-only pre-execution hypotheses',
            'recipes': rows, 'recipe_count': len(rows), 'source_bytes': sum(row['source_bytes'] for row in rows),
            'scheduled_calls': sum(len(row['schedule']) for row in rows),
            'positive_calls': sum(sum(n > 0 for n in row['schedule']) for row in rows),
            'whole_comparisons': sum(row['whole_compare'] for row in rows),
            'header_only': [{'pledge': n, 'window_log': w, 'completed_frame': False} for n, w in HEADERS]}
