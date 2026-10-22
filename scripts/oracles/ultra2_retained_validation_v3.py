"""Additive v3: retain the distinct fresh and retained event caps.

Search indices retain the first block's bias. Source access is a checked view
of only the consumed prefix. No native candidate list or Rust implementation
is used to predict matches, prices, paths or publication decisions.
"""
from dataclasses import asdict
import io

from .ultra2_retained import Request, MAX_OUTPUT, MAX_LINE, REVISION, need
from .ultra2_validation import LiveSource, SourceRequest as FreshRequest, frequency
from .ultra2_lifecycle import Coordinates, Entry
from .optimal_match_broad_validation import Replay as OrdinarySearch
from .ultra_retained_validation import BlockReplay as RetainedBlock, PATH_CELLS
from .ultra_frequency import Model
from .ultra_fresh_validation import BlockReplay as FreshBlock
from .optimal_price_validation import fnv
from .ultra2_protocol import canonical
from .ultra_scalar import load


class Prefix:
    def __init__(self, original, end, origin):
        original.validate()
        need(type(end) is int and end in self.ends(original), 'complete consumed prefix')
        self.original = original
        self.source = LiveSource(original.source[:end], origin)

    @staticmethod
    def ends(original):
        total = 0
        result = []
        for length in original.blocks:
            total += length
            result.append(total)
        return result

    def __getattr__(self, name):
        return getattr(self.original, name)

    def validate(self):
        self.original.validate()
        self.source.coordinates.validate()
        return self


class Search(OrdinarySearch):
    def __init__(self, original):
        super().__init__(Prefix(original, original.blocks[0], 2))

    def floor(self, target):
        return max(self.request.source.coordinates.origin,
                   target - (1 << self.request.window_log))

    def rebase_first(self):
        view = self.request
        need(view.source.coordinates.origin == 2 and
             len(view.source.source) == view.original.blocks[0], 'first prefix only rebase')
        origin = 2 + view.original.blocks[0]
        Coordinates(origin, len(view.original.source)).validate()
        self.request = Prefix(view.original, view.original.blocks[0], origin)
        self.frontier = self.short_frontier = origin


class Block(RetainedBlock):
    def __init__(self, request, index, start, search, prices, cells, repeats,
                 path_baseline, search_baseline):
        request.validate()
        r = request.original
        need(start == sum(r.blocks[:index]) and 0 <= index < len(r.blocks),
             'exact physical block start')
        length = r.blocks[index]
        fields = asdict(r)
        fields.update(source=r.source[start:start + length], blocks=(length,))
        self.r = FreshRequest(**fields).validate()
        self.full, self.block, self.start = r, index, start
        origin = search.request.source.coordinates.origin
        current = origin + start
        if current > search.frontier + 384:
            search.frontier = current - min(192, current - search.frontier - 384)
        search.request = Prefix(r, start + length, origin)
        search.short_frontier = search.frontier
        self.search, self.prices, self.cells = search, prices, cells
        self.repeats = self.wire_repeats = tuple(repeats)
        self.before = None
        self.sequences, self.events, self.witnesses = [], [], []
        self.coverage, self.script = set(), {}
        self.phase, self.ip, self.anchor = 'forward', 0, 0
        self.path_baseline, self.search_baseline = path_baseline, search_baseline
        self.policy = request.policy

    def emit(self, stage, cur=None, last=None, stretch=None, reverse=None,
             query=None, matches=None, tail=None, alternative=None):
        # Fresh predecessor controls admit 16384 bytes and 65536 events; actual
        # retained blocks admit 8192 bytes and keep their 32768-event cap.
        emitter = FreshBlock.emit if len(self.full.blocks) == 1 else RetainedBlock.emit
        emitter(self, stage, cur, last, stretch, reverse, query, matches, tail, alternative)

    def find(self, position, repeats, zero, cur, last):
        need((0 if self.start else 1) <= position <= len(self.r.source) - 8,
             'retained query footprint')
        need(len(repeats) == 3 and all(type(v) is int and
             0 < v <= max(8, self.start + len(self.r.source)) for v in repeats),
             'retained repeat domain')
        words = (self.start + position, *repeats, int(zero), self.r.minimum)
        shift = self.search.request.source.coordinates.origin - 2
        observation, phase = self.search.query((words[0] + shift, *words[1:]))
        self.coverage.add('search_' + phase)
        if position == 0:
            self.coverage.add('block_boundary_query')
        self.before = observation['before']
        matches = observation['matches']
        self.emit('query', cur, last, query=list(words), matches=matches)
        return tuple(tuple(pair) for pair in matches)


def records(request, kinds):
    need(type(request) is Request, 'exact phase/block request')
    request.validate()
    r = request.original
    need(type(kinds) is tuple and len(kinds) == len(r.blocks) and
         all(type(k) is int and k in (0, 1, 2) for k in kinds), 'publication hypotheses')
    search, prices = Search(r), Model(request.policy, r.literal_mode)
    cells = [[0] * 7 for _ in range(PATH_CELLS)]
    path_baseline = [[0] * 7 for _ in range(PATH_CELLS)]
    search_baseline = [0] * (len(search.heads) + 2 * len(search.nodes) + len(search.short) + 2)
    owners, previous, start = [[1, 4, 8], [0, 0, 0]], 0, 0
    rows, coverage = [], set()
    for index, length in enumerate(r.blocks):
        repeats = owners[previous].copy()
        origin = search.request.source.coordinates.origin
        rows.append(dict(record='block_begin', block=index, start=start, length=length,
                         previous_owner=previous, owners=[v.copy() for v in owners],
                         frontier=search.frontier, price_policy=request.policy))
        # A disabled bootstrap is an explicit first-entry opaque-prefix control,
        # used for predecessor reconciliation, not an alternative public mode.
        prefix = int(index == 0 and not request.bootstrap)
        # buildSeqStore applies catch-up before the wrapper observes eligibility.
        current = origin + start
        if current > search.frontier + 384:
            search.frontier = current - min(192, current - search.frontier - 384)
        entry = Entry(prices.sums['ll'], prefix, origin, origin, origin + start,
                      length, search.frontier)
        eligible = entry.eligible()
        need(not eligible or index == 0, 'bootstrap belongs only to first block')
        rows.append(dict(record='eligibility', block=index, predicates=list(entry.predicates()),
                         seed=eligible, current=entry.current, dictionary_limit=origin,
                         low_limit=origin, next_to_update=search.frontier, source_bytes=length,
                         original_repeats=repeats.copy(), sequence_prefix=prefix))
        for phase in (('seeding', 'encoding') if eligible else ('encoding',)):
            origin = search.request.source.coordinates.origin
            rows.append(dict(record='pass_begin', block=index, phase=phase, origin=origin,
                             repeats=repeats.copy(), frequency=frequency(prices)))
            block = Block(request, index, start, search, prices, cells, repeats,
                          path_baseline, search_baseline)
            rows.extend(dict(event, pass_name=phase) for event in block.run())
            coverage.update(phase + ':' + name for name in block.coverage)
            rows.append(dict(record='pass_end', block=index, phase=phase, origin=origin,
                             candidate_repeats=list(block.wire_repeats),
                             sequences=[[start + v[0], *v[1:]] for v in block.sequences],
                             tail=length - block.anchor, frequency=frequency(prices),
                             source_reads=search.request.source.reads))
            if phase == 'seeding':
                before = search.snapshot()
                search.rebase_first()
                rebased = entry.rebased()
                after = search.snapshot()
                need(all(before[k] == after[k] for k in ('heads', 'nodes', 'short_heads')),
                     'physical search cells survive first-block rebase')
                need(search.frontier == search.short_frontier == rebased.current,
                     'both insertion frontiers reset to new origin')
                rows.append(dict(record='reset', block=index,
                                 discarded_sequences=len(block.sequences),
                                 discarded_literals=sum(v[1] for v in block.sequences),
                                 discarded_tail=length - block.anchor,
                                 caller_repeats=repeats.copy(), discarded_repeats=list(block.wire_repeats),
                                 origin_before=origin, origin_after=rebased.current,
                                 low_limit=rebased.low_limit, dictionary_limit=rebased.dictionary_limit,
                                 next_to_update=rebased.next_to_update,
                                 frequency=frequency(prices), tables_preserved=True))
        pending = 1 - previous
        candidate = list(block.wire_repeats)
        owners[pending] = candidate
        before_owner = previous
        if kinds[index] == 2:
            previous = pending
            coverage.add('compressed_commit')
        else:
            coverage.add('raw_rollback' if kinds[index] == 0 else 'rle_rollback')
            if candidate != repeats:
                coverage.add('discarded_candidate_repeats')
        rows.append(dict(record='block_publish', block=index, kind=kinds[index],
                         previous_owner_before=before_owner, previous_owner_after=previous,
                         before_repeats=repeats, candidate_repeats=candidate,
                         owners=[v.copy() for v in owners], sequences=len(block.sequences),
                         tail=length - block.anchor, price_policy=request.policy))
        start += length
    need(start == len(r.source), 'complete source consumed')
    return rows, coverage


def observations(request, kinds):
    rows, _ = records(request, kinds)
    r = request.original
    yield dict(record='metadata', protocol='ZU2RT01', revision=REVISION,
               source_bytes=len(r.source), source_hash=fnv(r.source), bias=2,
               parameters=[r.window_log, r.hash_log, r.chain_log, r.search_log,
                           r.min_match, r.target, r.literal_mode], blocks=list(r.blocks),
               bootstrap=request.bootstrap, price_policy=request.policy,
               path_cells=PATH_CELLS, padding_passes=2,
               state_encoding='canonical-deltas-from-zero')
    for padding in range(2):
        for row in rows:
            row = {k: v for k, v in row.items() if k != 'source_reads'}
            if row['record'] == 'event':
                row = dict(row, source_unchanged=True, guards_unchanged=True, path_tail_zero=True)
            yield dict(row, padding_pass=padding)
    yield dict(record='complete', padding_passes=2, blocks=len(r.blocks),
               algorithm_passes=2 * sum(row['record'] == 'pass_end' for row in rows),
               events=2 * sum(row['record'] == 'event' for row in rows),
               source_unchanged=True, guards_unchanged=True)


def validate_output(data, request, kinds):
    need(type(data) is bytes and 0 < len(data) <= MAX_OUTPUT and data.endswith(b'\n'),
         'bounded newline-terminated retained Ultra2 transcript')
    stream = io.BytesIO(data)
    for expected in observations(request, kinds):
        line = stream.readline(MAX_LINE + 1)
        need(0 < len(line) <= MAX_LINE and line.endswith(b'\n'), 'bounded complete row')
        need(canonical(load(line)) == canonical(expected), 'independent complete phase/block state')
    need(stream.read(1) == b'', 'exact transcript EOF')
