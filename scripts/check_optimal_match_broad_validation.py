#!/usr/bin/env python3
"""Verify chunked replay equivalence and broader boundaries before C execution."""
from copy import deepcopy
from dataclasses import replace
import json
import sys
from oracles.optimal_match_validation_v2 import Replay as ScalarReplay
from oracles.optimal_match_broad_validation import Replay,expected,validate
from oracles.optimal_match_recipes import recipes as tiny
from oracles.optimal_match_broad_recipes import recipes
from oracles.optimal_matches_broad import REVISION,require
from oracles.tree_validation import canonical


def records(request):
    observations=expected(request)
    result=[dict(record='metadata',protocol='ZOPMB02',revision=REVISION,source_bytes=len(request.source),
                 queries=len(request.queries),bias=2,parameters=[request.window_log,request.hash_log,
                 request.chain_log,request.search_log,request.min_match,request.target,request.hash3_log])]
    for padding_pass in range(2):
        result.extend(dict(record='operation',padding_pass=padding_pass,index=index,
                           source_unchanged=True,guards_unchanged=True,**observation)
                      for index,observation in enumerate(observations))
    result.append(dict(record='complete',operations=len(observations),passes=2,allocations=0))
    return result


def encode(values):return b''.join(canonical(v)+b'\n' for v in values)


def main():
    children=[]
    def guard(event,args):
        if event in ('subprocess.Popen','os.system','os.posix_spawn','os.fork'):
            children.append(event);raise ValueError('children forbidden')
    sys.addaudithook(guard)
    comparisons=0
    for recipe in tiny():
        scalar=ScalarReplay(recipe.request);chunked=Replay(recipe.request)
        for query in recipe.request.queries:
            require(scalar.query(query)==chunked.query(query),'complete tiny scalar/chunk equivalence');comparisons+=1
    base=tiny()[0].request
    sources=[b'a'*8200,b'a'*4095+b'b'+b'a'*4104,bytes(range(256))*33]
    for source in sources:
        request=replace(base,source=source)
        scalar=ScalarReplay(request);chunked=Replay(request)
        for a in (3,4,8,4095,4096,4097,len(source)-6):
            for b in (2,3,a-1):
                if b<a:
                    require(scalar.length(a,b)==chunked.length(a,b),'chunk boundary source length');comparisons+=1
    values=recipes();branch_counts={};queries=0
    for recipe in values:
        replay=Replay(recipe.request)
        for q in recipe.request.queries:
            _,phase=replay.query(q);branch_counts[phase]=branch_counts.get(phase,0)+1
        result=validate(recipe.request,encode(records(recipe.request)));queries+=result['operations']
    require(queries==82,'broader query count')
    for window in (18,19):
        for distance in (262143,262144,262145):
            r=next(r for r in values if r.name==f'om_broad_distance_w{window}_d{distance}')
            matches=expected(r.request)[1]['matches']
            require(matches==([[distance+3,3]] if distance<262144 else []),'strict short-key distance cutoff')
    for ll0 in (0,1):
        for target in (0,3,4,7,8,11,12,4095,4096):
            r=next(r for r in values if r.name==f'om_broad_repeat_ll{ll0}_t{target}')
            matches=expected(r.request)[0]['matches']
            count=1 if target<4 else 2 if target<8 else 3
            require(matches==[[1,4],[2,8],[3,12]][:count],'strict repeat target threshold')
    # Coherently encoded model records still need exact independent re-evaluation.
    r=next(r.request for r in values if r.name=='om_broad_repeat_ll0_t0')
    original=records(r);rejected=[]
    def reject(label,rows):
        try:validate(r,encode(rows))
        except ValueError:rejected.append(label)
        else:raise ValueError('accepted '+label)
    for index,path,value in [(0,['parameters'],[17,12,12,4,3,0,16]),(0,['bias'],True),
                            (1,['before','frontier'],0),(1,['after','frontier'],67),
                            (1,['after','short_frontier'],66),(1,['after','short_heads',0],2),
                            (1,['after','heads',0],2),(1,['after','nodes',0,1],2),
                            (1,['matches'],[[1,5]]),(1,['matches'],[[2,4]]),
                            (1,['query',1],49),(1,['guards_unchanged'],False),
                            (2,['padding_pass'],0),(3,['allocations'],1)]:
        rows=deepcopy(original);target=rows[index]
        for field in path[:-1]:target=target[field]
        target[path[-1]]=value;reject(str(path),rows)
    require(not children,'no child attempts')
    print(json.dumps(dict(status='passed',recipes=37,queries=queries,scalar_comparisons=comparisons,
                          publication_branches=branch_counts,forged_rejections=len(rejected),child_attempts=0,
                          qualification='Synthetic wider replay and hand-derived boundaries only, before C execution.'),indent=2))


if __name__=='__main__':main()
