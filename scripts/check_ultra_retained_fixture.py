#!/usr/bin/env python3
"""Reconstruct retained policy fixture states from source without native providers."""
import argparse
import json
from pathlib import Path
import struct
import sys
sys.dont_write_bytecode = True
from oracles.ultra_retained_recipes import recipes
from oracles.ultra_retained_validation import records
from oracles.ultra_scalar import need, sha, load
from check_ultra_retained_capture import read, validate_frame, validate_ledger_pair

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / 'tests/fixtures/reference-ultra-retained'
STAGES = ('begin','query','origin','seed','literal','relax','select','reverse_start','reverse','sequence','refresh','complete','alternative')
KEYS = ('lit','ll','ml','of')


def reconstruct(capture):
    provenance = load((DEST/'provenance.json').read_bytes())
    identity = sha(read(capture,'evidence.json',8<<20))
    need(identity == provenance['capture_evidence_sha256'], 'original audited capture identity')
    evidence = load(read(capture,'evidence.json',8<<20))
    need(evidence['status']=='passed' and evidence['source_unchanged'] is True
         and evidence['audit']['observed_events_per_build']==415270
         and evidence['audit']['recipes']==42, 'complete audited native envelope')
    for name,digest in evidence['source'].items():
        need(sha((ROOT/name).read_bytes())==digest==sha(read(capture,'source/'+name,8<<20)), 'unchanged qualified source')
    output = bytearray(b'ZURFX01\0'+struct.pack('<I',42)); layouts=[]
    def words(*values):
        need(all(type(v)is int and 0<=v<=0xffffffff for v in values), 'independent u32 domain')
        output.extend(struct.pack('<'+'I'*len(values),*values))
    def optional(values):
        words(int(values is not None))
        if values is not None:words(*values)
    def native(path):
        data=read(capture,path,32832)
        need(evidence['artifacts'][path]==dict(bytes=len(data),sha256=sha(data)), 'native opaque artifact identity')
        return data
    for name,recipe in recipes().items():
        request=recipe.request;r=request.original;encoded=name.encode();start=len(output)
        frame=native(f'artifacts/normal/{name}.0.zst');validate_frame(frame,r,recipe.kinds)
        ledgers=[load(native(f'artifacts/{mode}/{name}.0.json')) for mode in ['normal','sanitized']]
        validate_ledger_pair(*ledgers)
        for mode in ['normal','sanitized']:
            for padding in [0,1]:need(native(f'artifacts/{mode}/{name}.{padding}.zst')==frame,'all captured frames agree')
        words(len(encoded),len(r.source),r.window_log,r.hash_log,r.chain_log,r.search_log,r.min_match,r.target,r.literal_mode,len(r.blocks),len(frame),4099,request.policy)
        for ledger in ledgers:words(*ledger['allocations'],ledger['peak_bytes'])
        output.extend(encoded);output.extend(r.source);output.extend(frame)
        rows,_=records(request,recipe.kinds);counts=[0]*386;blocks=[]
        for block,length in enumerate(r.blocks):
            selected=[row for row in rows if row['block']==block]
            begin,pub=selected[0],selected[-1];events=selected[1:-1];block_at=len(output)
            need(begin['record']=='block_begin' and pub['record']=='block_publish','complete block ownership')
            words(begin['start'],length,pub['kind'],pub['previous_owner_before'],pub['previous_owner_after'],begin['frontier'],len(events),*pub['before_repeats'],*pub['candidate_repeats'],*(v for owner in pub['owners'] for v in owner),pub['tail'])
            sequence_count=0;event_layouts=[]
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
            blocks.append(dict(start=block_at,events=event_layouts))
        layouts.append(dict(name=name,start=start,blocks=blocks))
    need(len(layouts)==42 and sum(len(b['events'])for c in layouts for b in c['blocks'])==207635, 'complete independent case/event inventory')
    return bytes(output),layouts


def check(data,expected,digest=True):
    provenance=load((DEST/'provenance.json').read_bytes())
    need(type(data)is bytes and len(data)==provenance['pack_bytes'], 'exact fixture extent')
    if digest:need(sha(data)==provenance['pack_sha256'], 'native fixture digest')
    need(data==expected, 'complete independent retained reconstruction')


def controls(data,expected,layouts,out):
    need(not out.exists(), 'fresh reconstruction controls');out.mkdir();rows=[]
    def reject(name,at,removed,replacement):
        changed=data[:at]+replacement+data[at+removed:]
        need(changed!=data,'effective forgery '+name)
        try:check(changed,expected,False)
        except ValueError as error:diagnostic=str(error)
        else:raise ValueError('accepted reconstruction forgery '+name)
        row=dict(name=name,offset=at,removed=removed,replacement_hex=replacement.hex(),bytes=len(changed),sha256=sha(changed),diagnostic=diagnostic)
        path=out/(name+'.json');path.write_text(json.dumps(row,sort_keys=True)+'\n')
        saved=load(path.read_bytes());recovered=data[:saved['offset']]+bytes.fromhex(saved['replacement_hex'])+data[saved['offset']+saved['removed']:]
        need(recovered==changed and sha(recovered)==saved['sha256'], 'lossless forgery delta');rows.append(row)
    def word(name,at,value):reject(name,at,4,struct.pack('<I',value))
    def increment(name,at):word(name,at,struct.unpack_from('<I',data,at)[0]+1)
    reject('truncated',len(data)-1,1,b'');reject('trailing',len(data),0,b'!');reject('magic',0,1,b'!')
    word('empty-cases',8,0);word('case-cap',8,43);word('source-cap',16,32769);word('unknown-policy',60,1);word('wrong-valid-policy',60,2)
    case=next(c for c in layouts if c['name']=='ur_p2_alternatives_0202')
    block=next(b for b in case['blocks'][1:] if any(e['alternative']is not None for e in b['events']))
    increment('later-block-start',block['start']);word('later-publication-kind',block['start']+8,0)
    increment('later-publication-owner',block['start']+16)
    index=next(i for i,e in enumerate(block['events'])if e['alternative']is not None);event=block['events'][index]
    word('later-alternative-stage',event['start'],4);word('later-alternative-presence',event['alternative']-4,0)
    for column in range(12):
        at=event['alternative']+8*column;value=struct.unpack_from('<q',data,at)[0];reject('later-alternative-'+str(column),at,8,struct.pack('<q',value+1))
    increment('later-literal-frontier',block['events'][index+1]['start']+20)
    path_event=next(e for e in block['events']if struct.unpack_from('<I',data,e['path'])[0]>0)
    word('later-path-domain',path_event['path']+4,4099)
    increment('later-nonquery-search',event['search'])
    query=next(e for e in block['events']if e['stage']=='query' and struct.unpack_from('<I',data,e['search'])[0]>0)
    word('later-search-index',query['search']+4,73730)
    frequency=next(e for e in block['events']if struct.unpack_from('<I',data,e['frequency'])[0]>0)
    increment('later-frequency-value',frequency['frequency']+8)
    check(data,expected)
    report=dict(status='passed',optimized=not __debug__,rejected=len(rows),root_digest_bypassed=True,child_processes=0,records=rows,
        qualification='Every retained parser/search/frequency/publication state byte is independently source-driven. Native frame payloads and allocation sizes are retained audited inputs. Negative controls include effective later-block state forgeries.')
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
