"""Independent retained search/path/frequency composition before native execution.

Transition rules are derived from pinned v1.5.7 zstd_opt.c. Frame publication
hypotheses are explicit source-only inputs, never scripted C callbacks.
"""
from dataclasses import dataclass
import io
import json
from .optimal_retained import Request,REVISION,require
from .optimal_paths import Request as PathRequest,valid_match,next_repeats
from .optimal_path_validation import Replay as PathReplay,ML,LL,OFF,PRICE
from .optimal_match_broad_validation import Replay as SearchReplay
from .optimal_price_validation import Model,fnv
from .tree_validation import parse,same

MAX_EVENTS=1024
MAX_OUTPUT=256<<20
MAX_LINE=1<<20

@dataclass(frozen=True)
class SearchPrefix:
    full: Request
    source: bytes
    def __getattr__(self,name):return getattr(self.full,name)
    def validate(self):
        self.full.validate()
        ends=[];position=0
        for size in self.full.blocks:position+=size;ends.append(position)
        require(len(self.source)in ends and self.source==self.full.source[:len(self.source)],'exact consumed prefix')
        return self

class BlockReplay(PathReplay):
    def __init__(self,request,index,start,search,prices,cells,repeats):
        length=request.blocks[index]
        super().__init__(PathRequest(request.source[start:start+length],(),request.min_match,request.target,request.literal_mode))
        self.full=request;self.block=index;self.start=start
        self.search=search;self.prices=prices;self.cells=cells
        self.repeats=self.wire_repeats=tuple(repeats)
        self.before=None
        self.search.request=SearchPrefix(request,request.source[:start+length]).validate()
        self.search.short_frontier=self.search.frontier

    def emit(self,*args,**kwargs):
        require(len(self.events)<MAX_EVENTS,'bounded retained block events')
        super().emit(*args,**kwargs)
        e=self.events[-1]
        e['block']=self.block;e['index']=len(self.events)-1;e['ip']+=self.start;e['anchor']+=self.start
        e['sequences']=[[self.start+row[0],*row[1:]]for row in e['sequences']]
        e['search']=self.search.snapshot();e['search_before']=self.before;self.before=None

    def find(self,position,repeats,zero,cur,last):
        require((0 if self.start else 1)<=position<=len(self.r.source)-8,'retained query footprint')
        require(len(repeats)==3 and all(type(v)is int and 0<v<=max(8,self.start+len(self.r.source))for v in repeats),'retained repeat domain')
        words=(self.start+position,*repeats,int(zero),self.r.minimum)
        observation,phase=self.search.query(words)
        self.coverage.add('search_'+phase)
        if position==0:self.coverage.add('block_boundary_query')
        self.before=observation['before'];matches=observation['matches']
        self.emit('query',cur,last,query=list(words),matches=matches)
        return tuple(tuple(pair)for pair in matches)

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
            valid_match(self.full.source, self.start+position, off, length, self.wire_repeats, litlen == 0)
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
        self.ip = 1 if self.start==0 else 0
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


def records(request,kinds):
    require(type(request)is Request,'exact retained request');request.validate()
    require(type(kinds)is tuple and len(kinds)==len(request.blocks) and
            all(type(k)is int and k in (0,1,2)for k in kinds),'publication hypotheses')
    search=SearchReplay(SearchPrefix(request,request.source[:request.blocks[0]]))
    prices=Model(request.literal_mode);cells=[[0]*7 for _ in range(515)]
    owners=[[1,4,8],[0,0,0]];previous=0;start=0;rows=[];coverage=set()
    for index,length in enumerate(request.blocks):
        before=owners[previous].copy();pending=1-previous
        rows.append(dict(record='block_begin',block=index,start=start,length=length,
            previous_owner=previous,owners=[v.copy()for v in owners],frontier=search.frontier))
        replay=BlockReplay(request,index,start,search,prices,cells,before)
        rows.extend(replay.run());coverage.update(replay.coverage)
        candidate=list(replay.wire_repeats);owners[pending]=candidate
        if kinds[index]==2:previous=pending;coverage.add('compressed_commit')
        else:
            coverage.add('raw_rollback'if kinds[index]==0 else'rle_rollback')
            if candidate!=before:coverage.add('discarded_candidate_repeats')
        rows.append(dict(record='block_publish',block=index,kind=kinds[index],
            previous_owner_before=1-pending,previous_owner_after=previous,
            before_repeats=before,candidate_repeats=candidate,owners=[v.copy()for v in owners],
            sequences=len(replay.sequences),tail=length-replay.anchor))
        start+=length
    require(start==len(request.source),'complete retained source')
    return rows,coverage


def observations(request,kinds):
    rows,_=records(request,kinds)
    yield dict(record='metadata',protocol='ZOPRT01',revision=REVISION,source_bytes=len(request.source),
        source_hash=fnv(request.source),parameters=[request.window_log,request.hash_log,request.chain_log,
        request.search_log,request.min_match,request.target,request.literal_mode],
        blocks=list(request.blocks),path_cells=515,passes=2,bias=2)
    for padding in range(2):
        for row in rows:
            if row['record']=='event':
                yield dict(row,padding_pass=padding,source_unchanged=True,guards_unchanged=True,path_tail_zero=True)
            else:yield dict(row,padding_pass=padding)
    yield dict(record='complete',passes=2,blocks=len(request.blocks),
        events=2*sum(r['record']=='event'for r in rows),source_unchanged=True,guards_unchanged=True)


def encoded(request,kinds):
    data=b''.join((json.dumps(r,sort_keys=True,separators=(',',':'))+'\n').encode()for r in observations(request,kinds))
    require(len(data)<=MAX_OUTPUT,'bounded retained output')
    return data


def validate_output(data,request,kinds):
    require(type(data)is bytes and 0<len(data)<=MAX_OUTPUT and data.endswith(b'\n'),'bounded complete retained observations')
    stream=io.BytesIO(data)
    for expected in observations(request,kinds):
        line=stream.readline(MAX_LINE+1)
        require(0<len(line)<=MAX_LINE and line.endswith(b'\n'),'bounded retained line')
        same(parse(line),expected,'independent retained state/publication')
    require(stream.read(1)==b'','exact retained observation EOF')
