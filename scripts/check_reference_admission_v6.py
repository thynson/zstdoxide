#!/usr/bin/env python3
"""Freeze source-aware bounded Optimal policy without native or Rust execution."""
from pathlib import Path
from types import SimpleNamespace
import argparse,collections,csv,hashlib,io,json,sys
ROOT=Path(__file__).resolve().parents[1]
DEST=ROOT/'tests/fixtures/reference-admission-v6'
PINS={'scripts/check_reference_admission_v5.py': 'f016f23bf50a00a529276e506684c4cee1129aebbe222ca5b812cff34537287b', 'scripts/check_lazy_interop_capture_v2.py': '0cb1ea0cef617b732c02537dfda09f5d4ac3080d2f3e2dccab6cb2c399d8410f', 'tests/upstream/parameters/presets.tsv': '4e188cc16826c6665dcd440ae9d153484aeb4e561321a089be2aec260667881d', 'tests/fixtures/reference-admission-v5/README.md': '9a0f073716215e978c3d18eb0be1c3be665ab5bb4cfeed931396aceff9854a2d', 'tests/fixtures/reference-admission-v5/broader.tsv': '987997de3241abd0c07f149f120eb432c2d70ba53a65582a0ea0ae865a3602eb', 'tests/fixtures/reference-admission-v5/fuzz.tsv': '3f6cc5d182eef4da8ac8919bbe9c26e06a62148acdeb41ce03a12381598c717d', 'tests/fixtures/reference-admission-v5/greedy.tsv': '48aa1e7d6155b3cb9a36ca441dafa9a63e894f97e4b1d6b463387772dc1ace24', 'tests/fixtures/reference-admission-v5/interop.tsv': '8d13b0ba3d264e9ea76c3d437dbfc6a7c568c7cd356d7b8b2d7d50edb9a625be', 'tests/fixtures/reference-admission-v5/lazy.tsv': '2c210de372c6f918c55bd7fb8883290a8f0cdfc5d2d7b91f81b4182dd21edd20', 'tests/fixtures/reference-admission-v5/loops.tsv': 'a7725e10501a9190d53c0d15b2ff746f873502f293195f4e335474b4694f3be2', 'tests/fixtures/reference-admission-v5/provenance.json': 'be264e270bac401b405bd6b18a58cefbc624b59d88ad54e1948abbd55b10c6bc'}

def need(ok,why):
    if not ok:raise ValueError(why)
def sha(data):return hashlib.sha256(data).hexdigest()
def encoded(value):return (json.dumps(value,sort_keys=True,indent=2)+'\n').encode()
def tsv(fields,rows):
    out=io.StringIO(newline='');writer=csv.DictWriter(out,fields,delimiter='\t',lineterminator='\n');writer.writeheader();writer.writerows(rows);return out.getvalue().encode()
def classify(values,size):
    need(type(size)is int and 0<=size<(1<<64)-1,'source extent')
    from check_reference_admission_v5 import classify as prior
    previous=prior(values)
    if values[6]!=7:return previous
    w,c,h,s,_,t,_=values
    return 'success'if 64<=size<=16384 and size<=1<<w and w<=16 and c<=12 and h<=12 and s<=6 and t<=4095 else'resource'

def project():
    children=[]
    def guard(event,args):
        if event in ('subprocess.Popen','os.system','os.fork','os.posix_spawn'):
            children.append(event);raise ValueError('source-only projection forbids children')
    sys.addaudithook(guard)
    for name,digest in PINS.items():
        p=ROOT/name;need(p.is_file()and not p.is_symlink()and sha(p.read_bytes())==digest,'immutable predecessor '+name)
    from check_lazy_interop_capture_v2 import presets,resolved_new_request
    from check_reference_admission_v5 import classify as prior
    table=presets();files={};coverage={};changed={};reconstructed=0
    for filename in ['fuzz.tsv','loops.tsv','interop.tsv','broader.tsv','greedy.tsv','lazy.tsv']:
        rows=list(csv.DictReader(io.StringIO((ROOT/'tests/fixtures/reference-admission-v5'/filename).read_text()),delimiter='\t'))
        fields=list(rows[0]);fields.insert(fields.index('v5')+1,'v6');fields+=['v6_independent_tuple','v6_additive_c_required']
        for row in rows:
            old=row['v5'];row['v6']=old;row['v6_independent_tuple']='';row['v6_additive_c_required']=0
            if old=='advanced':continue
            size=int(row.get('source_size',row.get('prefix_size')))
            value=row.get('resolved_tuple',row.get('historical_resolved_tuple'))
            values=list(map(int,value.split(',')))if value else table[int(row['preset_tier']),int(row['preset_row'])]
            need(prior(values)==old,'exact predecessor status')
            if values[6]==7:
                if 'request'in row:request=tuple(map(int,row['request'].split(',')))
                elif 'overrides'in row:request=(int(row['level']),*map(int,row['overrides'].split(',')),int(row['hint']))
                else:request=(int(row['level']),0,0,0,0,0,0,0,0)
                independent=resolved_new_request(SimpleNamespace(source=bytes(size),request=request),table)
                need(value is None or independent==values,'independent historical tuple')
                values=independent;row['v6_independent_tuple']=','.join(map(str,values));reconstructed+=1
            row['v6']=classify(values,size)
            row['v6_additive_c_required']=int(old!='success'and row['v6']=='success')
            need(old!='success'or row['v6']=='success','preserve predecessor admission')
        files[filename]=tsv(fields,rows)
        coverage[filename]={v:dict(collections.Counter(r[v]for r in rows))for v in ['v5','v6']}
        changed[filename]=[r for r in rows if r['v5']!=r['v6']]
    rejected=0
    def reject(call):
        nonlocal rejected
        try:call()
        except(ValueError,TypeError):rejected+=1
        else:raise ValueError('malformed policy input accepted')
    base=[14,12,12,6,3,4095,7]
    for size in [True,-1,1.0,(1<<64)-1,None]:reject(lambda n=size:classify(base,n))
    for values in [None,(),[],[14,12,12,6,3,4095,True],[14,12,12,6.0,3,4095,7],[9,12,12,6,3,4095,7],[14,5,12,6,3,4095,7],[14,12,12,0,3,4095,7],[14,12,12,6,2,4095,7],[14,12,12,6,3,131073,7]]:reject(lambda v=values:classify(v,16384))
    for size,want in [(0,'resource'),(63,'resource'),(64,'success'),(16384,'success'),(16385,'resource')]:need(classify(base,size)==want,'source boundary')
    for at,value in [(0,17),(1,13),(2,13),(3,7),(5,4096)]:
        bad=base.copy();bad[at]=value;need(classify(bad,16384)=='resource','geometry cap')
    need(classify([13,12,12,6,3,4095,7],16384)=='resource','source fits window')
    need(not children and reconstructed==22 and rejected==15,'complete source-only controls')
    files['provenance.json']=encoded(dict(schema_version=6,input_sha256=PINS,generator_sha256=sha(Path(__file__).read_bytes()),coverage=coverage,changed=changed,independently_reconstructed=22,malformed_rejections=rejected,child_processes=0,files=[dict(path=n,bytes=len(b),sha256=sha(b))for n,b in files.items()],qualification='Source-only complete-fresh Optimal policy: source64..16384 within window,W16/H12/C12/S6,target4095. Original selector/numeric/source/request bytes and historical columns remain unchanged. All strategy7 tuples independently reconstructed; newly admitted public recipes, native interoperability and public enabling are separate gates.'))
    return files

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);g=p.add_mutually_exclusive_group(required=True);g.add_argument('--write',action='store_true');g.add_argument('--check',action='store_true');a=p.parse_args();files=project()
    if a.write:
        need(not DEST.exists(),'fresh successor destination');DEST.mkdir()
        for n,b in files.items():(DEST/n).write_bytes(b)
    else:
        need(DEST.is_dir()and not DEST.is_symlink()and {p.name for p in DEST.iterdir()}==set(files)|{'README.md'},'complete successor inventory')
        for n,b in files.items():need((DEST/n).is_file()and not(DEST/n).is_symlink()and (DEST/n).read_bytes()==b,'exact successor '+n)
    print(json.dumps(dict(status='passed',coverage=json.loads(files['provenance.json'])['coverage'],independently_reconstructed=22,rejected=15,child_processes=0),sort_keys=True))
