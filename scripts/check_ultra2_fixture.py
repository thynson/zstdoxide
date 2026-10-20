#!/usr/bin/env python3
"""Reconstruct every Ultra2 fixture state from independent source-only models."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys
sys.dont_write_bytecode=True
from oracles.ultra2_recipes import recipes
from oracles.ultra2_validation import records
from oracles.ultra_scalar import need,load
from check_ultra2_capture import read,validate_ledger_pair

ROOT=Path(__file__).resolve().parents[1]
DEST=ROOT/'tests/fixtures/reference-ultra2'
STAGES=('begin','query','origin','seed','literal','relax','select','reverse_start','reverse','sequence','refresh','complete','alternative')
KEYS=('lit','ll','ml','of')
def sha(data):return hashlib.sha256(data).hexdigest()


def reconstruct_case(capture,name):
    children=[]
    def guard(event,args):
        if event in ('subprocess.Popen','os.system','os.fork','os.posix_spawn','os.exec','os.spawn'):
            children.append(event);raise RuntimeError('independent reconstruction worker forbids children')
    sys.addaudithook(guard)
    evidence=load(read(capture,'evidence.json',8<<20));provenance=load((DEST/'provenance.json').read_bytes())
    need(sha((capture/'evidence.json').read_bytes())==provenance['capture_evidence_sha256'] and evidence['status']=='passed','audited native identity')
    recipe=recipes()[name];r=recipe.request;rows,_=records(r,recipe.seed);output=bytearray();layouts=[]
    def words(*values):
        need(all(type(v)is int and 0<=v<=0xffffffff for v in values),'independent u32 encoding')
        output.extend(struct.pack('<'+'I'*len(values),*values))
    def frequencies(row):
        return [value for key in KEYS for value in row['tables'][key]]+[row['sums'][key]for key in KEYS]+[row['cache'][key]for key in KEYS]+[int(row['predefined'])]
    def optional(value):
        words(int(value is not None))
        if value is not None:words(*value)
    encoded=name.encode('ascii');phases=sum(row['record']=='pass_begin'for row in rows)
    words(len(encoded),len(r.source),r.window_log,r.hash_log,r.chain_log,r.search_log,r.min_match,r.target,r.literal_mode,int(recipe.seed),phases,4099)
    ledgers=[]
    for mode in ('normal','sanitized'):
        path=f'artifacts/{mode}/{name}.0.json';data=read(capture,path,32768)
        need(evidence['artifacts'][path]==dict(bytes=len(data),sha256=sha(data)),'opaque native allocation identity')
        ledger=load(data);ledgers.append(ledger);words(*ledger['allocations'],ledger['peak_bytes'])
    validate_ledger_pair(*ledgers);output.extend(encoded);output.extend(r.source)
    entry_at=len(output);entry=rows[0]
    words(*map(int,entry['predicates']),int(entry['seed']),entry['current'],entry['dictionary_limit'],entry['low_limit'],entry['next_to_update'],entry['source_bytes'],*entry['original_repeats'],entry['sequence_prefix'])
    counts=[0]*386;sequence_count=0;event_layout=[];active=None
    for row in rows[1:]:
        kind=row['record']
        if kind=='pass_begin':
            active=dict(phase=row['phase'],header=len(output),events=[]);layouts.append(active);sequence_count=0
            event_count=sum(e['record']=='event'and e.get('pass_name')==row['phase']for e in rows)
            words(int(row['phase']=='encoding'),row['origin'],*row['repeats'],event_count,*frequencies(row['frequency']))
        elif kind=='event':
            at=len(output);words(STAGES.index(row['stage']),int(row['phase']=='reverse'),row['ip'],row['anchor'],*[0xffffffff if row[key]is None else row[key]for key in ('cur','last','tail')],*row['repeats'])
            optional(row['stretch']);optional(row['reverse']);optional(row['query'])
            if row['query']is not None:
                words(len(row['matches']))
                for pair in row['matches']:words(*pair)
            words(int(row['alternative']is not None));alternative=None
            if row['alternative']is not None:alternative=len(output);output.extend(struct.pack('<12q',*row['alternative']))
            path=len(output);words(len(row['cells']['changes']))
            for index,cell in row['cells']['changes']:words(index,*cell)
            current=[v for k in KEYS for v in row['frequencies'][k]]+[row['sums'][k]for k in KEYS]+[row['base_prices'][k]for k in KEYS]+[int(row['predefined'])]
            changed=[(i,v)for i,v in enumerate(current)if counts[i]!=v];frequency_at=len(output);words(len(changed))
            for pair in changed:words(*pair)
            counts=current;added=row['sequences'][sequence_count:];words(len(row['sequences']),len(added))
            for sequence in added:words(*sequence)
            sequence_count=len(row['sequences']);search=len(output);words(len(row['search']['changes']))
            for pair in row['search']['changes']:words(*pair)
            active['events'].append(dict(start=at,stage=row['stage'],alternative=alternative,path=path,frequency=frequency_at,search=search))
        elif kind=='pass_end':
            active['footer']=len(output);words(*row['candidate_repeats'],len(row['sequences']),row['tail'],*frequencies(row['frequency']))
            words(int(row['phase']=='seeding'))
        elif kind=='reset':
            active['reset']=len(output);words(row['discarded_sequences'],row['discarded_literals'],row['discarded_tail'],*row['caller_repeats'],*row['discarded_repeats'],row['origin_before'],row['origin_after'],row['low_limit'],row['dictionary_limit'],row['next_to_update'],int(row['tables_preserved']),*frequencies(row['frequency']))
        else:raise ValueError('unknown independent lifecycle record')
    need(not children,'no child processes')
    return bytes(output),dict(name=name,entry=entry_at,phases=layouts,events=sum(len(p['events'])for p in layouts),child_processes=0,optimized=not __debug__)


def check(data,expected,digest=True):
    provenance=load((DEST/'provenance.json').read_bytes())
    need(type(data)is bytes and len(data)==provenance['pack_bytes'],'exact fixture extent')
    if digest:need(sha(data)==provenance['pack_sha256'],'immutable native fixture digest')
    need(data==expected,'every independent lifecycle/state byte')


def controls(data,expected,layouts,out):
    need(not out.exists(),'fresh reconstruction controls');out.mkdir();results=[]
    def reject(name,at,remove,replacement):
        forged=data[:at]+replacement+data[at+remove:]
        try:check(forged,expected,False)
        except ValueError as error:diagnostic=str(error)
        else:raise ValueError('accepted reconstruction forgery '+name)
        row=dict(name=name,offset=at,removed=remove,replacement_hex=replacement.hex(),bytes=len(forged),sha256=sha(forged),diagnostic=diagnostic)
        p=out/(name+'.json');p.write_text(json.dumps(row,sort_keys=True)+'\n');saved=load(p.read_bytes())
        recovered=data[:saved['offset']]+bytes.fromhex(saved['replacement_hex'])+data[saved['offset']+saved['removed']:]
        need(recovered==forged and sha(recovered)==saved['sha256'],'lossless forgery description');results.append(row)
    def change(name,at,value=None):
        old=struct.unpack_from('<I',data,at)[0];reject(name,at,4,struct.pack('<I',old^1 if value is None else value))
    reject('truncated',len(data)-1,1,b'');reject('trailing',len(data),0,b'!');reject('magic',0,1,b'!')
    change('case-count',8,54);case=next(c for c in layouts if c['name']=='u2_seed_alternative_seed_0');base=case['offset']
    change('request-seed',base+36,0);change('phase-count',base+40,1)
    for column in range(15):change('entry-'+str(column),base+case['entry']+4*column)
    first,second=case['phases'];change('phase-order',base+first['header'],1);change('stale-origin',base+second['header']+4,2)
    change('second-original-repeat',base+second['header']+8,99);change('phase-event-count',base+second['header']+20,0)
    for column in range(15):change('reset-'+str(column),base+first['reset']+4*column)
    change('reset-frequency',base+first['reset']+60);change('second-frequency',base+second['header']+24)
    change('encoding-tail',base+second['footer']+16)
    event=next(e for e in first['events'] if e['alternative']is not None)
    for column in range(12):
        at=base+event['alternative']+8*column;old=struct.unpack_from('<q',data,at)[0];reject('alternative-'+str(column),at,8,struct.pack('<q',old+1))
    change('path-domain',base+event['path']+4,4099)
    query=next(e for e in second['events']if e['stage']=='query');change('search-domain',base+query['search']+4,73730)
    check(data,expected)
    result=dict(status='passed',optimized=not __debug__,rejected=len(results),root_digest_bypassed=True,records=results,qualification='Every lifecycle and parser/search/frequency state byte is independently source-derived. Only qualified native allocation sizes are opaque. Forgery controls bypass the outer digest.')
    (out/'summary.json').write_text(json.dumps(result,sort_keys=True,indent=2)+'\n');return result


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('capture',type=Path);p.add_argument('--controls',type=Path,required=True);a=p.parse_args()
    expected=bytearray(b'ZU2FX01\0'+struct.pack('<I',53));layouts=[]
    for name in recipes():
        body,layout=reconstruct_case(a.capture.resolve(strict=True),name);layout['offset']=len(expected);expected.extend(body);layouts.append(layout)
    data=(DEST/'cases.bin').read_bytes();check(data,bytes(expected));result=controls(data,bytes(expected),layouts,a.controls)
    print(json.dumps(dict(status='passed',events=sum(c['events']for c in layouts),bytes=len(data),sha256=sha(data),rejected=result['rejected'])))
if __name__=='__main__':main()
