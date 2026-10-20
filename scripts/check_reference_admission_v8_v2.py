#!/usr/bin/env python3
"""Freeze source-aware bounded Ultra2 policy without native or Rust execution."""
from pathlib import Path
from types import SimpleNamespace
import argparse,collections,csv,hashlib,io,json,sys
ROOT=Path(__file__).resolve().parents[1]
DEST=ROOT/'tests/fixtures/reference-admission-v8-v2'
PINS={'scripts/check_reference_admission_v7.py': 'e7d00a7b657bd518cc41d81a5076f6d1f831c511862643e40e92252d4c6df93f', 'scripts/check_lazy_interop_capture_v2.py': '0cb1ea0cef617b732c02537dfda09f5d4ac3080d2f3e2dccab6cb2c399d8410f', 'tests/upstream/parameters/presets.tsv': '4e188cc16826c6665dcd440ae9d153484aeb4e561321a089be2aec260667881d', 'tests/fixtures/reference-admission-v7/README.md': '16274d528fc8a030aee9f605be1528c309063753ba9c42a575576398aa99ed3c', 'tests/fixtures/reference-admission-v7/broader.tsv': '420806286982a8f49e247a7ac12ca7730af80c60c9679f5ad6f649c39a93c8f5', 'tests/fixtures/reference-admission-v7/fuzz.tsv': 'bc0811b799d5b7b698372f2e3707906daf120fd4b609bab61cf7177e4cd63bdf', 'tests/fixtures/reference-admission-v7/greedy.tsv': '99a1639d778b8d6e7df3663b24506aed2170b008f08e7fc6c32c0093bf71c300', 'tests/fixtures/reference-admission-v7/interop.tsv': '4669c156877d149da9c66ce4bdd9e872367ab4892010552626054be9f1f0be08', 'tests/fixtures/reference-admission-v7/lazy.tsv': '922de316b6e39d413b420b8d7fdfa04dcbf9e30182922b3221d06b3798213e15', 'tests/fixtures/reference-admission-v7/loops.tsv': 'b01332f7dbe670b3a0bf0ba70098f444dcaf9f7fa769e30f1dbf0cdd910e7e7d', 'tests/fixtures/reference-admission-v7/provenance.json': '547009d418dd4b8f50f416aba5193538600f9622cb2f8a6432f2be8c09f21243'}

def need(ok,why):
    if not ok:raise ValueError(why)
def sha(data):return hashlib.sha256(data).hexdigest()
def encoded(value):return (json.dumps(value,sort_keys=True,indent=2)+'\n').encode()
def tsv(fields,rows):
    out=io.StringIO(newline='');writer=csv.DictWriter(out,fields,delimiter='\t',lineterminator='\n');writer.writeheader();writer.writerows(rows);return out.getvalue().encode()
def classify(values,size):
    need(type(size)is int and 0<=size<(1<<64)-1,'source extent')
    from check_reference_admission_v7 import classify as prior
    previous=prior(values,size)
    if values[6]!=9:return previous
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
    from check_reference_admission_v7 import classify as prior
    table=presets();files={};coverage={};changed={};reconstructed=0
    for filename in ['fuzz.tsv','loops.tsv','interop.tsv','broader.tsv','greedy.tsv','lazy.tsv']:
        rows=list(csv.DictReader(io.StringIO((ROOT/'tests/fixtures/reference-admission-v7'/filename).read_text()),delimiter='\t'))
        fields=list(rows[0]);fields.insert(fields.index('v7')+1,'v8');fields+=['v8_independent_tuple','v8_additive_c_required']
        for row in rows:
            old=row['v7'];row['v8']=old;row['v8_independent_tuple']='';row['v8_additive_c_required']=0
            if old=='advanced':continue
            size=int(row.get('source_size',row.get('prefix_size')))
            value=row.get('v7_independent_tuple') or row.get('v6_independent_tuple') or row.get('resolved_tuple',row.get('historical_resolved_tuple'))
            values=list(map(int,value.split(',')))if value else table[int(row['preset_tier']),int(row['preset_row'])]
            need(prior(values,size)==old,'exact predecessor status')
            if values[6]==9:
                if 'request'in row:request=tuple(map(int,row['request'].split(',')))
                elif 'overrides'in row:request=(int(row['level']),*map(int,row['overrides'].split(',')),int(row['hint']))
                else:request=(int(row['level']),0,0,0,0,0,0,0,0)
                independent=resolved_new_request(SimpleNamespace(source=bytes(size),request=request),table)
                need(value is None or independent==values,'independent historical tuple')
                values=independent;row['v8_independent_tuple']=','.join(map(str,values));reconstructed+=1
            row['v8']=classify(values,size)
            row['v8_additive_c_required']=int(old!='success'and row['v8']=='success')
            need(old!='success'or row['v8']=='success','preserve predecessor admission')
        files[filename]=tsv(fields,rows)
        coverage[filename]={v:dict(collections.Counter(r[v]for r in rows))for v in ['v7','v8']}
        changed[filename]=[r for r in rows if r['v7']!=r['v8']]
    rejected=0
    def reject(call):
        nonlocal rejected
        try:call()
        except(ValueError,TypeError):rejected+=1
        else:raise ValueError('malformed policy input accepted')
    base=[14,12,12,6,3,4095,9]
    for size in [True,-1,1.0,(1<<64)-1,None]:reject(lambda n=size:classify(base,n))
    for values in [None,(),[],[14,12,12,6,3,4095,True],[14,12,12,6.0,3,4095,9],[9,12,12,6,3,4095,9],[14,5,12,6,3,4095,9],[14,12,12,0,3,4095,9],[14,12,12,6,2,4095,9],[14,12,12,6,3,131073,9]]:reject(lambda v=values:classify(v,16384))
    for size,want in [(0,'resource'),(63,'resource'),(64,'success'),(16384,'success'),(16385,'resource')]:need(classify(base,size)==want,'source boundary')
    for at,value in [(0,17),(1,13),(2,13),(3,7),(5,4096)]:
        bad=base.copy();bad[at]=value;need(classify(bad,16384)=='resource','geometry cap')
    need(classify([13,12,12,6,3,4095,9],16384)=='resource','source fits window')
    need(not children and reconstructed==54 and rejected==15,'complete source-only controls')
    files['provenance.json']=encoded(dict(schema_version=8,input_sha256=PINS,generator_sha256=sha(Path(__file__).read_bytes()),coverage=coverage,changed=changed,independently_reconstructed=54,malformed_rejections=rejected,child_processes=0,files=[dict(path=n,bytes=len(b),sha256=sha(b))for n,b in files.items()],qualification='Source-only complete-fresh Ultra2 policy: source64..16384 within window,W16/H12/C12/S6,target4095. Original selector/numeric/source/request bytes and historical columns remain unchanged. All strategy9 tuples independently reconstructed; newly admitted public recipes, native interoperability and public enabling are separate gates.'))
    return files

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);g=p.add_mutually_exclusive_group(required=True);g.add_argument('--write',action='store_true');g.add_argument('--check',action='store_true');a=p.parse_args();files=project()
    if a.write:
        need(not DEST.exists(),'fresh successor destination');DEST.mkdir()
        for n,b in files.items():(DEST/n).write_bytes(b)
    else:
        need(DEST.is_dir()and not DEST.is_symlink()and {p.name for p in DEST.iterdir()}==set(files)|{'README.md'},'complete successor inventory')
        for n,b in files.items():need((DEST/n).is_file()and not(DEST/n).is_symlink()and (DEST/n).read_bytes()==b,'exact successor '+n)
    print(json.dumps(dict(status='passed',coverage=json.loads(files['provenance.json'])['coverage'],independently_reconstructed=54,rejected=15,child_processes=0),sort_keys=True))
