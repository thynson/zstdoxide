"""Independent broader replay with bounded equal-slice source comparison.

The checked integer-tree rules are the reconciled tiny v2 model. Only source
comparison changes: complete equal chunks precede bytewise mismatch counting.
"""
import io
from .optimal_matches_broad import require, REVISION
from .optimal_match_validation_v2 import Replay as ScalarReplay
from .tree_validation import parse, same

MAX_OUTPUT=64<<20
MAX_LINE=8<<20


class Replay(ScalarReplay):
    def length(self,a,b):
        require(2<=b<a<=len(self.request.source)-6,'ordered source positions')
        source=self.request.source
        a-=2;b-=2
        available=len(source)-a
        count=0
        while count+4096<=available:
            if source[a+count:a+count+4096]!=source[b+count:b+count+4096]:
                break
            count+=4096
        while count<available and source[a+count]==source[b+count]:
            count+=1
        return count


def expected(request):
    replay=Replay(request)
    return [replay.query(q)[0] for q in request.queries]


def validate(request,data):
    request.validate()
    require(type(data) is bytes and 0<len(data)<=MAX_OUTPUT and data.endswith(b'\n'),
            'bounded newline-terminated observations')
    stream=io.BytesIO(data)
    def record():
        line=stream.readline(MAX_LINE+1)
        require(0<len(line)<=MAX_LINE and line.endswith(b'\n'),'bounded observation line')
        return parse(line)
    same(record(),dict(record='metadata',protocol='ZOPMB02',revision=REVISION,
                       source_bytes=len(request.source),queries=len(request.queries),bias=2,
                       parameters=[request.window_log,request.hash_log,request.chain_log,
                                   request.search_log,request.min_match,request.target,request.hash3_log]),'exact request metadata')
    observations=expected(request)
    for padding_pass in range(2):
        for index,observation in enumerate(observations):
            wanted=dict(record='operation',padding_pass=padding_pass,index=index,
                        source_unchanged=True,guards_unchanged=True,**observation)
            same(record(),wanted,'independent complete broader search state')
    same(record(),dict(record='complete',operations=len(observations),passes=2,allocations=0),'complete bounded trailer')
    require(stream.read(1)==b'','exact observation EOF')
    return dict(operations=len(observations),matches=sum(len(o['matches']) for o in observations),
                records=2*len(observations)+2,stdout_bytes=len(data))
