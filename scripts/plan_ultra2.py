#!/usr/bin/env python3
"""Freeze verified source-only recipe predictions before native execution."""
import argparse
import hashlib
import json
from pathlib import Path

from oracles.ultra2_recipes import recipes
from oracles.ultra2_protocol import Request, REVISION
from oracles.ultra2_validation import records
from oracles.ultra_scalar import need, load

ROOT=Path(__file__).resolve().parents[1]


def sha(data):return hashlib.sha256(data).hexdigest()


def ring_coverage():
    result=[]
    for name,recipe in recipes().items():
        if not name.startswith('u2_ring_'):continue
        r=recipe.request;rows,_=records(r,recipe.seed)
        h=1<<r.hash_log;c=1<<r.chain_log;state=[0]*(h+c+(1<<r.hash3_log)+2)
        aliases=short=0;origin=len(r.source)+2
        for row in rows:
            if row['record']!='event':continue
            if row['pass_name']=='encoding'and row['stage']=='query':
                slot=(origin+row['query'][0])&((1<<(r.chain_log-1))-1)
                aliases+=any(0<value<origin for value in state[h+2*slot:h+2*slot+2])
                short+=any(0<value<origin for value in state[h+c:-2])
            for index,value in row['search']['changes']:state[index]=value
        need(aliases>0 and short>0,'actual stale tree-slot and short-table coverage')
        result.append(dict(name=name,stale_ring_query_slots=aliases,queries_with_stale_short_cells=short))
    need(len(result)==6,'all chosen ring residues')
    return result


def manifest(qualified,survey,wire):
    e=load((qualified/'evidence.json').read_bytes());normal=load((survey/'summary.json').read_bytes());wire_e=load((wire/'evidence.json').read_bytes())
    need(wire_e['status']=='passed'and wire_e['source_unchanged']is True,'complete bounded wire predictions')
    wire_rows={r['name']:r for r in wire_e['cases']}
    for name,digest in wire_e['source'].items():need(sha((ROOT/name).read_bytes())==digest,'unchanged wire model source')
    need(e['status']==normal['status']=='passed'and e['source_unchanged']is True,'complete source-only campaigns')
    for name,digest in e['source'].items():need(sha((ROOT/name).read_bytes())==digest,'unchanged executed model source')
    cases=recipes();primary={r['name']:r for r in normal['cases']};checked={r['name']:r for r in e['cases']}
    need(set(cases)==set(primary)==set(checked)==set(wire_rows)and len(cases)==53,'complete two-mode recipe inventory')
    result=[]
    for name,recipe in cases.items():
        a,b=primary[name],checked[name]
        for field in ('name','seed','source_sha256','rows','events','rows_sha256','passes','changed_path','sequence_counts','coverage'):
            need(type(a[field])is type(b[field])and a[field]==b[field],'normal/optimized prediction equality: '+field)
        need(b['status']=='passed'and b['optimized']is True and b['child_processes']==0,'optimized qualified worker')
        need(b['packet_sha256']==sha(Request(recipe.request,recipe.seed).packet()),'exact new packet')
        wire_row=wire_rows[name]
        need(wire_row['events']==2*b['events']and wire_row['child_processes']==0 and wire_row['optimized']is False,'complete normal wire events')
        result.append(dict({k:v for k,v in b.items()if k not in ('status','optimized','child_processes')},wire={k:v for k,v in wire_row.items()if k not in ('name','optimized','child_processes')}))
    changed=[r['name']for r in result if r['changed_path']]
    need(changed and sum(r['predecessor']is not None for r in result)==44,'preserved inputs and changed seeded paths')
    return dict(schema_version=1,revision=REVISION,model_source=wire_e['source'],cases=result,
                logical_events=sum(r['events']for r in result),changed_paths=changed,
                ring_coverage=ring_coverage(),qualification='Source-only predictions frozen before Ultra2 native execution. '
                'Optimized workers retain source snapshots and before/after equality; the independent sequential normal '
                'survey has identical complete row hashes. Every predecessor first-pass state matches the unchanged '
                'ordinary native transcript. No second-pass native observation or entropy/frame prediction is claimed.')


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('qualified',type=Path);p.add_argument('survey',type=Path)
    p.add_argument('wire',type=Path)
    a=p.parse_args();print(json.dumps(manifest(a.qualified,a.survey,a.wire),sort_keys=True,indent=2))
