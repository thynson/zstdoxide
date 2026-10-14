#!/usr/bin/env python3
"""Check independent tiny search replay before any upstream capture."""
from collections import Counter
from copy import deepcopy
from dataclasses import replace
import json
import sys
from oracles.optimal_matches import Request, REVISION, require
from oracles.optimal_match_recipes import recipes
from oracles.optimal_match_validation import Replay, bucket, expected, validate
from oracles.tree_validation import canonical


def records(request):
    observations = expected(request)
    result = [dict(record='metadata', protocol='ZOPMT01', revision=REVISION,
                   source_bytes=len(request.source), queries=len(request.queries), bias=2,
                   parameters=[request.window_log,request.hash_log,request.chain_log,
                               request.search_log,request.min_match,request.target,request.hash3_log])]
    for padding_pass in range(2):
        result.extend(dict(record='operation',padding_pass=padding_pass,index=index,
                           source_unchanged=True,guards_unchanged=True,**value)
                      for index,value in enumerate(observations))
    result.append(dict(record='complete',operations=len(observations),passes=2,allocations=0))
    return result


def encode(values):
    return b''.join(canonical(value)+b'\n' for value in values)


def main():
    children=[]
    def guard(event,args):
        if event in ('subprocess.Popen','os.system','os.posix_spawn','os.fork'):
            children.append(event)
            raise ValueError('child execution forbidden')
    sys.addaudithook(guard)
    cases=recipes()
    phases=Counter()
    for case in cases:
        replay=Replay(case.request)
        for query in case.request.queries:
            _,phase=replay.query(query)
            phases[phase]+=1
        result=validate(case.request,encode(records(case.request)))
        require(result['operations']==len(case.request.queries),'recipe observation inventory')
    # Literal hand calculations, independent of the generic transcript builder.
    request=Request(b'a'*16,((0,0,0,0,0,4),(1,0,0,0,0,4),(2,0,0,0,0,4)),target=4096)
    replay=Replay(request)
    first,phase=replay.query(request.queries[0])
    require(phase=='tree' and first['matches']==[] and first['after']['frontier']==3,'first empty tree')
    second,phase=replay.query(request.queries[1])
    require(phase=='tree' and second['matches']==[[4,15]] and second['after']['frontier']==9,'distance-one full tail')
    third,phase=replay.query(request.queries[2])
    require(phase=='skipped' and third['matches']==[] and third['before']==third['after'],'skipped publication')
    short=cases[0].request
    observations=expected(short)
    require(observations[2]['matches']==[[7,3]] and observations[2]['after']['frontier']==7
            and observations[2]['after']['short_frontier']==6,'length-three short-key match')
    key=bucket(short.source,6,short.hash_log,3)
    require(observations[2]['after']['heads'][key]!=6,'short early return omits main head')
    ordinary=expected(replace(short,target=3))
    require(ordinary[2]['matches']==[[7,3]] and ordinary[2]['after']['heads'][key]==6,'strict target equality inserts tree')
    for name in ('om_repeats_0','om_repeats_1'):
        request=next(c.request for c in cases if c.name==name)
        require(expected(request)[0]['matches']==[[1,4],[2,8],[3,12]],'literal-zero repeat intent order')
        early=replace(request,target=0)
        replay=Replay(early);observation,phase=replay.query(early.queries[0])
        require(phase=='repeat' and observation['matches']==[[1,4]]
                and observation['after']['frontier']==66
                and observation['after']['short_frontier']==2,'repeat early publication')
        phases[phase]+=1
    require(set(phases)=={'tree','short','repeat','skipped'},'all publication branches')
    request=short
    original=records(request)
    rejected=[]
    def reject(name,data):
        try:validate(request,data)
        except (ValueError,UnicodeDecodeError):rejected.append(name)
        else:raise ValueError('accepted forged observation '+name)
    def change(name,index,path,value):
        modified=deepcopy(original);target=modified[index]
        for field in path[:-1]:target=target[field]
        target[path[-1]]=value
        reject(name,encode(modified))
    for field,value in [('revision','wrong'),('source_bytes',True),('queries',6),('bias',0),('parameters',[10,6,6,1,3,1,6]),('unknown',1)]:
        change('metadata '+field,0,[field],value)
    for field,value in [('index',True),('padding_pass',1),('source_unchanged',False),('guards_unchanged',1),('query',[0,0,0,0,0,4]),('unknown',0)]:
        change('operation '+field,1,[field],value)
    for state in ('before','after'):
        for field,value in [('frontier',99),('short_frontier',99),('heads',[]),('nodes',[]),('short_heads',[])]:
            change(state+' '+field,1,[state,field],value)
        change(state+' single head',1,[state,'heads',0],99)
        change(state+' single edge',1,[state,'nodes',0,1],99)
        change(state+' single short head',1,[state,'short_heads',0],99)
    for value in ([],[[8,3]],[[7,4]],[[7,True]],[[7,3],[7,3]],[[7,3.0]]):
        change('candidate',3,['matches'],value)
    for field,value in [('operations',0),('passes',1),('allocations',1),('unknown',1)]:
        change('trailer '+field,len(original)-1,[field],value)
    raw=encode(original)
    for name,data in [('trailing',raw+b'{}\n'),('truncated',raw[:-1]),('missing-record',encode(original[:-2]+original[-1:])),
                      ('duplicate-key',raw.replace(b'"bias":2',b'"bias":2,"bias":2',1)),
                      ('non-finite',raw.replace(b'"bias":2',b'"bias":NaN',1)),('not-bytes',bytearray(raw))]:
        reject(name,data)
    require(not children,'no child attempts')
    print(json.dumps(dict(status='passed',recipes=len(cases),queries=273,hand_calculated_examples=8,
                          publication_branches=dict(phases),forged_rejections=len(rejected),
                          child_attempts=0,qualification='Synthetic model transcripts only; no C execution or observed C expectations.'),indent=2))


if __name__=='__main__':main()
