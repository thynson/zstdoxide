"""Source-driven ordinary coupled-policy replay; no native or Rust outputs as inputs.

Search state is independently advanced at every policy-dependent query. The new
path branch reuses the independently native-qualified scripted transition rules.
"""
import io,json
from .ultra_fresh import Request,REVISION,require
from .optimal_fresh_wide_validation import BlockReplay as Prior,PATH_CELLS,MAX_EVENTS,MAX_OUTPUT,MAX_LINE,SearchPrefix,SearchReplay
from .ultra_path_validation import Replay as UltraPath
from .ultra_frequency import Model
from .optimal_price_validation import fnv
from .ultra_scalar import load

class BlockReplay(Prior):
    def __init__(self,request,index,start,search,prices,cells,repeats,path_baseline,search_baseline):
        request.validate()
        super().__init__(request.original,index,start,search,prices,cells,repeats,path_baseline,search_baseline)
        self.policy=request.policy
        self.witnesses=[]
    def emit(self,stage,cur=None,last=None,stretch=None,reverse=None,query=None,matches=None,tail=None,alternative=None):
        super().emit(stage,cur,last,stretch,reverse,query,matches,tail)
        self.events[-1].update(price_policy=self.policy,alternative=alternative)
    def forward(self,last):
        if self.policy==0:return super().forward(last)
        return UltraPath.forward(self,last)

def records(request,kinds):
    require(type(request)is Request,'exact coupled fresh request');request.validate()
    policy=request.policy; tagged=request;request=request.original
    require(type(kinds)is tuple and len(kinds)==len(request.blocks)
            and all(type(k)is int and k in (0,1,2) for k in kinds),'publication hypotheses')
    search=SearchReplay(SearchPrefix(request,request.source[:request.blocks[0]]))
    prices=Model(policy,request.literal_mode);cells=[[0]*7 for _ in range(PATH_CELLS)]
    path_baseline=[[0]*7 for _ in range(PATH_CELLS)]
    size=(1<<request.hash_log)+(1<<request.chain_log)+(1<<request.hash3_log if request.hash3_log else 0)+2
    search_baseline=[0]*size
    owners=[[1,4,8],[0,0,0]];previous=0;start=0;rows=[];coverage=set()
    for index,length in enumerate(request.blocks):
        before=owners[previous].copy();pending=1-previous
        rows.append(dict(record='block_begin',block=index,start=start,length=length,
            previous_owner=previous,owners=[v.copy() for v in owners],frontier=search.frontier))
        replay=BlockReplay(tagged,index,start,search,prices,cells,before,path_baseline,search_baseline)
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
    for row in rows:row['price_policy']=policy
    return rows,coverage


def observations(request,kinds):
    rows,_=records(request,kinds);r=request.original
    yield dict(record='metadata',protocol='ZUFRS01',revision=REVISION,source_bytes=len(r.source),
        source_hash=fnv(r.source),parameters=[r.window_log,r.hash_log,r.chain_log,r.search_log,r.min_match,r.target,r.literal_mode],
        blocks=list(r.blocks),path_cells=PATH_CELLS,passes=2,bias=2,state_encoding='canonical-deltas-from-zero',price_policy=request.policy)
    for padding in range(2):
        for row in rows:
            if row['record']=='event':yield dict(row,padding_pass=padding,source_unchanged=True,guards_unchanged=True,path_tail_zero=True)
            else:yield dict(row,padding_pass=padding)
    yield dict(record='complete',passes=2,blocks=len(r.blocks),events=2*sum(row['record']=='event' for row in rows),source_unchanged=True,guards_unchanged=True,price_policy=request.policy)

def validate_output(data,request,kinds):
    require(type(data)is bytes and 0<len(data)<=MAX_OUTPUT and data.endswith(b'\n'),'bounded fresh observations')
    stream=io.BytesIO(data)
    for expected in observations(request,kinds):
        line=stream.readline(MAX_LINE+1)
        require(0<len(line)<=MAX_LINE and line.endswith(b'\n'),'bounded observation line')
        require(json.dumps(load(line),sort_keys=True,separators=(',',':'))==json.dumps(expected,sort_keys=True,separators=(',',':')),'exact coupled ordinary replay')
    require(stream.read(1)==b'','exact observation EOF')
