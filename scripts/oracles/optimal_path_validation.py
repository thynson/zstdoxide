"""Independent bounded optLevel0 path replay with explicit phase and integer cells.

Observable transition rules: pinned v1.5.7 zstd_opt.c. No C or production Rust is
executed here. Price/frequency arithmetic reuses the independently captured model.
"""
from .optimal_paths import (Request, REVISION, MAX_EVENTS, MAX_OUTPUT, require,
                            next_repeats, valid_match)
from .optimal_price_validation import Model, finite

INFINITY = 1 << 30
PRICE, OFF, ML, LL = range(4)


class Replay:
    def __init__(self, request):
        require(type(request) is Request, 'exact path request')
        request.validate()
        self.r = request
        self.prices = Model(request.literal_mode)
        self.cells = [[0] * 7 for _ in range(len(request.source) + 3)]
        self.repeats = (1, 4, 8)
        self.wire_repeats = self.repeats
        self.sequences = []
        self.events = []
        self.coverage = set()
        self.script = dict(request.scripts)
        self.phase = 'forward'
        self.ip = self.anchor = 0

    def cell(self, index):
        require(type(index) is int and 0 <= index < len(self.cells), 'path cell index')
        return self.cells[index]

    def emit(self, stage, cur=None, last=None, stretch=None, reverse=None, query=None, matches=None, tail=None):
        require(len(self.events) < MAX_EVENTS, 'bounded event count')
        require(self.phase in ('forward', 'reverse'), 'path phase')
        self.events.append(dict(record='event', stage=stage, phase=self.phase,
            ip=self.ip, anchor=self.anchor, cur=cur, last=last,
            stretch=None if stretch is None else stretch.copy(), reverse=reverse,
            query=query, matches=matches, tail=tail, repeats=list(self.repeats),
            cells=[c.copy() for c in self.cells],
            frequencies={k: v.copy() for k, v in self.prices.tables.items()},
            sums=self.prices.sums.copy(), base_prices=self.prices.cache.copy(),
            predefined=self.prices.predefined,
            sequences=[s.copy() for s in self.sequences]))

    def find(self, position, repeats, zero, cur, last):
        require(1 <= position <= len(self.r.source) - 8, 'callback source footprint')
        matches = self.script.get(position, ())
        for off, length in matches:
            valid_match(self.r.source, position, off, length, repeats, zero)
        self.emit('query', cur, last, query=[position, *repeats, int(zero), self.r.minimum],
                  matches=[list(m) for m in matches])
        return matches

    def seed(self, matches, litlen):
        for pos in range(1, self.r.minimum):
            c = self.cell(pos)
            c[PRICE], c[ML], c[LL] = INFINITY, 0, litlen + pos
        pos = self.r.minimum
        for off, end in matches:
            while pos <= end:
                c = self.cell(pos)
                c[ML], c[OFF], c[LL] = pos, off, 0
                c[PRICE] = finite(self.cell(0)[PRICE] + self.prices.match_price(off, pos) + self.prices.ll_price(0))
                pos += 1
        self.cell(pos)[PRICE] = INFINITY
        self.emit('seed', 0, pos - 1)
        return pos - 1

    def forward(self, last):
        cur = 1
        while cur <= last:
            previous = self.cell(cur - 1)
            litlen = previous[LL] + 1
            price = finite(previous[PRICE] + self.prices.literal_price(self.r.source[self.ip + cur - 1:self.ip + cur])
                           + self.prices.ll_price(litlen) - self.prices.ll_price(litlen - 1))
            if price <= self.cell(cur)[PRICE]:
                self.coverage.add('literal_tie' if price == self.cell(cur)[PRICE] else 'literal_improvement')
                self.cells[cur] = previous.copy()
                self.cell(cur)[LL], self.cell(cur)[PRICE] = litlen, price
            else:
                self.coverage.add('literal_rejected')
            c = self.cell(cur)
            require(cur >= c[ML], 'match predecessor index')
            if c[LL] == 0:
                predecessor = self.cell(cur - c[ML])
                c[4:] = next_repeats(c[OFF], tuple(predecessor[4:]), predecessor[LL] == 0)
                self.coverage.add('deferred_repeat_publication')
            self.emit('literal', cur, last)
            if self.ip + cur > len(self.r.source) - 8:
                self.coverage.add('tail_no_query')
                cur += 1
                continue
            if cur == last:
                break
            if self.cell(cur + 1)[PRICE] <= c[PRICE] + 128:
                self.coverage.add('unpromising_skip')
                cur += 1
                continue
            matches = self.find(self.ip + cur, tuple(c[4:]), c[LL] == 0, cur, last)
            if not matches:
                cur += 1
                continue
            off, longest = matches[-1]
            if longest > self.r.target or cur + longest >= 4096 or self.ip + cur + longest >= len(self.r.source):
                self.coverage.add('later_sufficient' if longest > self.r.target else 'later_source_end')
                return cur, cur + longest, [0, off, longest, 0, 0, 0, 0], True
            base = finite(c[PRICE] + self.prices.ll_price(0))
            previous_length = self.r.minimum - 1
            for off, end in matches:
                for length in range(end, previous_length, -1):
                    pos = cur + length
                    price = finite(base + self.prices.match_price(off, length))
                    if pos > last or price < self.cell(pos)[PRICE]:
                        self.coverage.add('match_extend' if pos > last else 'match_improvement')
                        while last < pos:
                            last += 1
                            self.cell(last)[PRICE], self.cell(last)[LL] = INFINITY, 1
                        dest = self.cell(pos)
                        dest[ML], dest[OFF], dest[LL], dest[PRICE] = length, off, 0, price
                    else:
                        self.coverage.add('match_tie_abort' if price == self.cell(pos)[PRICE] else 'match_worse_abort')
                        break
                previous_length = end
            self.cell(last + 1)[PRICE] = INFINITY
            self.emit('relax', cur, last)
            cur += 1
        stretch = self.cell(last).copy()
        require(last >= stretch[ML], 'last stretch predecessor')
        return last - stretch[ML], last, stretch, False

    def reconstruct(self, cur, last, stretch):
        self.emit('select', cur, last, stretch)
        require(self.cell(0)[ML] == 0 and last >= stretch[ML] and cur == last - stretch[ML], 'selected stretch bounds')
        if stretch[ML] == 0:
            require(stretch[LL] == self.ip - self.anchor + last, 'all-literal path extent')
            self.coverage.add('all_literals')
            self.ip += last
            return
        require(stretch[OFF] > 0, 'selected match intent')
        if stretch[LL] == 0:
            predecessor = self.cell(cur)
            self.repeats = next_repeats(stretch[OFF], tuple(predecessor[4:]), predecessor[LL] == 0)
        else:
            self.repeats = tuple(stretch[4:])
            require(cur >= stretch[LL], 'trailing literal predecessor')
            cur -= stretch[LL]
            self.coverage.add('trailing_backtrack')
        end = cur + 2
        start = end
        position = cur
        self.phase = 'reverse'
        if stretch[LL] > 0:
            self.cell(end)[LL], self.cell(end)[ML] = stretch[LL], 0
            start = end - 1
            self.cells[start] = stretch.copy()
        # Preserve the pinned unconditional block after the preceding if.
        self.cells[end] = stretch.copy()
        start = end
        self.emit('reverse_start', cur, last, stretch, [start, end, position])
        steps = 0
        while True:
            previous = self.cell(position).copy()
            require(start > position, 'destination must not erase unread predecessor')
            self.cell(start)[LL] = previous[LL]
            self.emit('reverse', cur, last, stretch, [start, end, position])
            if previous[ML] == 0:
                break
            start -= 1
            self.cells[start] = previous.copy()
            advance = previous[LL] + previous[ML]
            require(0 < advance <= position, 'strict decreasing predecessor')
            position -= advance
            steps += 1
            require(steps <= len(self.cells), 'acyclic predecessor chain')
        if end > start:
            self.coverage.add('multiple_sequences')
        for index in range(start, end + 1):
            c = self.cell(index)
            litlen, length, off = c[LL], c[ML], c[OFF]
            if length == 0:
                require(index == end, 'only final entry may contain literals alone')
                self.ip = self.anchor + litlen
                continue
            position = self.anchor + litlen
            valid_match(self.r.source, position, off, length, self.wire_repeats, litlen == 0)
            literals = self.r.source[self.anchor:position]
            require(len(literals) == litlen, 'literal source extent')
            self.prices.update(literals, off, length)
            self.sequences.append([self.anchor, litlen, length, off])
            self.wire_repeats = next_repeats(off, self.wire_repeats, litlen == 0)
            self.anchor = position + length
            self.ip = self.anchor
            self.emit('sequence', cur, last, stretch, [start, end, index])
        require(self.wire_repeats == self.repeats, 'path/sequence repeat publication agreement')
        self.prices.refresh()
        self.emit('refresh', cur, last, stretch)
        self.phase = 'forward'

    def run(self):
        self.prices.rescale(self.r.source)
        self.ip = 1
        self.emit('begin')
        while self.ip < len(self.r.source) - 8:
            litlen = self.ip - self.anchor
            matches = self.find(self.ip, self.repeats, litlen == 0, 0, 0)
            if not matches:
                self.coverage.add('initial_no_match')
                self.ip += 1
                continue
            origin = self.cell(0)
            origin[ML], origin[LL], origin[PRICE] = 0, litlen, self.prices.ll_price(litlen)
            origin[4:] = self.repeats
            self.emit('origin', 0, 0)
            off, longest = matches[-1]
            if longest > self.r.target:
                self.coverage.add('initial_sufficient')
                # lastStretch is initially zero and retains stale fields later.
                stretch = self.last_stretch.copy() if hasattr(self, 'last_stretch') else [0] * 7
                stretch[LL], stretch[ML], stretch[OFF] = 0, longest, off
                cur, last = 0, longest
            else:
                last = self.seed(matches, litlen)
                cur, last, stretch, early = self.forward(last)
                if early:
                    old = self.last_stretch.copy() if hasattr(self, 'last_stretch') else [0] * 7
                    old[ML], old[OFF], old[LL] = stretch[ML], stretch[OFF], 0
                    stretch = old
            self.last_stretch = stretch.copy()
            self.reconstruct(cur, last, stretch)
        self.emit('complete', tail=len(self.r.source) - self.anchor)
        return self.events


def expected(request):
    return Replay(request).run()


def observations(request):
    from .optimal_price_validation import fnv
    replay = Replay(request)
    events = replay.run()
    yield dict(record='metadata', protocol='ZOPTH01', revision=REVISION,
               source_bytes=len(request.source), source_hash=fnv(request.source),
               script_positions=len(request.scripts), parameters=[request.min_match, request.target, request.literal_mode],
               path_cells=len(request.source) + 3, passes=2)
    for padding_pass in range(2):
        for index, event in enumerate(events):
            yield dict(event, padding_pass=padding_pass, index=index,
                       source_unchanged=True, guards_unchanged=True, path_tail_zero=True)
    yield dict(record='complete', events=2 * len(events), passes=2,
               sequences=len(replay.sequences), guards_unchanged=True, source_unchanged=True)


def validate_output(data, request):
    import io
    from .tree_validation import parse, same
    require(type(data) is bytes and 0 < len(data) <= MAX_OUTPUT and data.endswith(b'\n'), 'bounded complete trace')
    stream = io.BytesIO(data)
    for wanted in observations(request):
        line = stream.readline((128 << 10) + 1)
        require(0 < len(line) <= (128 << 10) and line.endswith(b'\n'), 'bounded observation line')
        same(parse(line), wanted, 'independent complete path observation')
    require(stream.read() == b'', 'no trailing trace')
    return True
