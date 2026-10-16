#!/usr/bin/env python3
"""Project immutable fresh startup C observations, frames and sparse search maps."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import tarfile
from oracles.optimal_startup import require
from oracles.optimal_startup_recipes import recipes
from oracles.optimal_startup_validation import validate_output
from oracles.tree_validation import parse

ROOT=Path(__file__).resolve().parents[1]
DEST=ROOT/'tests/fixtures/reference-optimal-startup'
STAGES=['begin','query','origin','seed','literal','relax','select','reverse_start','reverse','sequence','refresh','complete']
KEYS=['lit','ll','ml','of']
NONE=0xffffffff


def sha(data):return hashlib.sha256(data).hexdigest()
def enc(value):return (json.dumps(value,sort_keys=True,indent=2)+'\n').encode()
def words(*values):
    require(all(type(v)is int and 0<=v<=NONE for v in values),'u32 projection domain')
    return struct.pack('<'+'I'*len(values),*values)


def captured():
    report=parse((ROOT/'docs/evidence/2026-10-16-optimal-startup-capture.json').read_bytes())
    path=ROOT/'docs/evidence'/report['archive']
    require(path.is_file() and not path.is_symlink() and path.stat().st_size==report['archive_bytes']
            and sha(path.read_bytes())==report['archive_sha256'],'immutable startup archive')
    expected={r['path']:r for r in report['files']};retained={};total=0
    require(len(expected)==len(report['files']),'unique member inventory')
    with tarfile.open(path,'r:gz')as archive:
        entries=archive.getmembers()
        require(len(entries)==len(expected) and {e.name for e in entries}==set(expected),'complete archive inventory')
        for entry in entries:
            require(entry.isfile() and 0<=entry.size<=128<<20,'bounded regular member')
            total+=entry.size;require(total<=512<<20,'bounded expanded archive')
            data=archive.extractfile(entry).read();row=expected[entry.name]
            require(len(data)==row['bytes'] and sha(data)==row['sha256'],'exact member identity')
            if entry.name=='evidence.json' or entry.name.startswith(('requests/','artifacts/','source/')) or ('-run-startup_'in entry.name and entry.name.endswith('.stdout')):
                retained[entry.name]=data
    evidence=parse(retained['evidence.json'])
    require(evidence['status']=='passed' and evidence['source_unchanged']is True
            and evidence['audit']==report['qualification'],'completed independently audited capture')
    for name,digest in evidence['source'].items():
        require(sha((ROOT/name).read_bytes())==digest and sha(retained['source/'+name])==digest,'frozen capture source')
    return retained,report['archive_sha256']


def path_projection(name,request,events):
    # Lists below are observed C results, never candidate inputs to the helper.
    observed={}
    for e in events:
        if e['query']is not None:
            position=e['query'][0]
            require(position not in observed or observed[position]==e['matches'],'unambiguous position projection')
            observed[position]=e['matches']
    nonempty=sorted((p,m)for p,m in observed.items()if m)
    require(len(nonempty)<=64 and all(len(m)<=8 for _,m in nonempty),'embedded checked path reader capacity')
    n=len(request.source);encoded_name=name.encode('ascii')
    pack=bytearray(b'ZOPFX01\0'+words(1,len(encoded_name),n,request.min_match,request.target,request.literal_mode,n+3,len(events),len(nonempty),0))
    pack.extend(encoded_name);pack.extend(request.source)
    for position,matches in nonempty:
        pack.extend(words(position,len(matches)))
        for pair in matches:pack.extend(words(*pair))
    previous=[[0]*7 for _ in range(n+3)];frequencies=[0]*386;sequences=[]
    for e in events:
        pack.extend(words(STAGES.index(e['stage']),int(e['phase']=='reverse'),e['ip'],e['anchor'],
            *(NONE if e[k]is None else e[k]for k in ('cur','last','tail')),*e['repeats']))
        for k in ['stretch','reverse','query']:
            pack.extend(words(int(e[k]is not None)))
            if e[k]is not None:pack.extend(words(*e[k]))
        if e['query']is not None:
            pack.extend(words(len(e['matches'])))
            for pair in e['matches']:pack.extend(words(*pair))
        require(len(e['cells'])==n+3,'complete path extent')
        changes=[(i,c)for i,c in enumerate(e['cells'])if c!=previous[i]]
        pack.extend(words(len(changes)))
        for i,c in changes:pack.extend(words(i,*c))
        previous=e['cells']
        current=[v for k in KEYS for v in e['frequencies'][k]]+[e['sums'][k]for k in KEYS]+[e['base_prices'][k]for k in KEYS]+[int(e['predefined'])]
        require(len(current)==386,'complete frequencies')
        changes=[(i,v)for i,v in enumerate(current)if v!=frequencies[i]]
        pack.extend(words(len(changes)))
        for pair in changes:pack.extend(words(*pair))
        frequencies=current
        require(e['sequences'][:len(sequences)]==sequences,'append-only stored sequences')
        added=e['sequences'][len(sequences):];pack.extend(words(len(e['sequences']),len(added)))
        for row in added:pack.extend(words(*row))
        sequences=e['sequences']
    return bytes(pack)


def search_words(state):
    return state['heads']+[v for pair in state['nodes']for v in pair]+state['short_heads']+[state['frontier'],state['short_frontier']]


def project():
    stored,digest=captured();cases=recipes();pack=bytearray(b'ZOSFX01\0'+words(len(cases)));catalog=[]
    for name,request in cases.items():
        require(stored['requests/'+name+'.bin']==request.packet(),'exact immutable packet')
        raw=stored['logs/normal-run-'+name+'.stdout']
        require(raw==stored['logs/sanitized-run-'+name+'.stdout'],'both builds agree')
        validate_output(raw,request)
        events=[parse(line)for line in raw.splitlines() if b'"record":"event"'in line]
        events=[e for e in events if e['padding_pass']==0]
        require(2<=len(events)<=1024,'complete first padding pass')
        frame=stored[f'artifacts/normal/{name}.0.zst']
        for mode in ['normal','sanitized']:
            for padding in [0,1]:require(stored[f'artifacts/{mode}/{name}.{padding}.zst']==frame,'all four complete frames agree')
        path=path_projection(name,request,events);start=len(pack)
        pack.extend(words(request.window_log,request.hash_log,request.chain_log,request.search_log,len(frame),len(path),len(events)))
        for mode in ['normal','sanitized']:
            ledger=parse(stored[f'artifacts/{mode}/{name}.0.json'])
            require(len(ledger['allocations'])==2 and ledger['released']is True,'two released owners')
            pack.extend(words(*ledger['allocations'],ledger['peak_bytes']))
        pack.extend(frame);pack.extend(path)
        size=(1<<request.hash_log)+(1<<request.chain_log)+(1<<request.window_log if request.min_match==3 else 0)+2
        previous=[0]*size
        for index,e in enumerate(events):
            require(e['index']==index,'ordered C event index')
            if e['query']is not None:require(search_words(e['search_before'])==previous,'pre-query state equals preceding complete state')
            current=search_words(e['search']);require(len(current)==size,'fixed complete search dimensions')
            changes=[(i,v)for i,v in enumerate(current)if v!=previous[i]]
            require(e['query']is not None or index==0 or not changes,'search changes only in queries')
            pack.extend(words(len(changes)))
            for pair in changes:pack.extend(words(*pair))
            previous=current
        catalog.append(dict(name=name,offset=start,bytes=len(pack)-start,events=len(events),queries=sum(e['query']is not None for e in events),
            sequences=len(events[-1]['sequences']),frame_bytes=len(frame),frame_sha256=sha(frame),packet_sha256=sha(request.packet()),transcript_sha256=sha(raw)))
    require(len(pack)<=16<<20 and sum(c['events']for c in catalog)==4409,'bounded complete startup projection')
    provenance=dict(schema_version=1,archive_sha256=digest,pack_bytes=len(pack),pack_sha256=sha(pack),cases=catalog,
        qualification='Unchanged-provider ordinary CCtx C output projected only after independent full-state audit; embedded candidate lists are observations, not provider inputs. No retained publication or public Optimal admission.')
    return bytes(pack),enc(provenance)


def main(write):
    pack,provenance=project()
    if write:
        require(not DEST.exists(),'fresh immutable fixture directory');DEST.mkdir(parents=True)
        (DEST/'cases.bin').write_bytes(pack);(DEST/'provenance.json').write_bytes(provenance)
    require((DEST/'cases.bin').read_bytes()==pack and (DEST/'provenance.json').read_bytes()==provenance,'exact fixture reproduction')
    print(json.dumps(dict(status='passed',cases=12,events=4409,pack_bytes=len(pack),pack_sha256=sha(pack)),sort_keys=True))

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--write',action='store_true');main(parser.parse_args().write)
