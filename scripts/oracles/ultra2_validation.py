"""Source-only two-pass Ultra2 replay with unreadable stale coordinates.

The qualified ordinary Ultra path and frequency transitions are reused. Search
tables retain physical first-pass cells. A checked source view translates live
indices and rejects every stale read instead of allocating a fake prefix.
These are model predictions until a separate native observer qualifies them.
"""
from dataclasses import dataclass

from .ultra2_lifecycle import Coordinates, Entry, integer
from .ultra_fresh_validation import BlockReplay as OrdinaryBlock, PATH_CELLS
from .optimal_match_broad_validation import Replay as OrdinarySearch
from .optimal_fresh_wide import Request as OrdinaryRequest
from .ultra_frequency import Model
from .ultra_scalar import need


@dataclass(frozen=True)
class SourceRequest(OrdinaryRequest):
    """Same geometry as ordinary requests, with private 7/8/9 controls."""
    def validate(self):
        need(type(self.source) is bytes and type(self.blocks) is tuple,
             'owned immutable source and schedule')
        integer(len(self.source), 7, 16384, 'private source extent')
        need(self.blocks == (len(self.source),) and type(self.blocks[0]) is int,
             'one complete block')
        for value, low, high, label in (
            (self.window_log, 10, 16, 'window'), (self.hash_log, 6, 12, 'hash'),
            (self.chain_log, 6, 12, 'chain'), (self.search_log, 1, 6, 'search'),
            (self.min_match, 3, 7, 'minimum'), (self.target, 0, 4095, 'target'),
            (self.literal_mode, 0, 1, 'literal mode')):
            integer(value, low, high, label)
        need(len(self.source) <= 1 << self.window_log, 'complete source in window')
        return self


@dataclass(frozen=True)
class Tagged:
    original: SourceRequest
    policy: int = 2

    def validate(self):
        need(type(self.policy) is int and self.policy == 2, 'coupled Ultra2 policy')
        need(type(self.original) is SourceRequest, 'exact independent source request')
        self.original.validate()
        return self


class LiveSource:
    """Index space expected by the ordinary model, with no readable prefix."""
    def __init__(self, source, origin):
        self.source = source
        self.coordinates = Coordinates(origin, len(source)).validate()
        self.reads = 0

    def __len__(self):
        return self.coordinates.origin - 2 + len(self.source)

    def __getitem__(self, key):
        if type(key) is slice:
            need(key.step is None and type(key.start) is int and type(key.stop) is int,
                 'explicit contiguous source slice')
            start = self.coordinates.offset(key.start + 2, key.stop - key.start)
            self.reads += 1
            return self.source[start:start + key.stop - key.start]
        need(type(key) is int, 'integer source index')
        start = self.coordinates.offset(key + 2)
        self.reads += 1
        return self.source[start]


class SearchView:
    def __init__(self, original, origin):
        self.original = original
        self.source = LiveSource(original.source, origin)

    def __getattr__(self, name):
        return getattr(self.original, name)

    def validate(self):
        self.original.validate()
        self.source.coordinates.validate()
        return self


class Search(OrdinarySearch):
    def __init__(self, request):
        super().__init__(SearchView(request, 2))

    def floor(self, target):
        return max(self.request.source.coordinates.origin,
                   target - (1 << self.request.window_log))

    def rebase(self):
        old = self.request.source.coordinates
        new = old.rebase()
        # No hash, tree or short-key owner is replaced or cleared here.
        self.request = SearchView(self.request.original, new.origin)
        self.frontier = self.short_frontier = new.origin


class Block(OrdinaryBlock):
    def __init__(self, request, search, prices, cells, repeats, path_baseline, search_baseline):
        view = search.request
        super().__init__(request, 0, 0, search, prices, cells, repeats,
                         path_baseline, search_baseline)
        # Ordinary initialization installs a zero-origin SearchPrefix. Restore
        # the checked view; all path/source offsets remain relative to the block.
        self.search.request = view

    def find(self, position, repeats, zero, cur, last):
        need(1 <= position <= len(self.r.source) - 8, 'complete source query footprint')
        shift = self.search.request.source.coordinates.origin - 2
        words = (position, *repeats, int(zero), self.r.minimum)
        physical_words = (position + shift, *words[1:])
        observation, phase = self.search.query(physical_words)
        self.coverage.add('search_' + phase)
        self.before = observation['before']
        matches = observation['matches']
        self.emit('query', cur, last, query=list(words), matches=matches)
        return tuple(tuple(pair) for pair in matches)


def frequency(prices):
    return dict(tables={k: v.copy() for k, v in prices.tables.items()},
                sums=prices.sums.copy(), cache=prices.cache.copy(), predefined=prices.predefined)


def records(request, seed=True):
    """Predict two passes or a fresh ordinary append with an opaque prefix."""
    request.validate()
    need(type(seed) is bool, 'explicit seeding hypothesis')
    tagged = Tagged(request)
    entry = Entry(0, 0 if seed else 1, 2, 2, 2, len(request.source), 2)
    search = Search(request)
    prices = Model(2, request.literal_mode)
    cells = [[0] * 7 for _ in range(PATH_CELLS)]
    path_baseline = [[0] * 7 for _ in range(PATH_CELLS)]
    size = len(search.heads) + 2 * len(search.nodes) + len(search.short) + 2
    search_baseline = [0] * size
    original_repeats = (1, 4, 8)
    rows = [dict(record='eligibility', predicates=list(entry.predicates()),
                 seed=entry.eligible(), current=2, dictionary_limit=2, low_limit=2,
                 next_to_update=2, source_bytes=len(request.source),
                 original_repeats=list(original_repeats), sequence_prefix=entry.sequence_count)]
    coverage = set()
    passes = ('seeding', 'encoding') if entry.eligible() else ('encoding',)
    for phase in passes:
        origin = search.request.source.coordinates.origin
        rows.append(dict(record='pass_begin', phase=phase, origin=origin,
                         repeats=list(original_repeats), frequency=frequency(prices)))
        block = Block(tagged, search, prices, cells, original_repeats,
                      path_baseline, search_baseline)
        events = block.run()
        rows.extend(dict(event, pass_name=phase) for event in events)
        coverage.update(phase + ':' + name for name in block.coverage)
        rows.append(dict(record='pass_end', phase=phase, origin=origin,
                         candidate_repeats=list(block.wire_repeats),
                         sequences=[v.copy() for v in block.sequences],
                         tail=len(request.source) - block.anchor,
                         frequency=frequency(prices), source_reads=search.request.source.reads))
        if phase == 'seeding':
            before = search.snapshot()
            search.rebase()
            rebased = entry.rebased()
            after = search.snapshot()
            need(all(before[k] == after[k] for k in ('heads', 'nodes', 'short_heads')),
                 'retain physical search cells')
            need(search.frontier == search.short_frontier == rebased.current,
                 'reset both update frontiers')
            rows.append(dict(record='reset', discarded_sequences=len(block.sequences),
                             discarded_literals=sum(v[1] for v in block.sequences),
                             discarded_tail=len(request.source) - block.anchor,
                             caller_repeats=list(original_repeats),
                             discarded_repeats=list(block.wire_repeats),
                             origin_before=origin, origin_after=rebased.current,
                             low_limit=rebased.low_limit, dictionary_limit=rebased.dictionary_limit,
                             next_to_update=rebased.next_to_update,
                             frequency=frequency(prices), tables_preserved=True))
    return rows, coverage
