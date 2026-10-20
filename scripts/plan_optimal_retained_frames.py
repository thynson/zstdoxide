#!/usr/bin/env python3
"""Freeze independent retained source/state hypotheses before C execution."""
import argparse
import hashlib
import json
from pathlib import Path
from oracles.optimal_retained_frames import REVISION,require
from oracles.optimal_retained_frames_recipes import recipes
from oracles.optimal_retained_frames_validation import records
ROOT=Path(__file__).resolve().parents[1]
DEST=ROOT/'tests/fixtures/reference-optimal-retained-frames-recipes/recipes.json'


def manifest():
    cases=[]
    for name,recipe in recipes().items():
        rows,coverage=records(recipe.request,recipe.kinds)
        data=json.dumps(rows,sort_keys=True,separators=(',',':')).encode()
        events=[r for r in rows if r['record']=='event']
        cases.append(dict(highest_path=max((at for e in events for at,_ in e['cells']['changes']),default=0),
            max_last=max((e['last']or 0 for e in events),default=0),name=name,packet_sha256=hashlib.sha256(recipe.request.packet()).hexdigest(),
            predicted_state_sha256=hashlib.sha256(data).hexdigest(),blocks=list(recipe.request.blocks),
            hypothesized_kinds=list(recipe.kinds),events=sum(r['record']=='event'for r in rows),
            queries=sum(r.get('stage')=='query'for r in rows),
            sequences=sum(r['sequences']for r in rows if r['record']=='block_publish'),coverage=sorted(coverage)))
    require(len(cases)==19 and 'later_horizon' in next(c for c in cases if c['name']=='wide_horizon')['coverage'], 'actual wider horizon prediction')
    require(all(c['blocks']==list(recipes()[c['name']].request.blocks) for c in cases),'source-only checked schedules')
    require(max(c['highest_path'] for c in cases)==4096,'reconstruction reaches boundary cell')
    return dict(schema_version=1,revision=REVISION,source_cap=32768,cases=cases,
        qualification='Independent retained predictions including seven unchanged predecessor inputs and twelve larger source hypotheses; no new C execution or larger-domain production qualification.')


def main(write):
    data=(json.dumps(manifest(),sort_keys=True,indent=2)+'\n').encode()
    if write:
        require(not DEST.exists(),'immutable retained manifest destination');DEST.parent.mkdir(parents=True,exist_ok=True);DEST.write_bytes(data)
    require(DEST.read_bytes()==data,'exact retained source-only manifest')
    print(json.dumps(dict(status='passed',cases=19,manifest_sha256=hashlib.sha256(data).hexdigest()),sort_keys=True))

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--write',action='store_true');main(p.parse_args().write)
