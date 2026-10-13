"""Deterministic source-only hypotheses for complete Optimal price lifecycles.

Symbol extra-bit constants derive from facebook/zstd v1.5.7 zstd_internal.h;
upstream BSD attribution is retained in NOTICE and LICENSE.zstd. No observations
or expected price results are generated here.
"""
from dataclasses import dataclass
import re
from .optimal_prices_v2 import Request, REVISION, digest, require

LL_BITS = (0,) * 16 + (1, 1, 1, 1, 2, 2, 3, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
ML_BITS = (0,) * 32 + (1, 1, 1, 1, 2, 2, 3, 3, 4, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)


@dataclass(frozen=True)
class Recipe:
    name: str
    request: Request
    hypothesis: str

    def metadata(self):
        require(type(self.name) is str and re.fullmatch('op_[a-z0-9_]{1,60}', self.name), 'recipe name')
        require(type(self.hypothesis) is str and 0 < len(self.hypothesis) <= 256, 'hypothesis')
        require(type(self.request) is Request, 'bounded request type')
        packet = self.request.packet()
        return dict(id=self.name, source_bytes=len(self.request.source),
                    source_sha256=digest(self.request.source), packet_bytes=len(packet),
                    packet_sha256=digest(packet), literal_mode=self.request.literal_mode,
                    operations=[list(op) for op in self.request.operations], hypothesis=self.hypothesis)


def boundaries(bits, first, last):
    positions = {first, last}
    base = first
    for width in bits:
        positions.update(n for n in (base - 1, base, base + 1, base + (1 << width) - 1)
                         if first <= n <= last)
        base += 1 << width
    return sorted(positions)


def recipes():
    result = []

    def add(name, source, operations, hypothesis, mode=0):
        result.append(Recipe('op_' + name, Request(source, tuple(operations), mode), hypothesis))

    refresh = (2, 0, 0, 0, 0, 0)
    for length in (0, 1, 7, 8, 9, 15, 255, 256, 257, 65535, 65536, 131071, 131072):
        for pattern in ('single', 'cycle', 'skewed'):
            source = bytes((97 if pattern == 'single' or (pattern == 'skewed' and i % 17) else i % 256)
                           for i in range(length))
            for mode in (0, 1):
                query = (4, 0, length, length, 1, 3)
                operations = [(0, 0, length, 0, 0, 0), query,
                              (1, 0, min(length, 15), 4, 35, 0), refresh, query,
                              (3, 0, length, 0, 0, 0), query]
                add(f'init_{pattern}_{length}_m{mode}', source, operations,
                    'First-block histogram and small-input pricing, update/refresh and next-block rescaling.', mode)
    queries = []
    queries += [(4, 0, 0, value, 1, 3) for value in boundaries(LL_BITS, 0, 131072)]
    queries += [(4, 0, 0, 0, 1, value) for value in boundaries(ML_BITS, 3, 131072)]
    offsets = sorted({n for bit in range(32) for n in ((1 << bit) - 1, 1 << bit, (1 << bit) + 1)
                      if 1 <= n <= 0xffffffff} | {0xffffffff})
    queries += [(4, 0, 0, 0, value, 131072) for value in offsets]
    for mode in (0, 1):
        for length in (8, 9):
            for batch, start in enumerate(range(0, len(queries), 100)):
                add(f'symbols_n{length}_m{mode}_{batch}', bytes(range(length)),
                    [(0, 0, length, 0, 0, 0), *queries[start:start + 100]],
                    'Every LL/ML symbol interval and offset power boundary in predefined/dynamic pricing.', mode)
    source = bytes(range(256)) * 512
    query = (4, 0, 256, 65536, 1 << 20, 65539)
    # Request counts only, chosen around the independently specified rescaling
    # thresholds. These are not generated price or observation expectations.
    for count in (1, 2, 3, 7, 8, 15, 16, 31, 32, 255, 256, 257,
                  4042, 4043, 4044, 4055, 4056, 4057, 4094, 4095, 4096, 8192):
        for mode in (0, 1):
            add(f'counts_{count}_m{mode}', source,
                [(0, 0, len(source), 0, 0, 0), (5, 0, 0, 1, 3, count), refresh,
                 query, (3, 0, 0, 0, 0, 0), query, (3, 0, 1, 0, 0, 0), query],
                'Whole-bit frequency boundaries and repeated sequence-table downscaling with complete retained state.', mode)
    for length in (3711, 3712, 3713, 131071):
        for count in (1, 2, 32):
            add(f'literals_{length}_x{count}', source,
                [(0, 0, len(source), 0, 0, 0), (5, 0, length, 0xffffffff, 131072, count),
                 refresh, query, (3, 0, 0, 0, 0, 0), query],
                'Literal-frequency scaling threshold, absent/present symbols and maximum bounded accumulated updates.')
    add('grouped_dirty_cache', source,
        [(0, 0, 8, 0, 0, 0), (1, 0, 256, 2, 35, 0), (1, 256, 256, 3, 67, 0),
         (5, 0, 0, 4, 131, 4096), refresh, query, (3, 0, 9, 0, 0, 0), query],
        'Several updates intentionally retain cached base prices until the explicit refresh operation.')
    require(len(result) == len({r.name for r in result}), 'unique recipe identities')
    for recipe in result:
        recipe.metadata()
    return result


def manifest():
    values = [r.metadata() for r in recipes()]
    return dict(schema_version=2, protocol='optimal-price-v2', revision=REVISION,
                status='source-only pre-execution hypotheses', recipes=values,
                recipe_count=len(values), source_bytes=sum(r['source_bytes'] for r in values),
                operation_count=sum(len(r['operations']) for r in values),
                qualification='No C execution, observed statistics, prices, predicted results or public strategy admission.')
