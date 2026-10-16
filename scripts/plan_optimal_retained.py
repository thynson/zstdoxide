#!/usr/bin/env python3
"""Freeze independent retained source/state hypotheses before C execution."""
import argparse
import hashlib
import json
from pathlib import Path
from oracles.optimal_retained import REVISION,require
from oracles.optimal_retained_recipes import recipes
from oracles.optimal_retained_validation import records
ROOT=Path(__file__).resolve().parents[1]
DEST=ROOT/'tests/fixtures/reference-optimal-retained-recipes/recipes.json'


def manifest():
    cases=[]
    for name,recipe in recipes().items():
        rows,coverage=records(recipe.request,recipe.kinds)
        data=json.dumps(rows,sort_keys=True,separators=(',',':')).encode()
        cases.append(dict(name=name,packet_sha256=hashlib.sha256(recipe.request.packet()).hexdigest(),
            predicted_state_sha256=hashlib.sha256(data).hexdigest(),blocks=list(recipe.request.blocks),
            hypothesized_kinds=list(recipe.kinds),events=sum(r['record']=='event'for r in rows),
            queries=sum(r.get('stage')=='query'for r in rows),
            sequences=sum(r['sequences']for r in rows if r['record']=='block_publish'),coverage=sorted(coverage)))
    return dict(schema_version=1,revision=REVISION,cases=cases,
        qualification='Independent retained predictions with explicit publication hypotheses; no C execution or retained production qualification.')


def main(write):
    data=(json.dumps(manifest(),sort_keys=True,indent=2)+'\n').encode()
    if write:
        require(not DEST.exists(),'immutable retained manifest destination');DEST.parent.mkdir(parents=True,exist_ok=True);DEST.write_bytes(data)
    require(DEST.read_bytes()==data,'exact retained source-only manifest')
    print(json.dumps(dict(status='passed',cases=12,manifest_sha256=hashlib.sha256(data).hexdigest()),sort_keys=True))

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--write',action='store_true');main(p.parse_args().write)
