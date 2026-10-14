"""Wider replay reuses captured transitions and emits canonical complete-state deltas."""
from .optimal_path_validation import Replay as SmallReplay
from .optimal_paths_wide import Request, REVISION, MAX_EVENTS, MAX_OUTPUT, require
from .optimal_price_validation import Model, fnv


class Replay(SmallReplay):
    def __init__(self, request):
        require(type(request) is Request, 'exact wider request')
        request.validate()
        self.r = request
        self.prices = Model(request.literal_mode)
        self.cells = [[0] * 7 for _ in range(min(len(request.source) + 3, 4099))]
        self.previous = [c.copy() for c in self.cells]
        self.repeats = (1, 4, 8)
        self.wire_repeats = self.repeats
        self.sequences = []
        self.events = []
        self.coverage = set()
        self.script = dict(request.scripts)
        self.phase = 'forward'
        self.ip = self.anchor = 0

    def emit(self, stage, cur=None, last=None, stretch=None, reverse=None, query=None, matches=None, tail=None):
        require(len(self.events) < MAX_EVENTS, 'bounded wider events')
        changes = []
        for index, (before, after) in enumerate(zip(self.previous, self.cells)):
            if before != after:
                changes.append([index, after.copy()])
                self.previous[index] = after.copy()
        self.events.append(dict(record='event', stage=stage, phase=self.phase,
            ip=self.ip, anchor=self.anchor, cur=cur, last=last,
            stretch=None if stretch is None else stretch.copy(), reverse=reverse,
            query=query, matches=matches, tail=tail, repeats=list(self.repeats),
            cells=dict(length=len(self.cells), changes=changes),
            frequencies={k:v.copy()for k,v in self.prices.tables.items()},
            sums=self.prices.sums.copy(), base_prices=self.prices.cache.copy(),
            predefined=self.prices.predefined, sequences=[s.copy()for s in self.sequences]))

    def forward(self, last):
        had_source_end = 'later_source_end' in self.coverage
        result = super().forward(last)
        cur, last, stretch, early = result
        if early and stretch[2] <= self.r.target and self.ip + last < len(self.r.source):
            require(last >= 4096, 'horizon is the remaining exit condition')
            if not had_source_end:
                self.coverage.discard('later_source_end')
            self.coverage.add('later_horizon')
        return result


def apply_delta(previous, delta):
    require(type(delta) is dict and set(delta) == {'length','changes'}, 'exact delta shape')
    require(type(delta['length']) is int and delta['length'] == len(previous), 'fixed path extent')
    require(type(delta['changes']) is list and len(delta['changes']) <= len(previous), 'bounded changes')
    last = -1
    result = [c.copy() for c in previous]
    for entry in delta['changes']:
        require(type(entry) is list and len(entry) == 2, 'delta entry')
        index, cell = entry
        require(type(index) is int and last < index < len(previous), 'strict delta indices')
        require(type(cell) is list and len(cell) == 7 and all(type(v) is int and 0 <= v <= 0xffffffff for v in cell), 'initialized complete cell')
        require(previous[index] != cell, 'changed cells only')
        result[index] = cell.copy()
        last = index
    return result


def observations(request):
    replay = Replay(request)
    replay.run()
    yield dict(record='metadata', protocol='ZOPHW02', revision=REVISION,
        source_bytes=len(request.source), source_hash=fnv(request.source), script_positions=len(request.scripts),
        parameters=[request.min_match,request.target,request.literal_mode], path_cells=len(replay.cells), passes=2,
        path_encoding='canonical-delta-from-zero')
    for padding_pass in range(2):
        for index,event in enumerate(replay.events):
            yield dict(event,padding_pass=padding_pass,index=index,source_unchanged=True,guards_unchanged=True,path_tail_zero=True)
    yield dict(record='complete',events=2*len(replay.events),passes=2,sequences=len(replay.sequences),guards_unchanged=True,source_unchanged=True)


from functools import lru_cache
import json


@lru_cache(maxsize=16)
def _expected_json(request):
    # Cache immutable canonical strings, never mutable path/frequency records.
    return tuple(json.dumps(row,sort_keys=True,separators=(',',':')) for row in observations(request))


def event_count(request):
    return (len(_expected_json(request))-2)//2


def validate_output(data,request):
    import io
    from .tree_validation import parse
    require(type(data)is bytes and 0<len(data)<=MAX_OUTPUT and data.endswith(b'\n'),'bounded wider trace')
    stream=io.BytesIO(data)
    for wanted in _expected_json(request):
        line=stream.readline((512<<10)+1)
        require(0<len(line)<=(512<<10)and line.endswith(b'\n'),'bounded wider observation line')
        actual=json.dumps(parse(line),sort_keys=True,separators=(',',':'))
        require(actual==wanted,'exact wider path replay')
    require(stream.read()==b'','no trailing wider trace')
    return True
