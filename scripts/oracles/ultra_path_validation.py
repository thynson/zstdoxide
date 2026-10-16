"""Independent coupled optLevel0/2 path replay before native execution.

Pinned upstream counterparts: ZSTD_compressBlock_opt_generic and ZSTD_newRep.
Prices reuse the independently reconciled frequency model. Complete path events
are canonical copied deltas; no C or production Rust is executed.
"""
from .optimal_path_wide_validation import Replay as Prior
from .optimal_path_validation import INFINITY,PRICE,OFF,ML,LL
from .optimal_paths_wide import require,next_repeats
from .optimal_price_validation import finite,fnv
from .ultra_frequency import Model
from .ultra_paths import Request,REVISION,MAX_EVENTS,MAX_OUTPUT

class Replay(Prior):
    def __init__(self,request):
        require(type(request)is Request,'exact policy-tagged path request')
        request.validate()
        super().__init__(request.original)
        self.policy=request.policy
        self.prices=Model(request.policy,request.original.literal_mode)
        self.witnesses=[]
    def emit(self,stage,cur=None,last=None,stretch=None,reverse=None,query=None,matches=None,tail=None,alternative=None):
        super().emit(stage,cur,last,stretch,reverse,query,matches,tail)
        self.events[-1].update(price_policy=self.policy,alternative=alternative)
    def forward(self,last):
        if self.policy==0:return super().forward(last)
        cur=1
        while cur<=last:
            previous=self.cell(cur-1)
            litlen=previous[LL]+1
            price=finite(previous[PRICE]+self.prices.literal_price(self.r.source[self.ip+cur-1:self.ip+cur])+self.prices.ll_price(litlen)-self.prices.ll_price(litlen-1))
            if price<=self.cell(cur)[PRICE]:
                self.coverage.add('literal_tie'if price==self.cell(cur)[PRICE]else'literal_improvement')
                replaced=self.cell(cur).copy()
                self.cells[cur]=previous.copy()
                self.cell(cur)[LL],self.cell(cur)[PRICE]=litlen,price
                increment=self.prices.ll_price(1)-self.prices.ll_price(0)
                if replaced[LL]==0 and increment<0 and self.ip+cur<len(self.r.source):
                    self.coverage.add('ultra_alternative_considered')
                    literal=self.prices.literal_price(self.r.source[self.ip+cur:self.ip+cur+1])
                    with_one=finite(replaced[PRICE]+literal+increment)
                    with_more=finite(price+literal+self.prices.ll_price(litlen+1)-self.prices.ll_price(litlen))
                    require(cur>=replaced[ML],'alternative predecessor index')
                    self.emit('alternative',cur,last,alternative=[*replaced,increment,with_one,with_more,self.cell(cur+1)[PRICE],cur-replaced[ML]])
                    accepted=with_one<with_more and with_one<self.cell(cur+1)[PRICE]
                    self.witnesses.append(dict(ip=self.ip,cur=cur,last=last,replaced=replaced.copy(),predecessor=self.cell(cur-replaced[ML]).copy(),increment=increment,with_one=with_one,with_more=with_more,next_price=self.cell(cur+1)[PRICE],accepted=accepted))
                    if accepted:
                        require(cur>=replaced[ML],'alternative predecessor index')
                        predecessor=self.cell(cur-replaced[ML])
                        reps=next_repeats(replaced[OFF],tuple(predecessor[4:]),predecessor[LL]==0)
                        self.cells[cur+1]=replaced.copy()
                        self.cell(cur+1)[4:]=reps
                        self.cell(cur+1)[LL],self.cell(cur+1)[PRICE]=1,with_one
                        if last<cur+1:self.coverage.add('ultra_alternative_extends_frontier');last=cur+1
                        self.coverage.add('ultra_alternative_selected')
                    else:self.coverage.add('ultra_alternative_rejected')
            else:self.coverage.add('literal_rejected')
            current=self.cell(cur)
            require(cur>=current[ML],'match predecessor index')
            if current[LL]==0:
                predecessor=self.cell(cur-current[ML])
                current[4:]=next_repeats(current[OFF],tuple(predecessor[4:]),predecessor[LL]==0)
                self.coverage.add('deferred_repeat_publication')
            self.emit('literal',cur,last)
            if self.ip+cur>len(self.r.source)-8:
                self.coverage.add('tail_no_query');cur+=1;continue
            if cur==last:break
            if self.cell(cur+1)[PRICE]<=current[PRICE]+128:self.coverage.add('ultra_unpromising_query')
            matches=self.find(self.ip+cur,tuple(current[4:]),current[LL]==0,cur,last)
            if not matches:cur+=1;continue
            off,longest=matches[-1]
            if longest>self.r.target or cur+longest>=4096 or self.ip+cur+longest>=len(self.r.source):
                self.coverage.add('later_sufficient'if longest>self.r.target else'later_horizon'if cur+longest>=4096 else'later_source_end')
                return cur,cur+longest,[0,off,longest,0,0,0,0],True
            base=finite(current[PRICE]+self.prices.ll_price(0));previous_length=self.r.minimum-1
            for off,end in matches:
                rejected=False
                for length in range(end,previous_length,-1):
                    position=cur+length;price=finite(base+self.prices.match_price(off,length))
                    if position>last or price<self.cell(position)[PRICE]:
                        self.coverage.add('match_extend'if position>last else'match_improvement')
                        if rejected:self.coverage.add('ultra_improvement_after_rejection')
                        while last<position:
                            last+=1;self.cell(last)[PRICE],self.cell(last)[LL]=INFINITY,1
                        destination=self.cell(position)
                        destination[ML],destination[OFF],destination[LL],destination[PRICE]=length,off,0,price
                    else:
                        self.coverage.add('ultra_match_tie_continue'if price==self.cell(position)[PRICE]else'ultra_match_worse_continue')
                        rejected=True
                previous_length=end
            self.cell(last+1)[PRICE]=INFINITY
            self.emit('relax',cur,last);cur+=1
        stretch=self.cell(last).copy();require(last>=stretch[ML],'last predecessor')
        return last-stretch[ML],last,stretch,False


def observations(request):
    replay=Replay(request);replay.run();original=request.original
    yield dict(record='metadata',protocol='ZUPTH01',revision=REVISION,source_bytes=len(original.source),source_hash=fnv(original.source),script_positions=len(original.scripts),parameters=[original.min_match,original.target,original.literal_mode],price_policy=request.policy,path_cells=len(replay.cells),passes=2,path_encoding='canonical-delta-from-zero')
    for padding_pass in range(2):
        for index,event in enumerate(replay.events):
            yield dict(event,padding_pass=padding_pass,index=index,source_unchanged=True,guards_unchanged=True,path_tail_zero=True)
    yield dict(record='complete',events=2*len(replay.events),passes=2,sequences=len(replay.sequences),price_policy=request.policy,guards_unchanged=True,source_unchanged=True)


from functools import lru_cache
import json

@lru_cache(maxsize=58)
def _expected_json(request):
    return tuple(json.dumps(row,sort_keys=True,separators=(',',':'))for row in observations(request))

def event_count(request):
    return (len(_expected_json(request))-2)//2

def validate_output(data,request):
    import io
    from .ultra_scalar import load
    require(type(data)is bytes and 0<len(data)<=MAX_OUTPUT and data.endswith(b'\n'),'bounded path trace')
    stream=io.BytesIO(data)
    for wanted in _expected_json(request):
        line=stream.readline((512<<10)+1)
        require(0<len(line)<=(512<<10)and line.endswith(b'\n'),'bounded observation line')
        require(json.dumps(load(line),sort_keys=True,separators=(',',':'))==wanted,'exact coupled path replay')
    require(stream.read()==b'','no trailing path trace');return True
