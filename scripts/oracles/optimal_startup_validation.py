"""Predictive integer composition of qualified search and path rules.

This model emits no C evidence and does not change production startup admission.
The subsequent unchanged-provider CCtx observation is a separate gate.
"""
import io
import json
from .optimal_startup import Request, require, REVISION
from .optimal_paths import Request as PathRequest
from .optimal_path_validation import Replay as PathReplay
from .optimal_match_broad_validation import Replay as SearchReplay
from .optimal_price_validation import fnv
from .tree_validation import parse, same

MAX_EVENTS=1024
MAX_OUTPUT=128<<20
MAX_LINE=1<<20

class Replay(PathReplay):
    def __init__(self,request):
        require(type(request)is Request,'exact fresh startup request')
        request.validate()
        super().__init__(PathRequest(request.source,(),request.min_match,request.target,request.literal_mode))
        self.request=request
        self.search=SearchReplay(request)
        self.before=None
        self.search_phases=set()

    def emit(self,*args,**kwargs):
        require(len(self.events)<MAX_EVENTS,'bounded ordinary event count')
        super().emit(*args,**kwargs)
        self.events[-1]['search']=self.search.snapshot()
        self.events[-1]['search_before']=self.before
        self.before=None

    def find(self,position,repeats,zero,cur,last):
        require(1<=position<=len(self.r.source)-8,'ordinary query footprint')
        require(type(repeats)is tuple and len(repeats)==3 and all(type(v)is int and 0<v<=max(8,len(self.r.source)) for v in repeats),'published repeats')
        words=(position,*repeats,int(zero),self.r.minimum)
        observation,phase=self.search.query(words)
        self.search_phases.add(phase)
        if any(v>position for v in repeats):self.coverage.add('fresh_unavailable_repeats')
        if zero:self.coverage.add('query_zero_literals')
        self.before=observation['before']
        matches=observation['matches']
        self.emit('query',cur,last,query=list(words),matches=matches)
        return tuple(tuple(pair)for pair in matches)


def observations(request):
    replay=Replay(request);events=replay.run()
    yield dict(record='metadata',protocol='ZOPST01',revision=REVISION,source_bytes=len(request.source),
        source_hash=fnv(request.source),bias=2,initial_repeats=[1,4,8],
        parameters=[request.window_log,request.hash_log,request.chain_log,request.search_log,request.min_match,request.target,request.literal_mode],
        path_cells=len(request.source)+3,passes=2)
    for padding_pass in range(2):
        for index,event in enumerate(events):
            yield dict(event,padding_pass=padding_pass,index=index,source_unchanged=True,guards_unchanged=True,path_tail_zero=True)
    yield dict(record='complete',events=2*len(events),passes=2,sequences=len(replay.sequences),guards_unchanged=True,source_unchanged=True)


def encoded(request):
    data=b''.join((json.dumps(row,sort_keys=True,separators=(',',':'))+'\n').encode()for row in observations(request))
    require(len(data)<=MAX_OUTPUT,'bounded predictive output')
    return data


def validate_output(data,request):
    require(type(data)is bytes and 0<len(data)<=MAX_OUTPUT and data.endswith(b'\n'),'bounded complete observation bytes')
    stream=io.BytesIO(data)
    for expected in observations(request):
        line=stream.readline(MAX_LINE+1)
        require(0<len(line)<=MAX_LINE and line.endswith(b'\n'),'bounded observation line')
        same(parse(line),expected,'complete independent fresh startup state')
    require(stream.read(1)==b'','exact observation EOF')
    return True
