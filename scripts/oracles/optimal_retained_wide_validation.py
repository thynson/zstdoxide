"""Independent sparse complete-state replay for wider ordinary retained Optimal.

Reuse qualified scalar transitions, not captured matches as provider inputs.
All old captures must project losslessly before new native execution.
"""
import io
import json
from .optimal_retained_wide import Request,REVISION,MAX_EVENTS,MAX_OUTPUT,require
from .optimal_retained_validation_v2 import BlockReplay as SmallBlock, SearchPrefix
from .optimal_paths_wide import Request as PathRequest
from .optimal_match_broad_validation import Replay as SearchReplay
from .optimal_price_validation import Model,fnv
from .tree_validation import parse,same

PATH_CELLS=4099
MAX_LINE=2<<20


def flattened(state):
    return state['heads']+[v for pair in state['nodes'] for v in pair]+state['short_heads']+[state['frontier'],state['short_frontier']]


def delta(previous,current,advance):
    require(len(previous)==len(current),'fixed sparse owner extent')
    changes=[]
    for index,(before,after) in enumerate(zip(previous,current)):
        if before!=after:
            value=after.copy() if type(after)is list else after
            changes.append([index,value])
            if advance:previous[index]=value.copy() if type(value)is list else value
    return dict(length=len(previous),changes=changes)


class BlockReplay(SmallBlock):
    def __init__(self,request,index,start,search,prices,cells,repeats,path_baseline,search_baseline):
        self.r=PathRequest(request.source[start:start+request.blocks[index]],(),request.min_match,request.target,request.literal_mode).validate()
        self.full=request;self.block=index;self.start=start
        current=start+2
        if current>search.frontier+384:search.frontier=current-min(192,current-search.frontier-384)
        self.search=search;self.prices=prices;self.cells=cells
        self.repeats=self.wire_repeats=tuple(repeats)
        self.before=None
        self.search.request=SearchPrefix(request,request.source[:start+request.blocks[index]]).validate()
        self.search.short_frontier=self.search.frontier
        self.sequences=[];self.events=[];self.coverage=set();self.script={}
        self.phase='forward';self.ip=self.anchor=0
        self.path_baseline=path_baseline;self.search_baseline=search_baseline

    def emit(self,stage,cur=None,last=None,stretch=None,reverse=None,query=None,matches=None,tail=None):
        require(len(self.events)<MAX_EVENTS,'bounded wider retained events')
        require(self.phase in ('forward','reverse'),'path phase')
        before=None if self.before is None else delta(self.search_baseline,flattened(self.before),False)
        require(before is None or not before['changes'],'query pre-state equals previous complete search state')
        self.events.append(dict(record='event',stage=stage,phase=self.phase,block=self.block,index=len(self.events),
            ip=self.ip+self.start,anchor=self.anchor+self.start,cur=cur,last=last,
            stretch=None if stretch is None else stretch.copy(),reverse=reverse,query=query,matches=matches,
            tail=tail,repeats=list(self.repeats),cells=delta(self.path_baseline,self.cells,True),
            frequencies={k:v.copy() for k,v in self.prices.tables.items()},sums=self.prices.sums.copy(),
            base_prices=self.prices.cache.copy(),predefined=self.prices.predefined,
            sequences=[[self.start+row[0],*row[1:]] for row in self.sequences],
            search=delta(self.search_baseline,flattened(self.search.snapshot()),True),search_before=before))
        self.before=None

    def forward(self,last):
        had_source_end='later_source_end' in self.coverage
        result=super().forward(last)
        cur,end,stretch,early=result
        if early and stretch[2]<=self.r.target and self.ip+end<len(self.r.source):
            require(end>=4096,'remaining horizon exit')
            if not had_source_end:self.coverage.discard('later_source_end')
            self.coverage.add('later_horizon')
        return result


def records(request,kinds):
    require(type(request)is Request,'exact wider retained request');request.validate()
    require(type(kinds)is tuple and len(kinds)==len(request.blocks)
            and all(type(k)is int and k in (0,1,2) for k in kinds),'publication hypotheses')
    search=SearchReplay(SearchPrefix(request,request.source[:request.blocks[0]]))
    prices=Model(request.literal_mode);cells=[[0]*7 for _ in range(PATH_CELLS)]
    path_baseline=[[0]*7 for _ in range(PATH_CELLS)]
    size=(1<<request.hash_log)+(1<<request.chain_log)+(1<<request.hash3_log if request.hash3_log else 0)+2
    search_baseline=[0]*size
    owners=[[1,4,8],[0,0,0]];previous=0;start=0;rows=[];coverage=set()
    for index,length in enumerate(request.blocks):
        before=owners[previous].copy();pending=1-previous
        rows.append(dict(record='block_begin',block=index,start=start,length=length,
            previous_owner=previous,owners=[v.copy() for v in owners],frontier=search.frontier))
        replay=BlockReplay(request,index,start,search,prices,cells,before,path_baseline,search_baseline)
        rows.extend(replay.run());coverage.update(replay.coverage)
        candidate=list(replay.wire_repeats);owners[pending]=candidate
        if kinds[index]==2:previous=pending;coverage.add('compressed_commit')
        else:
            coverage.add('raw_rollback' if kinds[index]==0 else 'rle_rollback')
            if candidate!=before:coverage.add('discarded_candidate_repeats')
        rows.append(dict(record='block_publish',block=index,kind=kinds[index],
            previous_owner_before=1-pending,previous_owner_after=previous,before_repeats=before,
            candidate_repeats=candidate,owners=[v.copy() for v in owners],sequences=len(replay.sequences),tail=length-replay.anchor))
        start+=length
    require(start==len(request.source),'complete retained source')
    return rows,coverage


def observations(request,kinds):
    rows,_=records(request,kinds)
    yield dict(record='metadata',protocol='ZOPRW02',revision=REVISION,source_bytes=len(request.source),
        source_hash=fnv(request.source),parameters=[request.window_log,request.hash_log,request.chain_log,
        request.search_log,request.min_match,request.target,request.literal_mode],blocks=list(request.blocks),
        path_cells=PATH_CELLS,passes=2,bias=2,state_encoding='canonical-deltas-from-zero')
    for padding in range(2):
        for row in rows:
            if row['record']=='event':yield dict(row,padding_pass=padding,source_unchanged=True,guards_unchanged=True,path_tail_zero=True)
            else:yield dict(row,padding_pass=padding)
    yield dict(record='complete',passes=2,blocks=len(request.blocks),events=2*sum(r['record']=='event' for r in rows),source_unchanged=True,guards_unchanged=True)


def validate_output(data,request,kinds):
    require(type(data)is bytes and 0<len(data)<=MAX_OUTPUT and data.endswith(b'\n'),'bounded newline-terminated wider output')
    stream=io.BytesIO(data)
    for expected in observations(request,kinds):
        line=stream.readline(MAX_LINE+1)
        require(0<len(line)<=MAX_LINE and line.endswith(b'\n'),'bounded sparse line')
        same(parse(line),expected,'independent wider sparse state/publication')
    require(stream.read(1)==b'','exact wider observation EOF')
