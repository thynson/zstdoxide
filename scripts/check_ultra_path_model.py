#!/usr/bin/env python3
"""Independent coupled path boundaries and complete original C-fixture reconciliation."""
from pathlib import Path
import copy,hashlib,json,struct,sys
sys.dont_write_bytecode=True
from oracles.ultra_paths import Request,parse_packet,MAX_PACKET
from oracles.ultra_path_validation import Replay,observations,validate_output
from oracles.ultra_path_recipes import originals,recipes,manifest
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

def encoded(records):
    return b''.join((json.dumps(r,sort_keys=True,separators=(',',':'))+'\n').encode()for r in records)

def check():
    baseline=reconstruct_original();coverage=set();rows=[];all_cases=recipes()
    need(len(all_cases)==58,'exact source-only corpus')
    for name,request in all_cases.items():
        need(parse_packet(request.packet())==request,'exact request roundtrip')
        replay=Replay(request);replay.run()
        if request.policy==2:coverage.update(replay.coverage)
        rows.append(dict(name=name,policy=request.policy,events=len(replay.events),queries=sum(e['stage']=='query'for e in replay.events),sequences=len(replay.sequences),alternatives=len(replay.witnesses),coverage=sorted(replay.coverage)))
        if name.endswith('no_query_minimum_source'):need(len(replay.events)==2 and replay.sequences==[]and replay.repeats==(1,4,8),'hand no-query state')
        if name.endswith('_immediate')and 'maximum'not in name:need(replay.sequences==[[0,1,3,4]]and replay.repeats==(1,1,4)and replay.events[-1]['tail']==12,'hand immediate sequence')
    required={'ultra_alternative_considered','ultra_alternative_selected','ultra_alternative_rejected','ultra_alternative_extends_frontier','ultra_improvement_after_rejection','ultra_match_tie_continue','ultra_match_worse_continue','ultra_unpromising_query','later_horizon','literal_tie','multiple_sequences','trailing_backtrack'}
    need(required<=coverage,'complete coupled branch witnesses')
    request=all_cases['up_p2_ultra_alternative_seed_0'];records=list(observations(request));validate_output(encoded(records),request)
    index=next(i for i,r in enumerate(records)if r.get('stage')=='alternative')
    # Independent arithmetic witness: the saved price plus one literal and a
    # negative LL increment is 4515 + 256 - 224 = 4547, which loses to 2168.
    need(records[index]['alternative']==[4515,5,3,0,0,0,0,-224,4547,2168,4515,0],'hand saved-match candidate witness')
    rejected=[]
    def forged(name,edit):
        changed=copy.deepcopy(records);edit(changed)
        try:validate_output(encoded(changed),request)
        except ValueError:rejected.append(name)
        else:raise ValueError('forged trace accepted '+name)
    forged('metadata-policy',lambda r:r[0].update(price_policy=0))
    forged('event-policy',lambda r:r[index].update(price_policy=0))
    forged('missing-alternative',lambda r:r[index].update(alternative=None))
    for column in range(12):forged('alternative-column-'+str(column),lambda r,c=column:r[index]['alternative'].__setitem__(c,r[index]['alternative'][c]+1))
    forged('negative-as-bool',lambda r:r[index]['alternative'].__setitem__(7,False))
    forged('next-frontier',lambda r:r[index+1].update(last=0))
    forged('repeats',lambda r:r[index+1].update(repeats=[1,2,3]))
    forged('missing-record',lambda r:r.pop(index))
    forged('extra-record',lambda r:r.append(r[-1]))
    packet=request.packet();malformed=[packet[:31],packet[:-1],packet+b'!',b'!'+packet[1:]]
    for offset,value in [(8,8),(8,8193),(12,65),(16,2),(16,8),(20,4096),(24,2),(28,1),(28,3)]:malformed.append(packet[:offset]+struct.pack('<I',value)+packet[offset+4:])
    for data in malformed:
        try:parse_packet(data)
        except ValueError:pass
        else:raise ValueError('malformed transport accepted')
    try:Request(True,request.original).validate()
    except ValueError:pass
    else:raise ValueError('boolean policy accepted')
    need(sum(r['events']for r in rows)==65672 and sum(r['alternatives']for r in rows)==72,'complete frozen observation totals')
    return dict(status='passed',optimized=not __debug__,baseline=baseline,cases=rows,coverage=sorted(coverage),forged_trace_rejections=len(rejected),transport_rejections=len(malformed)+1,qualification='Source-only independent model and original C-fixture reconciliation. No successor native path execution or production Ultra path claim.')

if __name__=='__main__':print(json.dumps(check(),sort_keys=True,indent=2))
