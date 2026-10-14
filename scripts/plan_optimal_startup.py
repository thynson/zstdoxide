#!/usr/bin/env python3
"""Freeze source-only fresh startup hypotheses; no C process or capture planning."""
import argparse
import hashlib
import json
from pathlib import Path
from oracles.optimal_startup import REVISION,require
from oracles.optimal_startup_recipes import recipes
from oracles.optimal_startup_validation import Replay

ROOT=Path(__file__).resolve().parents[1]
DEST=ROOT/'tests/fixtures/reference-optimal-startup-recipes/recipes.json'
def manifest():
    rows=[]
    for name,request in recipes().items():
        replay=Replay(request);events=replay.run()
        canonical=json.dumps(events,sort_keys=True,separators=(',',':')).encode()
        rows.append(dict(name=name,packet_sha256=hashlib.sha256(request.packet()).hexdigest(),
            source_sha256=hashlib.sha256(request.source).hexdigest(),events_sha256=hashlib.sha256(canonical).hexdigest(),
            events=len(events),queries=sum(e['stage']=='query'for e in events),sequences=len(replay.sequences),
            coverage=sorted(replay.coverage),search_phases=sorted(replay.search_phases)))
    return dict(schema_version=1,revision=REVISION,cases=rows,
        qualification='Frozen predictive fresh search/path composition; no C execution or public admission.')

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--write',action='store_true');args=parser.parse_args()
    data=(json.dumps(manifest(),sort_keys=True,indent=2)+'\n').encode()
    if args.write:
        require(not DEST.exists(),'never overwrite frozen recipe manifest')
        DEST.parent.mkdir(parents=True,exist_ok=True);DEST.write_bytes(data)
    require(DEST.read_bytes()==data,'exact immutable source-only manifest')
    print(json.dumps(dict(status='passed',cases=12,manifest_sha256=hashlib.sha256(data).hexdigest()),sort_keys=True))
if __name__=='__main__':main()
