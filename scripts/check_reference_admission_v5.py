#!/usr/bin/env python3
"""Freeze bounded tree admission over unchanged predecessor requests; no codec."""
from pathlib import Path
import argparse
import collections
import csv
import hashlib
import io
import json
ROOT=Path(__file__).resolve().parents[1]
DEST=ROOT/'tests/fixtures/reference-admission-v5'
PINS={'scripts/check_reference_admission_v4.py': '0589976b7ebdc12d132e8e9c1cc785d9952fae0ec158a3a61ca79d39f4785091', 'scripts/check_lazy_interop_capture_v2.py': '0cb1ea0cef617b732c02537dfda09f5d4ac3080d2f3e2dccab6cb2c399d8410f', 'scripts/oracles/lazy_interop.py': 'db2f4e9568eff5a2508e7b65e5106404fc03c957ea8cf0afb7d3ab887b56d937', 'tests/upstream/parameters/presets.tsv': '4e188cc16826c6665dcd440ae9d153484aeb4e561321a089be2aec260667881d', 'docs/evidence/2026-10-13-lazy-public-interop.json': 'e6dc06606c193f17b51d4556a362140d5e995cc78c4139ab9e8afdd4ebe26b46', 'tests/fixtures/reference-admission-v4/README.md': '8ab0c5337362dc2f33308f842376f692811d7f7c38e23f0f8f2f831bd952e25c', 'tests/fixtures/reference-admission-v4/broader.tsv': 'b859aaaa9dcc7ccafd3cad3a859524870e9d570fcef86bf8511812471122f7ba', 'tests/fixtures/reference-admission-v4/fuzz.tsv': 'd82a887c2e9aff5848d2aa9385ced786b91422b239d4817a233b87b97e888f45', 'tests/fixtures/reference-admission-v4/greedy.tsv': '3fea6e0c916a1fd8d391f3473fb490db07dc8d746559d57b01e958da7063c765', 'tests/fixtures/reference-admission-v4/interop.tsv': '175ee2f09d3646bdf1e27083686eac4d7bb38b593c9dfc1daeed750f46d41110', 'tests/fixtures/reference-admission-v4/loops.tsv': '84a06c0ff6794d2e5c5eccc8fb160252dbd72511c6634985a18fa06e1ba92d85', 'tests/fixtures/reference-admission-v4/provenance.json': '703418ab6cb62f9cfbbf2c4ac8a8a3ca6524302aa74fc4bbb5a5722a95749460'}
def need(x,why):
    if not x:raise ValueError(why)
def sha(b):return hashlib.sha256(b).hexdigest()
def encoded(v):return (json.dumps(v,sort_keys=True,indent=2)+'\n').encode()
def rows(b):return list(csv.DictReader(io.StringIO(b.decode()),delimiter='\t'))
def tsv(fields,data):
    out=io.StringIO(newline='');w=csv.DictWriter(out,fields,delimiter='\t',lineterminator='\n');w.writeheader();w.writerows(data);return out.getvalue().encode()
def classify(values):
    import check_reference_admission_v4 as prior
    old=prior.classify(values)
    if values[6]!=6:return old
    w,c,h,s,_,_,_=values
    return 'resource' if w>21 or c>19 or h>19 or s>8 else 'success'
def projection():
    for name,digest in PINS.items():
        p=ROOT/name;need(p.is_file() and not p.is_symlink() and sha(p.read_bytes())==digest,'frozen input '+name)
    import check_reference_admission_v4 as prior
    from check_lazy_interop_capture_v2 import presets,resolved_new_request
    from oracles import lazy_interop as generator
    table=presets();files={};coverage={};changed={}
    for filename in ['fuzz.tsv','loops.tsv','interop.tsv','broader.tsv','greedy.tsv']:
        data=rows((ROOT/'tests/fixtures/reference-admission-v4'/filename).read_bytes());fields=[k for k in data[0] if k!='v3'];fields.insert(fields.index('v4')+1,'v5')
        for row in data:
            row.pop('v3')
            value=None if row['v4']=='advanced' else table[int(row['preset_tier']),int(row['preset_row'])] if filename in ['fuzz.tsv','loops.tsv'] else list(map(int,row['historical_resolved_tuple'].split(',')))
            row['v5']='advanced' if value is None else classify(value)
            if 'additive_c_required' in row:row['additive_c_required']=int(row['v4']!='success' and row['v5']=='success')
            need(row['v4']!='success' or row['v5']=='success','preserve historical admissions')
        files[filename]=tsv(fields,data);coverage[filename]={v:dict(collections.Counter(r[v] for r in data)) for v in ['v4','v5']};changed[filename]=[r for r in data if r['v4']!=r['v5']]
    cases=generator.recipes();historical=rows((ROOT/'tests/fixtures/reference-admission-v4/greedy.tsv').read_bytes());need(len(cases)==886 and len(historical)==422,'complete predecessor inventory');data=[]
    for i,case in enumerate(cases):
        if i<422:
            old=historical[i];need(case.name==old['case'] and sha(case.source)==old['source_sha256'] and case.request==tuple(map(int,old['request'].split(','))),'unchanged historical request')
            resolved=list(map(int,old['historical_resolved_tuple'].split(',')));origin='historical Rust tuple'
        else:resolved=resolved_new_request(case,table);origin='independent pinned-preset reconstruction'
        need(prior.classify(resolved)==case.expected,'predecessor status binding')
        status=classify(resolved)
        if status=='success' and case.expected!='success':
            need(case.request[0]>0,'independent new positive-level tree scope')
            need(resolved_new_request(case,table)==resolved,'independent newly admitted tuple')
        data.append(dict(case=case.name,source_sha256=sha(case.source),source_size=len(case.source),request=','.join(map(str,case.request)),resolved_tuple=','.join(map(str,resolved)),resolution_origin=origin,v4=case.expected,v5=status,additive_c_required=int(case.expected!='success' and status=='success')))
    files['lazy.tsv']=tsv(list(data[0]),data);coverage['lazy.tsv']={v:dict(collections.Counter(r[v] for r in data)) for v in ['v4','v5']};changed['lazy.tsv']=[r for r in data if r['v4']!=r['v5']]
    malformed=0
    for bad in [None,(),[],[10,6,6,1,4,0,True],[10,6,6,1.0,4,0,6],[9,6,6,1,4,0,6],[10,5,6,1,4,0,6],[10,6,6,0,4,0,6],[10,6,6,1,2,0,6],[10,6,6,1,4,131073,6],[10,6,6,1,4,0,10]]:
        try:classify(bad)
        except ValueError:malformed+=1
        else:raise ValueError('malformed profile accepted')
    need(malformed==11,'malformed profile coverage')
    files['provenance.json']=encoded(dict(schema_version=5,upstream_revision=generator.REVISION,generator=str(Path(__file__).relative_to(ROOT)),generator_sha256=sha(Path(__file__).read_bytes()),input_sha256=PINS,coverage=coverage,changed=changed,malformed_rejections=malformed,fixtures=[dict(path=n,bytes=len(b),sha256=sha(b)) for n,b in sorted(files.items())],qualification='Source-only tree W21/H19/C19/S8 policy. Original selector/numeric and all 886 historical public source/request bytes are unchanged. Presets are pre-adjustment C values; archived tuples remain historical Rust observations; all newly admitted positive-level requests are independently reconstructed. New C execution and public enabling are separate gates.'))
    return files
if __name__=='__main__':
    p=argparse.ArgumentParser();g=p.add_mutually_exclusive_group(required=True);g.add_argument('--write',action='store_true');g.add_argument('--check',action='store_true');a=p.parse_args();files=projection()
    if a.write:
        need(not DEST.exists() and not DEST.is_symlink(),'fresh successor destination');DEST.mkdir()
        for n,b in files.items():(DEST/n).write_bytes(b)
    else:
        need(DEST.is_dir() and not DEST.is_symlink() and {p.name for p in DEST.iterdir()}==set(files)|{'README.md'},'complete successor inventory')
        for n,b in files.items():need((DEST/n).is_file() and not (DEST/n).is_symlink() and (DEST/n).read_bytes()==b,'immutable successor '+n)
    print(json.dumps(json.loads(files['provenance.json'])['coverage'],sort_keys=True))
