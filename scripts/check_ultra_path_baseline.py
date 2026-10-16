#!/usr/bin/env python3
"""Independent coupled path boundaries and complete original C-fixture reconciliation."""
from pathlib import Path
import copy,hashlib,json,struct,sys
sys.dont_write_bytecode=True
from oracles.ultra_paths import Request,parse_packet,MAX_PACKET
from oracles.ultra_path_validation import Replay,observations,validate_output
from oracles.optimal_path_recipes import recipes as small
from oracles.optimal_path_wide_recipes import recipes as wide
from oracles.optimal_paths_wide import Request as Original

def originals():
    result=[]
    for kind,cases in [(0,small()),(1,wide())]:
        for name,old in cases.items():
            result.append((name,Original(old.source,old.scripts,old.min_match,old.target,old.literal_mode),kind))
    return result
from oracles.ultra_scalar import need,sha
ROOT=Path(__file__).resolve().parents[1]
NONE=0xffffffff
STAGES=['begin','query','origin','seed','literal','relax','select','reverse_start','reverse','sequence','refresh','complete']
KEYS=['lit','ll','ml','of']

def reconstruct_original():
    pack=bytearray(b'ZOPFX01\0'+struct.pack('<I',27));rows=[]
    def words(*values):pack.extend(struct.pack('<'+'I'*len(values),*values))
    def optional(value):
     words(int(value is not None))
     if value is not None:words(*value)
    for name,old,kind in originals():
     request=old;replay=Replay(Request(0,request));replay.run();events=replay.events;encoded=name.encode();count=len(events);path_cells=len(replay.cells)
     words(len(encoded),len(request.source),request.min_match,request.target,request.literal_mode,path_cells,count,len(request.scripts),kind);pack.extend(encoded);pack.extend(request.source)
     for position,candidates in request.scripts:
      words(position,len(candidates))
      for pair in candidates:words(*pair)
     frequencies=[0]*386;sequences=[]
     for event in events:
      words(STAGES.index(event['stage']),int(event['phase']=='reverse'),event['ip'],event['anchor'],*(NONE if event[k]is None else event[k]for k in ('cur','last','tail')),*event['repeats']);optional(event['stretch']);optional(event['reverse']);optional(event['query'])
      if event['query']is not None:
       words(len(event['matches']))
       for pair in event['matches']:words(*pair)
      changes=event['cells']['changes'];words(len(changes))
      for index,cell in changes:words(index,*cell)
      current=[v for key in KEYS for v in event['frequencies'][key]]+[event['sums'][k]for k in KEYS]+[event['base_prices'][k]for k in KEYS]+[int(event['predefined'])];changed=[(i,v)for i,v in enumerate(current)if v!=frequencies[i]];words(len(changed))
      for pair in changed:words(*pair)
      frequencies=current;output=event['sequences'];appended=output[len(sequences):];words(len(output),len(appended))
      for row in appended:words(*row)
      sequences=output
     rows.append(dict(name=name,events=count))

    expected=(ROOT/'tests/fixtures/reference-optimal-paths/cases.bin').read_bytes()
    need(sha(expected)=='0defe329f5a50e2add4162fe865c7f1fc314db2c15733485d2cc92f463c7d167','original C fixture pin')
    need(bytes(pack)==expected,'complete original native pack reconstruction')
    return dict(bytes=len(pack),sha256=sha(pack),events=sum(r['events']for r in rows),cases=len(rows))

if __name__=='__main__':
    print(json.dumps(dict(status='passed',optimized=not __debug__,baseline=reconstruct_original(),qualification='Complete original27-case native path fixture reconstructed by the policy-0 successor. No native Ultra path execution.'),sort_keys=True,indent=2))
