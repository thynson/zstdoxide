#!/usr/bin/env python3
"""Freeze exact source-only ordinary requests and qualify Ultra-specific branches."""
from pathlib import Path
import copy,json,sys
sys.dont_write_bytecode=True
from oracles.ultra_fresh import parse_packet
from oracles.ultra_fresh_recipes import recipes
from oracles.ultra_fresh_validation import records,observations,validate_output
from oracles.optimal_fresh_wide_recipes import recipes as originals
from oracles.ultra_scalar import need,sha

REQUIRED={'ultra_alternative_considered','ultra_alternative_extends_frontier','ultra_alternative_rejected','ultra_alternative_selected','ultra_improvement_after_rejection','ultra_match_tie_continue','ultra_match_worse_continue','ultra_unpromising_query'}
def enc(rows):return b''.join((json.dumps(row,sort_keys=True,separators=(',',':'))+'\n').encode()for row in rows)
def check():
    children=[]
    def guard(event,args):
        if event in('subprocess.Popen','os.system','os.posix_spawn','os.fork'):
            children.append(event);raise ValueError('child execution forbidden')
    sys.addaudithook(guard)
    cases=recipes();old=originals();manifest=[];branches=[];coverage=set();preserved=0
    for name,recipe in cases.items():
        request=recipe.request;packet=request.packet();need(parse_packet(packet)==request,'exact packet round trip')
        r=request.original
        row=dict(name=name,policy=request.policy,packet_bytes=len(packet),packet_sha256=sha(packet),source_bytes=len(r.source),source_sha256=sha(r.source),parameters=[r.window_log,r.hash_log,r.chain_log,r.search_log,r.min_match,r.target,r.literal_mode],blocks=list(r.blocks),kinds=list(recipe.kinds));manifest.append(row)
        prefix=f'uf_p{request.policy}_'
        if name[len(prefix):]in old:
            need(packet[12:]==old[name[len(prefix):]].request.packet(),'unchanged original request bytes');preserved+=1
        else:
            rows,seen=records(request,recipe.kinds);events=[e for e in rows if e['record']=='event'];coverage.update(seen)
            branches.append(dict(name=name,events=len(events),alternatives=sum(e['alternative']is not None for e in events),sequences=rows[-1]['sequences'],rows_sha256=sha(enc(rows)),coverage=sorted(seen)))
    need(preserved==40 and REQUIRED<=coverage,'all original requests and Ultra branches')
    recipe=cases['uf_p2_alternative_seed_0'];good=list(observations(recipe.request,recipe.kinds));validate_output(enc(good),recipe.request,recipe.kinds)
    rejected=[]
    for index in range(12):
        rows=copy.deepcopy(good);event=next(e for e in rows if e.get('alternative')is not None);event['alternative'][index]+=1
        try:validate_output(enc(rows),recipe.request,recipe.kinds)
        except ValueError:rejected.append('alternative-'+str(index))
        else:raise ValueError('accepted changed signed alternative')
    for kind in ['literal-frontier','literal-cell','nonquery-search','policy-bool','alternative-stage']:
        rows=copy.deepcopy(good);at=next(i for i,e in enumerate(rows)if e.get('alternative')is not None);event=rows[at];following=rows[at+1]
        if kind=='literal-frontier':following['last']+=1
        elif kind=='literal-cell':following['cells']['changes'].append([4098,[1,0,0,0,0,0,0]])
        elif kind=='nonquery-search':event['search']['changes'].append([0,123])
        elif kind=='policy-bool':event['price_policy']=True
        else:event['stage']='literal'
        try:validate_output(enc(rows),recipe.request,recipe.kinds)
        except ValueError:rejected.append(kind)
        else:raise ValueError('accepted alternative publication forgery')
    need(not children and len(rejected)==17,'all controls without children')
    return dict(status='passed',optimized=not __debug__,recipes=manifest,branch_cases=branches,coverage=sorted(coverage),rejected=rejected,child_processes=0,qualification='Forty-four source-only ordinary requests preserve all twenty original requests under both policies. Two added sources close all eight Ultra-specific model branches. No new native execution, entropy-kind prediction or public admission.')
if __name__=='__main__':print(json.dumps(check(),sort_keys=True,indent=2))
