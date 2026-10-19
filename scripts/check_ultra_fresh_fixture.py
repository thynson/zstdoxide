#!/usr/bin/env python3
"""Reconstruct ordinary Ultra state bytes from source, retaining native frame inputs."""
import argparse
import json
from pathlib import Path
import struct
import sys
sys.dont_write_bytecode = True
from oracles.ultra_fresh_recipes import recipes
from oracles.ultra_fresh_validation import records
from oracles.ultra_scalar import need, sha, load
from check_ultra_fresh_capture_v2 import read, validate_frame, validate_ledger_pair

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / 'tests/fixtures/reference-ultra-fresh'
STAGES = ('begin','query','origin','seed','literal','relax','select','reverse_start','reverse','sequence','refresh','complete','alternative')
KEYS = ('lit','ll','ml','of')


def reconstruct(capture):
    provenance = load((DEST/'provenance.json').read_bytes())
    need(sha(read(capture,'evidence.json',8<<20))==provenance['capture_evidence_sha256'], 'original audited capture identity')
    evidence = load(read(capture,'evidence.json',8<<20))
    verdict=load(read(capture,'independent-audit-v2.json',1<<20))
    need(verdict['status']=='passed' and verdict['audit']['observed_events_per_build']==396952
         and verdict['capture_evidence_sha256']==provenance['capture_evidence_sha256']
         and verdict['auditor_sha256']==sha((ROOT/'scripts/check_ultra_fresh_capture_v2.py').read_bytes()), 'audited native envelope')
    output = bytearray(b'ZUFFX01\0'+struct.pack('<I',44)); layouts=[]
    def words(*values):
        need(all(type(v)is int and 0<=v<=0xffffffff for v in values), 'independent u32 domain')
        output.extend(struct.pack('<'+'I'*len(values),*values))
    def optional(values):
        words(int(values is not None))
        if values is not None:words(*values)
    def native(path):
        data=read(capture,path,32768)
        need(evidence['artifacts'][path]==dict(bytes=len(data),sha256=sha(data)), 'native opaque artifact identity')
        return data
    for name,recipe in recipes().items():
        request=recipe.request;r=request.original;encoded=name.encode();start=len(output)
        frame=native(f'artifacts/normal/{name}.0.zst');validate_frame(frame,r,recipe.kinds)
        ledgers=[load(native(f'artifacts/{mode}/{name}.0.json')) for mode in ['normal','sanitized']]
        validate_ledger_pair(*ledgers)
        words(len(encoded),len(r.source),r.window_log,r.hash_log,r.chain_log,r.search_log,r.min_match,r.target,r.literal_mode,1,len(frame),4099,request.policy)
        for ledger in ledgers:words(*ledger['allocations'],ledger['peak_bytes'])
        output.extend(encoded);output.extend(r.source);output.extend(frame)
        rows,_=records(request,recipe.kinds);begin,pub=rows[0],rows[-1];events=rows[1:-1];block_at=len(output)
        words(0,len(r.source),pub['kind'],pub['previous_owner_before'],pub['previous_owner_after'],begin['frontier'],len(events),*pub['before_repeats'],*pub['candidate_repeats'],*(v for owner in pub['owners'] for v in owner),pub['tail'])
        counts=[0]*386;sequence_count=0;event_layouts=[]
        for event in events:
            at=len(output);words(STAGES.index(event['stage']),int(event['phase']=='reverse'),event['ip'],event['anchor'],*[0xffffffff if event[k]is None else event[k] for k in ['cur','last','tail']],*event['repeats'])
            optional(event['stretch']);optional(event['reverse']);optional(event['query'])
            if event['query']is not None:
                words(len(event['matches']))
                for pair in event['matches']:words(*pair)
            words(int(event['alternative']is not None));alternative=None
            if event['alternative']is not None:
                alternative=len(output);output.extend(struct.pack('<12q',*event['alternative']))
            path_at=len(output);words(len(event['cells']['changes']))
            for index,cell in event['cells']['changes']:words(index,*cell)
            current=[v for k in KEYS for v in event['frequencies'][k]]+[event['sums'][k] for k in KEYS]+[event['base_prices'][k] for k in KEYS]+[int(event['predefined'])]
            changed=[(i,v)for i,v in enumerate(current)if counts[i]!=v];frequency_at=len(output);words(len(changed))
            for pair in changed:words(*pair)
            counts=current;appended=event['sequences'][sequence_count:];words(len(event['sequences']),len(appended))
            for sequence in appended:words(*sequence)
            sequence_count=len(event['sequences']);search_at=len(output);words(len(event['search']['changes']))
            for pair in event['search']['changes']:words(*pair)
            event_layouts.append(dict(start=at,stage=event['stage'],alternative=alternative,path=path_at,frequency=frequency_at,search=search_at))
        layouts.append(dict(name=name,start=start,block=block_at,events=event_layouts))
    need(len(layouts)==44 and sum(len(c['events'])for c in layouts)==198476, 'complete independent case/event inventory')
    return bytes(output),layouts


def check(data,expected,digest=True):
    provenance=load((DEST/'provenance.json').read_bytes())
    need(type(data)is bytes and len(data)==provenance['pack_bytes'], 'exact fixture extent')
    if digest:need(sha(data)==provenance['pack_sha256'], 'native fixture digest')
    need(data==expected, 'complete independent ordinary reconstruction')


def controls(data,expected,layouts,out):
    need(not out.exists(), 'fresh reconstruction controls');out.mkdir();rows=[]
    def reject(name,at,removed,replacement):
        changed=data[:at]+replacement+data[at+removed:]
        try:check(changed,expected,False)
        except ValueError as error:diagnostic=str(error)
        else:raise ValueError('accepted reconstruction forgery '+name)
        row=dict(name=name,offset=at,removed=removed,replacement_hex=replacement.hex(),bytes=len(changed),sha256=sha(changed),diagnostic=diagnostic)
        path=out/(name+'.json');path.write_text(json.dumps(row,sort_keys=True)+'\n')
        saved=load(path.read_bytes());recovered=data[:saved['offset']]+bytes.fromhex(saved['replacement_hex'])+data[saved['offset']+saved['removed']:]
        need(recovered==changed and sha(recovered)==saved['sha256'], 'lossless forgery delta');rows.append(row)
    def word(name,at,value):reject(name,at,4,struct.pack('<I',value))
    reject('truncated',len(data)-1,1,b'');reject('trailing',len(data),0,b'!');reject('magic',0,1,b'!')
    word('empty-cases',8,0);word('case-cap',8,45);word('source-cap',16,16385);word('unknown-policy',60,1);word('wrong-valid-policy',60,2)
    case=next(c for c in layouts if c['name']=='uf_p2_alternative_seed_0')
    word('publication-kind',case['block']+8,0);word('publication-owner',case['block']+16,0)
    index=next(i for i,e in enumerate(case['events'])if e['alternative']is not None);event=case['events'][index]
    word('alternative-stage',event['start'],4);word('alternative-presence',event['alternative']-4,0)
    for column in range(12):
        at=event['alternative']+8*column;value=struct.unpack_from('<q',data,at)[0];reject('alternative-'+str(column),at,8,struct.pack('<q',value+1))
    word('literal-frontier',case['events'][index+1]['start']+20,0)
    word('path-domain',event['path']+4,4099)
    word('nonquery-search',event['search'],1)
    query=next(e for e in case['events']if e['stage']=='query')
    word('search-index',query['search']+4,73730)
    check(data,expected)
    report=dict(status='passed',optimized=not __debug__,rejected=len(rows),root_digest_bypassed=True,child_processes=0,records=rows,
        qualification='Every parser/search/frequency/publication state byte is independently source-driven. Frame payloads and allocation sizes are retained audited native inputs, not entropy or allocator predictions.')
    (out/'summary.json').write_text(json.dumps(report,sort_keys=True,indent=2)+'\n');return report


def main():
    children=[]
    def guard(event,args):
        if event in ('subprocess.Popen','os.system','os.posix_spawn','os.fork'):
            children.append(event);raise ValueError('child execution forbidden')
    sys.addaudithook(guard)
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('capture',type=Path);p.add_argument('--controls',type=Path,required=True);a=p.parse_args()
    expected,layouts=reconstruct(a.capture.resolve(strict=True));data=(DEST/'cases.bin').read_bytes();check(data,expected);result=controls(data,expected,layouts,a.controls)
    need(not children,'no child execution');print(json.dumps(dict(status='passed',bytes=len(data),sha256=sha(data),rejected=result['rejected'])))
if __name__=='__main__':main()
