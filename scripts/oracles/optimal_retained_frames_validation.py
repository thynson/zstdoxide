"""Larger-source replay reusing immutable qualified scalar and sparse transitions.

Only the transport identity/source ceiling changes. Native matches remain outputs;
all provider searches are derived independently from the consumed source prefix.
"""
import io
from .optimal_retained_frames import Request,REVISION,MAX_OUTPUT,require
from .optimal_retained_wide_validation import BlockReplay,PATH_CELLS,MAX_LINE
from .optimal_retained_validation_v2 import SearchPrefix
from .optimal_match_broad_validation import Replay as SearchReplay
from .optimal_price_validation import Model,fnv
from .tree_validation import parse,same

def records(request,kinds):
    require(type(request)is Request,'exact larger retained request');request.validate()
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
    yield dict(record='metadata',protocol='ZOPRS03',revision=REVISION,source_bytes=len(request.source),
        source_hash=fnv(request.source),parameters=[request.window_log,request.hash_log,request.chain_log,
        request.search_log,request.min_match,request.target,request.literal_mode],blocks=list(request.blocks),
        path_cells=PATH_CELLS,passes=2,bias=2,state_encoding='canonical-deltas-from-zero')
    for padding in range(2):
        for row in rows:
            if row['record']=='event':yield dict(row,padding_pass=padding,source_unchanged=True,guards_unchanged=True,path_tail_zero=True)
            else:yield dict(row,padding_pass=padding)
    yield dict(record='complete',passes=2,blocks=len(request.blocks),events=2*sum(r['record']=='event' for r in rows),source_unchanged=True,guards_unchanged=True)


def validate_output(data,request,kinds):
    require(type(data)is bytes and 0<len(data)<=MAX_OUTPUT and data.endswith(b'\n'),'bounded newline-terminated larger retained output')
    stream=io.BytesIO(data)
    for expected in observations(request,kinds):
        line=stream.readline(MAX_LINE+1)
        require(0<len(line)<=MAX_LINE and line.endswith(b'\n'),'bounded sparse line')
        same(parse(line),expected,'independent larger sparse state/publication')
    require(stream.read(1)==b'','exact larger observation EOF')
