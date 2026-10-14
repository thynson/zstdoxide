#!/usr/bin/env python3
"""Reconstruct every small trace and prove wider horizon queries before C capture."""
from copy import deepcopy
import json
from oracles.optimal_path_recipes import recipes as small_recipes
from oracles.optimal_path_validation import Replay as SmallReplay
from oracles.optimal_path_wide_recipes import recipes
from oracles.optimal_path_wide_validation import Replay,apply_delta,observations,validate_output
from oracles.optimal_paths_wide import Request,parse_packet,require


def check():
    compared=0
    for name,small in small_recipes().items():
        request=Request(small.source,small.scripts,small.min_match,small.target,small.literal_mode)
        wide=Replay(request);wide.run();original=SmallReplay(small);original.run()
        require(len(wide.events)==len(original.events),'same event count: '+name)
        previous=[[0]*7 for _ in wide.cells]
        for got,want in zip(wide.events,original.events):
            previous=apply_delta(previous,got['cells']);complete=dict(got,cells=previous)
            require(complete==want,'exact reconstructed small state: '+name);compared+=1
    report=[]
    for name,request in recipes().items():
        require(parse_packet(request.packet())==request,'wider request roundtrip')
        state=Replay(request);state.run()
        previous=[[0]*7 for _ in state.cells]
        for event in state.events:previous=apply_delta(previous,event['cells'])
        require(previous==state.cells,'complete final state')
        queried=[e for e in state.events if e['stage']=='query'and e['query'][0]==2051]
        selected=[e['last']for e in state.events if e['stage']=='select']
        if name.startswith('path_wide_later_'):
            end=int(name.rsplit('_',1)[1]);require(len(queried)==1,'horizon candidate actually queried')
            require(selected[0]==end,'selected relative end')
            require(('later_horizon'in state.coverage)==(end>=4096),'strict horizon branch')
        if name=='path_wide_initial_4096':require('initial_sufficient'in state.coverage,'immediate above horizon')
        if name=='path_wide_initial_4095':require('initial_sufficient'not in state.coverage,'strict sufficient equality')
        if name=='path_wide_source_end':require('later_source_end'in state.coverage and 'later_horizon'not in state.coverage,'source end before horizon')
        if name=='path_wide_literal_pricing':require('later_horizon'in state.coverage,'enabled literal horizon')
        if name=='path_wide_maximum_immediate':require(state.sequences==[[0,1,8191,4]],'long match never indexes its length')
        report.append(dict(name=name,events=len(state.events),queries=sum(e['stage']=='query'for e in state.events),sequences=len(state.sequences),coverage=sorted(state.coverage)))
    prior=[[0]*7 for _ in range(3)]
    good={'length':3,'changes':[[1,[3,4,5,6,7,8,9]]]}
    require(apply_delta(prior,good)[1]==[3,4,5,6,7,8,9],'positive delta')
    nonzero=apply_delta(prior,good)
    require(apply_delta(nonzero,{'length':3,'changes':[[1,[0]*7]]})==prior,'explicit zero change')
    invalid=[{'length':4,'changes':[]},{'length':True,'changes':[]},{'length':3,'changes':[[1,[0]*7]]},
        {'length':3,'changes':[[3,[1]*7]]},{'length':3,'changes':[[-1,[1]*7]]},
        {'length':3,'changes':[[1,[1]*7],[1,[2]*7]]},{'length':3,'changes':[[2,[1]*7],[1,[2]*7]]},
        {'length':3,'changes':[[1,[1]*6]]},{'length':3,'changes':[[1,[True]*7]]},
        {'length':3,'changes':[[1,[-1]*7]]},{'length':3,'changes':[[1,[1<<32]*7]]},
        {'length':3,'changes':[],'extra':0}]
    for delta in invalid:
        try:apply_delta(prior,delta)
        except ValueError:pass
        else:raise ValueError('malformed delta accepted')
    state=Replay(next(iter(recipes().values())))
    for index in [-1,4099]:
        try:state.cell(index)
        except ValueError:pass
        else:raise ValueError('path index accepted')
    request=recipes()['path_wide_later_4096']
    rows=list(observations(request))
    def encode(records):return b''.join((json.dumps(row,sort_keys=True,separators=(',',':'))+'\n').encode()for row in records)
    encoded=encode(rows);validate_output(encoded,request)
    forged=[]
    index=next(i for i,row in enumerate(rows)if row.get('stage')=='seed')
    for kind in ['length','duplicate','unchanged','value','phase','pass','unknown']:
        changed=deepcopy(rows)
        row=changed[index]
        if kind=='length':row['cells']['length']+=1
        elif kind=='duplicate':row['cells']['changes'].append(row['cells']['changes'][0])
        elif kind=='unchanged':row['cells']['changes'].append([4098,[0]*7])
        elif kind=='value':row['cells']['changes'][0][1][0]+=1
        elif kind=='phase':row['phase']='reverse'
        elif kind=='pass':row['padding_pass']=True
        else:row['unexpected']=0
        forged.append(encode(changed))
    forged.extend([encoded[:-1],encoded+b'{}\n'])
    for data in forged:
        try:validate_output(data,request)
        except ValueError:pass
        else:raise ValueError('forged wider trace accepted')
    return dict(status='passed',forged_traces_rejected=len(forged),trace_bytes=len(encoded),small_cases=17,complete_states_compared=compared,wider_cases=report,
        malformed_deltas_rejected=len(invalid),path_index_rejections=2,child_processes=0)


if __name__=='__main__':print(json.dumps(check(),sort_keys=True))
