#!/usr/bin/env python3
"""Source-only ordinary Greedy recipes; never run or predict a codec."""

from dataclasses import dataclass
import hashlib
import re

REVISION = 'f8745da6ff1ad1e7bab384bd1f9d742439278e99'
MAX_SOURCE = 4 << 20
MAX_BLOCK = 128 << 10
MAX_CALLS = 64
HEADERS = tuple((n, 21) for n in (255, 256, 65791, 65792, (1 << 32) - 2,
                                (1 << 32) - 1, 1 << 32))
HEADERS += ((1024, 10), (1025, 10), (1 << 14, 14), ((1 << 14) + 1, 14),
            (1 << 15, 15), ((1 << 15) + 1, 15),
            (1 << 21, 21), ((1 << 21) + 1, 21))


def require(valid, reason):
    if not valid:
        raise ValueError(reason)


def noise(seed, count):
    """Fixed xorshift32, preserving the earlier Fast/DoubleFast source recipe."""
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
    window_log: int = 15
    hash_log: int = 10
    chain_log: int = 10
    search_log: int = 4
    min_match: int = 4
    target: int = 0
    whole_compare: bool = False

    def metadata(self):
        require(type(self.name) is str and re.fullmatch('[a-z][a-z0-9_]{0,62}', self.name), 'recipe name')
        require(type(self.source) is bytes and len(self.source) <= MAX_SOURCE, 'source cap/type')
        require(type(self.hypothesis) is str and 0 < len(self.hypothesis) <= 256, 'hypothesis')
        require(type(self.schedule) is tuple and len(self.schedule) <= MAX_CALLS, 'schedule type/count')
        for value, lower, upper in [(self.window_log, 10, 21), (self.hash_log, 6, 19),
                                     (self.chain_log, 6, 29), (self.search_log, 1, 29),
                                     (self.min_match, 3, 7), (self.target, 0, 131072)]:
            require(type(value) is int and lower <= value <= upper, 'parameter domain/type')
        row = self.window_log > 14
        require(row or (self.chain_log <= 18 and self.search_log <= 8), 'selected chain resource cap')
        block_limit = min(MAX_BLOCK, 1 << self.window_log, max(1, len(self.source)))
        require(all(type(n) is int and 0 <= n <= block_limit for n in self.schedule), 'scheduled extent/type')
        require(sum(self.schedule) == len(self.source), 'schedule source coverage')
        require(type(self.whole_compare) is bool, 'whole comparison flag')
        positive = [n for n in self.schedule if n]
        if self.whole_compare:
            require(positive and all(n == block_limit for n in positive[:-1])
                    and 0 < positive[-1] <= block_limit, 'whole comparison alignment')
        return {'id': self.name, 'source_bytes': len(self.source),
                'source_sha256': hashlib.sha256(self.source).hexdigest(), 'schedule': list(self.schedule),
                'parameters': [self.window_log, self.hash_log, self.chain_log, self.search_log,
                               self.min_match, self.target, 3],
                'selected_branch': 'row' if row else 'chain',
                'whole_compare': self.whole_compare, 'hypothesis': self.hypothesis}


def recipes():
    periodic = repeated(0x13579bdf, 257, 4096)
    random = noise(0x2468ace1, 4096)
    literal_tree = bytes(x & 7 for x in noise(0x55667789, 4096))
    literal_tree += noise(0x19283745, 1024) + bytes(x & 7 for x in noise(0x89abcdef, 4096))
    rows = []

    def add(name, source, schedule, hypothesis, **params):
        row = Recipe(name, source, tuple(schedule), hypothesis, **params)
        row.metadata()
        rows.append(row)

    for label, window in [('chain', 14), ('row', 15)]:
        def branch(suffix, source, schedule, hypothesis, **params):
            add(label + '_' + suffix, source, schedule, hypothesis, window_log=window, **params)

        branch('empty', b'', [], 'Complete empty frame without a parser candidate.')
        branch('first_uniform', b'Z' * 4096, [4096], 'First uniform block and candidate publication.', whole_compare=True)
        branch('zero_uniform', b'Z' * 4096, [0, 4096, 0, 0], 'Zero calls preserve source and publication, including after the first block.')
        branch('mixed_fallback', periodic + random + b'7' * 4096 + periodic, [4096] * 4,
               'Compressed/raw/RLE hypotheses and committed-repeat use after fallback.')
        branch('raw_then_copy', random + random, [4096] * 2,
               'Incompressible first block retains sampled history for a later copy.', hash_log=14, chain_log=12)
        pieces = [bytes([0x90 + n]) * n for n in range(1, 19)]
        branch('tiny_stale', periodic + b''.join(pieces) + periodic, [4096, *range(1, 19), 0, 4096],
               'Tiny stale scratch versus fresh candidates and distinct parser tail boundaries.')
        for length in [1, 6, 7, 8, 9, 10, 15, 16, 17, 18]:
            branch(f'fresh_length{length}', bytes(range(length)), [length],
                   'Fresh tiny entry with no prior prefix.', hash_log=6, chain_log=6)
        for length in [7, 8, 9, 10, 15, 16, 17]:
            branch(f'prefix_length{length}', periodic + periodic[:length], [4096, length],
                   'Retained-prefix entry at strict chain/row search-tail boundaries.')
        for gap in [383, 384, 385, 575, 576, 577]:
            tail = noise(0xabcdef01, 33)
            branch(f'ordinary_gap{gap}', b'Z' * gap + tail, [gap, 33],
                   'Uniform repeat-first candidate may leave a frontier gap; observe ordinary adjustment separately from direct row update.')
        mixed = periodic + random + periodic
        for minimum in range(3, 8):
            branch(f'width{minimum}', mixed, [4096] * 3,
                   'Greedy clamped minimum-match dispatch across emitted/fallback blocks.', min_match=minimum)
        for target in [0, 4, 131072]:
            branch(f'literal_target{target}', literal_tree, [4096, 1024, 4096],
                   'Target length must not introduce Fast-only literal acceleration.', min_match=7, target=target)
        for length in [4, 8, 12]:
            for distance in [128, 192, 256, 320]:
                middle = bytearray(noise(0xabcdef01, 1024))
                middle[distance:distance + length] = middle[32:32 + length]
                branch(f'raw_candidate_k{length}_d{distance}', periodic + middle + periodic, [4096, 1024, 4096],
                       'Potential raw candidate changes pending repeats; preserve all non-witnesses.', hash_log=18, chain_log=16)
        for h, c in [(6, 18), (19, 6), (6, 7), (7, 6)]:
            branch(f'layout_h{h}_c{c}', mixed, [4096] * 3,
                   'Independent owner extents and collision domains.', hash_log=h, chain_log=c)
        branch('skip_then_period', noise(0x5eed1234, 4096) + b'abcd' * 1024, [8192],
               'Incompressible skipping followed by a match resets the skipping flag.')

    mixed = periodic + random + periodic
    for search in [1, 4, 5, 6, 7, 29]:
        add(f'row_search{search}', mixed, [4096] * 3,
            'Row widths16/32/64 and capped visits across an ordinary context.', search_log=search)
    add('row_unused_chain29', mixed, [4096] * 3,
        'Inactive chain dimension must not allocate a chain owner.', chain_log=29, search_log=29)
    a, b = noise(0x2468ace1, MAX_BLOCK), noise(0xfedcba91, MAX_BLOCK)
    for window in [17, 19]:
        add(f'row_window{window}', a + b + a, [MAX_BLOCK] * 3,
            'A/B/A retained-prefix visibility versus window expiry.', window_log=window, hash_log=18, whole_compare=True)
    medium = a + noise(0x76543210, 1 << 20) + a
    for window in [20, 21]:
        add(f'row_window{window}', medium, [MAX_BLOCK] * (len(medium) // MAX_BLOCK),
            'A/1MiB-noise/A distinguishes retention across windows20/21.', window_log=window, hash_log=19, whole_compare=True)
    large = a + noise(0x89abcdef, 2 << 20) + a
    add('row_window21_expired', large, [MAX_BLOCK] * (len(large) // MAX_BLOCK),
        'A/2MiB-noise/A advances even the largest admitted prefix floor.', window_log=21, hash_log=19, whole_compare=True)
    a14, b14 = a[:1 << 14], b[:1 << 14]
    add('chain_window14_expired', a14 + b14 + a14, [1 << 14] * 3,
        'Chain history expires at its largest automatic-branch window.', window_log=14, hash_log=14, chain_log=14, whole_compare=True)
    for length in [65535, 65536, 65543, 131072]:
        add(f'row_long_match{length}', (b'abcd' * ((length + 3) // 4))[:length], [length],
            'Long match marker and block bound within an ordinary call.', window_log=17, whole_compare=True)
    prefix = noise(0x1234abcd, 65536)
    add('row_long_literals', prefix + b'abcd' * 1024, [len(prefix) + 4096],
        'Potential extended literal length followed by a periodic match.', window_log=17, hash_log=12, whole_compare=True)
    require(len(rows) == len({row.name for row in rows}), 'unique recipe names')
    return rows


def manifest():
    rows = [row.metadata() for row in recipes()]
    return {'schema_version': 1, 'upstream_revision': REVISION,
            'status': 'source-only pre-execution hypotheses', 'recipes': rows,
            'recipe_count': len(rows), 'source_bytes': sum(row['source_bytes'] for row in rows),
            'scheduled_calls': sum(len(row['schedule']) for row in rows),
            'positive_calls': sum(sum(n > 0 for n in row['schedule']) for row in rows),
            'whole_comparisons': sum(row['whole_compare'] for row in rows),
            'branches': {branch: sum(row['selected_branch'] == branch for row in rows) for branch in ['chain', 'row']},
            'header_only': [{'pledge': n, 'window_log': w, 'completed_frame': False} for n, w in HEADERS]}
