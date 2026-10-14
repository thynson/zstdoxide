"""Independent exact tiny sorted-match replay, using checked integer links.

Hash constants and observable rules: facebook/zstd v1.5.7, zstd_opt.c and
zstd_compress_internal.h, revision f8745da6ff1ad1e7bab384bd1f9d742439278e99.
Suffix comparison deliberately omits the C common-prefix optimization.
"""
import io
from .optimal_matches import require, REVISION
from .tree_validation import parse, same

MAX_OUTPUT = 64 << 20
MAX_LINE = 512 << 10


def bucket(source, native, bits, width):
    position = native - 2
    width = min(6, width)
    require(0 <= position <= len(source) - max(4, width), 'hash footprint')
    word = int.from_bytes(source[position:position + width], 'little')
    if width == 3:
        return (((word << 8) * 506832829) & 0xffffffff) >> (32 - bits)
    if width == 4:
        return ((word * 2654435761) & 0xffffffff) >> (32 - bits)
    shift, prime = (24, 889523592379) if width == 5 else (16, 227718039650203)
    return (((word << shift) * prime) & 0xffffffffffffffff) >> (64 - bits)


class Replay:
    def __init__(self, request):
        request.validate()
        self.request = request
        self.heads = [0] * (1 << request.hash_log)
        self.nodes = [[0, 0] for _ in range(1 << (request.chain_log - 1))]
        self.short = [0] * (1 << request.hash3_log) if request.hash3_log else []
        self.frontier = self.short_frontier = 2

    def snapshot(self):
        return dict(heads=self.heads.copy(), nodes=[p.copy() for p in self.nodes],
                    short_heads=self.short.copy(), frontier=self.frontier,
                    short_frontier=self.short_frontier)

    def length(self, a, b):
        require(2 <= b < a <= len(self.request.source) - 6, 'ordered source positions')
        source = self.request.source
        a -= 2
        b -= 2
        count = 0
        while a + count < len(source) and source[a + count] == source[b + count]:
            count += 1
        return count

    def floor(self, target):
        return max(2, target - (1 << self.request.window_log))

    def store(self, link, value):
        if link is not None:
            slot, side = link
            require(0 <= slot < len(self.nodes) and side in (0, 1), 'checked node link')
            self.nodes[slot][side] = value

    def insert(self, current, target, best, results):
        """Sort a node; return best length and farthest matched native end.

        results=None denotes catch-up, whose end handling and sampling differ.
        """
        r = self.request
        key = bucket(r.source, current, r.hash_log, r.min_match)
        candidate = self.heads[key]
        self.heads[key] = current
        mask = len(self.nodes) - 1
        expired = max(0, current - mask)
        smaller, larger = (current & mask, 0), (current & mask, 1)
        farthest = current + 9
        for _ in range(1 << r.search_log):
            if candidate < self.floor(target):
                break
            require(2 <= candidate < current, 'descending tree candidate')
            length = self.length(current, candidate)
            improved = length > best
            if improved:
                best = length
                farthest = max(farthest, candidate + length)
                if results is not None:
                    results.append([current - candidate + 3, length])
            end = current - 2 + length == len(r.source)
            if (results is None and end) or (results is not None and improved and (length > 4096 or end)):
                break
            require(not end, 'best length permits source-end exit')
            slot = candidate & mask
            if r.source[candidate - 2 + length] < r.source[current - 2 + length]:
                self.store(smaller, candidate)
                if candidate <= expired:
                    smaller = None
                    break
                smaller = (slot, 1)
                candidate = self.nodes[slot][1]
            else:
                self.store(larger, candidate)
                if candidate <= expired:
                    larger = None
                    break
                larger = (slot, 0)
                candidate = self.nodes[slot][0]
        self.store(smaller, 0)
        self.store(larger, 0)
        return best, farthest

    def query(self, words):
        q, rep0, rep1, rep2, ll0, beat = words
        current = q + 2
        before = self.snapshot()
        results = []
        phase = 'skipped'
        if current >= self.frontier:
            position = self.frontier
            while position < current:
                best, end = self.insert(position, current, 8, None)
                position += max(min(192, best - 384) if best > 384 else 0,
                                end - (position + 8))
            self.frontier = current
            best = beat - 1
            sufficient = min(self.request.target, 4095)
            repeats = (rep1, rep2, rep0 - 1) if ll0 else (rep0, rep1, rep2)
            phase = 'tree'
            for intent, distance in enumerate(repeats, 1):
                previous = current - distance
                if distance <= 0 or not self.floor(current) <= previous < current:
                    continue
                length = self.length(current, previous)
                if length >= (3 if self.request.min_match == 3 else 4) and length > best:
                    best = length
                    results.append([intent, length])
                    if length > sufficient or q + length == len(self.request.source):
                        phase = 'repeat'
                        break
            if phase == 'tree' and self.request.min_match == 3 and best < 3:
                for native in range(self.short_frontier, current):
                    self.short[bucket(self.request.source, native, self.request.hash3_log, 3)] = native
                self.short_frontier = current
                previous = self.short[bucket(self.request.source, current, self.request.hash3_log, 3)]
                if self.floor(current) <= previous < current and current - previous < (1 << 18):
                    length = self.length(current, previous)
                    if length >= 3:
                        best = length
                        results.append([current - previous + 3, length])
                        if length > sufficient or q + length == len(self.request.source):
                            self.frontier = current + 1
                            phase = 'short'
            if phase == 'tree':
                _, end = self.insert(current, current, best, results)
                self.frontier = end - 8
        require(len(results) <= 4099, 'bounded candidate count')
        return dict(query=list(words), before=before, after=self.snapshot(), matches=results), phase


def expected(request):
    replay = Replay(request)
    return [replay.query(q)[0] for q in request.queries]


def validate(request, data):
    request.validate()
    require(type(data) is bytes and 0 < len(data) <= MAX_OUTPUT and data.endswith(b'\n'),
            'bounded newline-terminated observations')
    stream = io.BytesIO(data)

    def record():
        line = stream.readline(MAX_LINE + 1)
        require(0 < len(line) <= MAX_LINE and line.endswith(b'\n'), 'bounded observation line')
        return parse(line)

    same(record(), dict(record='metadata', protocol='ZOPMT01', revision=REVISION,
                       source_bytes=len(request.source), queries=len(request.queries), bias=2,
                       parameters=[request.window_log, request.hash_log, request.chain_log,
                                   request.search_log, request.min_match, request.target, request.hash3_log]),
         'exact request metadata')
    observations = expected(request)
    for padding_pass in range(2):
        for index, observation in enumerate(observations):
            wanted = dict(record='operation', padding_pass=padding_pass, index=index,
                          source_unchanged=True, guards_unchanged=True, **observation)
            same(record(), wanted, 'independent complete search state')
    same(record(), dict(record='complete', operations=len(observations), passes=2, allocations=0),
         'complete bounded trailer')
    require(stream.read(1) == b'', 'exact observation EOF')
    return dict(operations=len(observations), matches=sum(len(o['matches']) for o in observations),
                records=2 * len(observations) + 2, stdout_bytes=len(data))
