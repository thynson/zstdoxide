#!/usr/bin/env python3
"""Retained transport/state controls and fresh-model reconciliation; no children."""
import dataclasses
import json
import struct
import sys
from oracles.optimal_retained import parse_packet,require
from oracles.optimal_retained_recipes import recipes
from oracles.optimal_retained_validation import records,encoded,validate_output
from oracles.optimal_startup import Request as FreshRequest
from oracles.optimal_startup_validation import Replay as FreshReplay


def check():
    children=[]
    def guard(event,args):
        if event in ('subprocess.Popen','os.system','os.posix_spawn','os.fork'):
            children.append(event);raise ValueError('no child execution')
    sys.addaudithook(guard)
    rejected=0
    def reject(fn):
        nonlocal rejected
        try:fn()
        except(ValueError,TypeError):rejected+=1
        else:raise RuntimeError('forgery accepted')
    coverage=set();events=queries=sequences=0
    cases=recipes()
    for name,recipe in cases.items():
        r=recipe.request;require(parse_packet(r.packet())==r,'exact retained transport round trip')
        rows,branches=records(r,recipe.kinds);coverage.update(branches)
        current=[e for e in rows if e['record']=='event'];events+=len(current)
        queries+=sum(e['query']is not None for e in current)
        sequences+=sum(e['sequences']for e in rows if e['record']=='block_publish')
        fresh=FreshReplay(FreshRequest(r.source[:r.blocks[0]],r.window_log,r.hash_log,r.chain_log,
            r.search_log,r.min_match,r.target,r.literal_mode)).run()
        first=[e for e in current if e['block']==0];require(len(first)==len(fresh),'fresh event count')
        for observed,want in zip(first,fresh):
            got={k:v for k,v in observed.items()if k not in ('block','index')}
            require(all(c==[0]*7 for c in got['cells'][r.blocks[0]+3:]),'fresh unused path cells')
            got['cells']=got['cells'][:r.blocks[0]+3]
            require(got==want,'independent fresh-model agreement')
        for index in range(1,len(r.blocks)):
            first=next(e for e in current if e['block']==index)
            require(first['ip']==sum(r.blocks[:index]) and first['search']['short_frontier']==first['search']['frontier'],
                'retained block boundary and local short frontier')
    require({'discarded_candidate_repeats','raw_rollback','rle_rollback','block_boundary_query','compressed_commit',
             'search_short','search_tree','search_repeat','multiple_sequences'}<=coverage,'publication and search coverage')
    base=cases['retained_uniform'];packet=base.request.packet()
    for data in [b'',packet[:43],packet[:-1],packet+b'!',b'!'+packet[1:]]:reject(lambda d=data:parse_packet(d))
    for offset,value in [(8,1025),(12,1),(12,5),(16,9),(16,13),(20,5),(20,10),(24,5),(24,10),
                         (28,0),(28,5),(32,2),(32,8),(36,4096),(40,2),(44,63),(44,513)]:
        changed=packet[:offset]+struct.pack('<I',value)+packet[offset+4:];reject(lambda d=changed:parse_packet(d))
    for field,value in [('source',bytearray(base.request.source)),('blocks',[64,64]),('blocks',(64,True)),('target',True)]:
        reject(lambda f=field,v=value:dataclasses.replace(base.request,**{f:v}).validate())
    good=encoded(base.request,base.kinds);validate_output(good,base.request,base.kinds)
    for kind in ['before-owner','after-owner','candidate-repeats','committed-repeats','kind','block-start',
                 'query','short-frontier','path-cell','frequency','phase','padding','sequence']:
        rows=[json.loads(line)for line in good.splitlines()]
        pub=next(r for r in rows if r['record']=='block_publish'and r['block']==1)
        event=next(r for r in rows if r.get('stage')=='query'and r['block']==1)
        if kind=='before-owner':pub['previous_owner_before']^=1
        elif kind=='after-owner':pub['previous_owner_after']^=1
        elif kind=='candidate-repeats':pub['candidate_repeats'][0]+=1
        elif kind=='committed-repeats':pub['owners'][pub['previous_owner_after']][0]+=1
        elif kind=='kind':pub['kind']=2
        elif kind=='block-start':next(r for r in rows if r['record']=='block_begin'and r['block']==1)['start']+=1
        elif kind=='query':event['query'][0]+=1
        elif kind=='short-frontier':event['search_before']['short_frontier']+=1
        elif kind=='path-cell':event['cells'][0][0]+=1
        elif kind=='frequency':event['frequencies']['ll'][0]+=1
        elif kind=='phase':event['phase']='reverse'
        elif kind=='padding':event['padding_pass']=1
        elif kind=='sequence':next(r for r in rows if r.get('stage')=='sequence')['sequences'][0][3]+=1
        data=b''.join((json.dumps(r)+'\n').encode()for r in rows)
        reject(lambda d=data:validate_output(d,base.request,base.kinds))
    for data in [good[:-1],good+b'{}\n',b'\n'.join(good.splitlines()[:-1])+b'\n']:
        reject(lambda d=data:validate_output(d,base.request,base.kinds))
    require(not children,'no child processes')
    return dict(status='passed',cases=12,events=events,queries=queries,sequences=sequences,
        coverage=sorted(coverage),rejected=rejected,child_processes=0,
        qualification='Predictive retained state and frozen publication hypotheses, not C observation evidence.')

if __name__=='__main__':print(json.dumps(check(),sort_keys=True))
