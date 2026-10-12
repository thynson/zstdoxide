#!/usr/bin/env python3
"""Freeze Greedy admission over unchanged historical inputs; no codec execution."""
from pathlib import Path
import argparse
import collections
import csv
import hashlib
import io
import json
import re
import tarfile

ROOT=Path(__file__).resolve().parents[1]
DESTINATION=ROOT/'tests/fixtures/reference-admission-v3'
PINS={'scripts/check_reference_admission_v2.py': '8df6864e736337c9150f111c000221975308c0cd72f1775d36981e13d59bef69',
 'scripts/check_double_fast_interop_recipes.py': '83af1fd5b89b0d512a3a1294492b6b5a9e73ad61f0bb992c043f839da3ae0fcf',
 'scripts/oracles/double_fast_interop.py': '05eefcf492ce7dca0741d3348d165aca8a48e402aa63710d1c043775125705f3',
 'scripts/oracles/double_fast_frames.py': 'ac10609d7fe7c0f4983899238dc8a852ca437e35aff42b317599a0286c8ea656',
 'scripts/oracles/double_fast_interop_protocol.py': 'af9562efe67846530ab364ce6c97388af7f407147040b67e330643517e5ad543',
 'scripts/import_reference_fast_fuzz.py': 'cbbbc0d8b4c8709b3002c7d55d995ed61477fbfa42f2ee624952d0d4c814adbc',
 'tests/upstream/parameters/presets.tsv': '4e188cc16826c6665dcd440ae9d153484aeb4e561321a089be2aec260667881d',
 'tests/upstream/reference-fast-fuzz/observations.tsv': '77a868913331f7e9edd4a73a60634eebd97d83b5b57050b6945fbde5b317b4e5',
 'tests/fixtures/reference-double-fast-interop-recipes/recipes.json': '81f39a8303c41232b5d4993168f986f967aa2b1034efc77b5f86c4b401373add',
 'docs/evidence/2026-10-12-double-fast-interop-full.json': 'b7c90ca59a094c9307ab4b361333b7391a86ad63d73885012fdf0d1b093ea476',
 'docs/evidence/2026-10-12-double-fast-interop-full.tar.gz': '9a3bede445e00aa88782516b24535da3a990cef001866487b5b36f7e5f569a76',
 'tests/fixtures/reference-admission-v2/README.md': '2aa36ddc6770a65b8391b6f704af054012910ffb0174a6d3a865b504387c410c',
 'tests/fixtures/reference-admission-v2/fuzz.tsv': '7b2fbce793df15e2185b65bad272e9e6428a614fdb00905f949bfeb8d45bbc13',
 'tests/fixtures/reference-admission-v2/interop.tsv': '6c1b9c77329cba64893738b92304d5e0020c4bc0a8ff08e6ea2da9d27d0c516c',
 'tests/fixtures/reference-admission-v2/loops.tsv': 'c5c2bed1fe2dcf8631d89eecf98510cab81b9c115d365e6a5dadef216245ce6d',
 'tests/fixtures/reference-admission-v2/provenance.json': 'a0c9899b439992f2e37b85be36276e63aa5a531f2bce012c20305e1ba6d99304'}
DATA_SHA={
    'fuzz.tsv':'e7c8b01438e2bb35cc66a81d42ad4fa2f02203d222c25600efc2205267dae611',
    'interop.tsv':'d1297efaeb9496515f41aaab46bc1a903dd652710234bbac4fbc7306463a7f24',
    'broader.tsv':'545ab96911821c87a0dafe2d8ad06b5bdc83de2da8ff08a7a9e69d791c4db24c',
    'loops.tsv':'bd35c8a40cc7f0bb01dc7393f1d072ff1240e923ffabe26cea24733e613d410e',
}
ARCHIVE='docs/evidence/2026-10-12-double-fast-interop-full.tar.gz'


def require(value,reason):
    if not value:raise ValueError(reason)


def sha(data):return hashlib.sha256(data).hexdigest()


def encoded(value):return (json.dumps(value,sort_keys=True,indent=2)+'\n').encode()


def table(data):return list(csv.DictReader(io.StringIO(data.decode('ascii')),delimiter='\t'))


def tsv(fields,rows):
    stream=io.StringIO(newline='');writer=csv.writer(stream,delimiter='\t',lineterminator='\n');writer.writerow(fields);writer.writerows(rows)
    return stream.getvalue().encode('ascii')


def classify(values):
    require(type(values) is list and len(values)==7 and all(type(n) is int for n in values),'exact scalar tuple')
    limits=[(10,31),(6,30),(6,30),(1,30),(3,7),(0,131072),(1,9)]
    require(all(lo<=n<=hi for n,(lo,hi) in zip(values,limits)),'valid scalar tuple')
    window,chain,table_log,search,_,_,strategy=values
    if strategy not in (1,2,3):return 'unsupported'
    if strategy==1:return 'resource' if window>20 or table_log>18 else 'success'
    if strategy==2:return 'resource' if window>21 or table_log>18 or chain>18 else 'success'
    return 'resource' if window>21 or table_log>19 or (window<=14 and (chain>18 or search>8)) else 'success'


def archived_observations():
    # Read only bounded exact members from the already pinned chunk archive.
    # No archived Python is imported/executed and nothing is extracted to disk.
    prefix='double-fast-interop-full/'
    with tarfile.open(ROOT/ARCHIVE) as archive:
        def member(name,cap):
            item=archive.getmember(prefix+name)
            require(item.isfile() and 0<=item.size<=cap,'bounded archive member')
            data=archive.extractfile(item).read(cap+1)
            require(len(data)==item.size,'complete member bytes')
            return data
        mapping=json.loads(member('file-map.json',16<<20))
        record=mapping['capture/evidence.json']
        require(type(record['bytes']) is int and 0<record['bytes']<=4<<20
                and len(record['chunks'])==(record['bytes']+4095)//4096,'bounded evidence chunk map')
        chunks=[]
        for digest in record['chunks']:
            require(type(digest) is str and re.fullmatch('[0-9a-f]{64}',digest),'chunk key')
            data=member('chunks/'+digest+'.bin',4096);require(sha(data)==digest,'chunk content hash');chunks.append(data)
        data=b''.join(chunks)
        require(len(data)==record['bytes'] and sha(data)==record['sha256'],'archived evidence identity')
        value=json.loads(data)
        require(value['status']=='passed' and value['upstream_revision']=='f8745da6ff1ad1e7bab384bd1f9d742439278e99'
                and value['source_before']==value['source_after'],'completed historical capture')
        return value,sha(data)


def projection():
    for name,digest in PINS.items():
        p=ROOT/name;require(p.is_file() and not p.is_symlink() and sha(p.read_bytes())==digest,'frozen input: '+name)
    import check_reference_admission_v2 as prior
    previous=prior.check()
    import check_double_fast_interop_recipes as broad
    broad.check()
    from oracles import double_fast_interop as generator
    presets={(int(r['tier']),int(r['row'])):[int(r[k]) for k in ('window_log','chain_log','hash_log','search_log','min_match','target_length','strategy')]
             for r in table((ROOT/'tests/upstream/parameters/presets.tsv').read_bytes())}
    fuzz=[]
    for r in table(previous['fuzz.tsv']):
        old=r['successor']
        new='advanced' if old=='advanced' else classify(presets[int(r['preset_tier']),int(r['preset_row'])])
        require(new!='resource','fixed selector presets fit before source reduction')
        fuzz.append([r[k] for k in ('case','input_sha256','prefix_size','level')]+[old,new]+[r[k] for k in ('preset_tier','preset_row','preset_strategy')])
    interop=[]
    for r in table(previous['interop.tsv']):
        values=list(map(int,r['historical_resolved_tuple'].split(',')));old=r['successor'];new=classify(values)
        interop.append([r[k] for k in ('case','source_sha256','source_size','level','hint','overrides','historical_resolved_tuple')]+[old,new,int(old!='success' and new=='success')])
    loops=[]
    for r in table(previous['loops.tsv']):
        values=presets[int(r['preset_tier']),int(r['preset_row'])];old=r['successor'];new=classify(values)
        require(new!='resource','fixed numeric presets fit before source reduction')
        loops.append([r[k] for k in ('family','source_size','level')]+[old,new]+[r[k] for k in ('preset_tier','preset_row','preset_strategy')])
    captured,evidence_sha=archived_observations();recipes=generator.recipes();broader=[]
    require(len(captured['cases'])==len(recipes)==159,'historical broader recipe count')
    for recipe,observed in zip(recipes,captured['cases'],strict=True):
        require(prior.encoded(recipe.metadata())==prior.encoded(observed['recipe']),'unchanged broader source/request identity')
        require(observed['rust']['safe']==observed['rust']['all'],'historical safe/all equality')
        values=observed['rust']['safe']['resolved'];old=prior.classify(values,2);new=classify(values)
        require(old==recipe.expected==observed['rust']['safe']['status'],'historical broader result')
        if old!='success':require(observed['c']=={'status':'not-run-for-rejected-Rust-profile'},'preserve missing historical C execution')
        broader.append([recipe.name,sha(recipe.source),len(recipe.source),','.join(map(str,recipe.request)),','.join(map(str,values)),old,new,int(old!='success' and new=='success')])
    def counts(rows,old,new):return {'v2':dict(collections.Counter(r[old] for r in rows)),'v3':dict(collections.Counter(r[new] for r in rows))}
    coverage={'selectors':counts(fuzz,4,5),'historical92':counts(interop,7,8),'broader159':counts(broader,5,6),
              'loops':{name:counts([r for r in loops if r[0]==name],3,4) for name in ('empty','initial_match')}}
    require(coverage=={'selectors':{'v2':{'success':61,'unsupported':16,'advanced':4},'v3':{'success':62,'unsupported':15,'advanced':4}},
        'historical92':{'v2':{'success':85,'unsupported':4,'resource':3},'v3':{'success':86,'unsupported':3,'resource':3}},
        'broader159':{'v2':{'success':113,'unsupported':42,'resource':4},'v3':{'success':121,'unsupported':33,'resource':5}},
        'loops':{'empty':{'v2':{'success':5,'unsupported':19},'v3':{'success':6,'unsupported':18}},
                 'initial_match':{'v2':{'success':3,'unsupported':16},'v3':{'success':4,'unsupported':15}}}},'fixed admission counts')
    files={
      'fuzz.tsv':tsv(['case','input_sha256','prefix_size','level','v2','v3','preset_tier','preset_row','preset_strategy'],fuzz),
      'interop.tsv':tsv(['case','source_sha256','source_size','level','hint','overrides','historical_resolved_tuple','v2','v3','additive_c_required'],interop),
      'broader.tsv':tsv(['case','source_sha256','source_size','request','historical_resolved_tuple','v2','v3','additive_c_required'],broader),
      'loops.tsv':tsv(['family','source_size','level','v2','v3','preset_tier','preset_row','preset_strategy'],loops),
    }
    require({n:sha(b) for n,b in files.items()}==DATA_SHA,'frozen successor data')
    files['provenance.json']=encoded({'schema_version':3,'upstream_revision':generator.REVISION,'generator':str(Path(__file__).relative_to(ROOT)),
        'generator_sha256':sha(Path(__file__).read_bytes()),'input_sha256':PINS,'broader_capture_evidence_sha256':evidence_sha,'coverage':coverage,
        'newly_admitted_selectors':[r[0] for r in fuzz if r[4]!=r[5]],'historical92_additive_c_required':[r[0] for r in interop if r[9]],
        'broader159_additive_c_required':[r[0] for r in broader if r[7]],'changed_broader':[{'case':r[0],'before':r[5],'after':r[6]} for r in broader if r[5]!=r[6]],
        'fixtures':[{'path':n,'bytes':len(b),'sha256':sha(b)} for n,b in sorted(files.items())],
        'algorithm_caps':{'Fast':{'window':20,'hash':18},'DoubleFast':{'window':21,'hash':18,'chain':18},
                          'Greedy':{'window':21,'hash':19,'row_when_window_above':14,'chain_branch_chain':18,'chain_branch_search':8,'row_unused_chain_cap':None,'row_attempt_cap':64}},
        'qualification':'Source-only v3 expectations before public Greedy admission. Original81 selectors and43 numeric loop inputs are unchanged; C presets are pre-adjustment values. Both92/159 historical interop sets retain archived Rust-resolved scalar tuples, not newly observed C parameters. New positive C cases remain obligations. No codec, compiler, archived code or child process executes.'})
    return files


def verify(files,destination=DESTINATION):
    require(destination.is_dir() and not destination.is_symlink()
            and {p.name for p in destination.iterdir()}==set(files)|{'README.md'},'exact v3 fixture inventory')
    require((destination/'README.md').is_file() and not (destination/'README.md').is_symlink(),'regular fixture documentation')
    for name,data in files.items():
        p=destination/name;require(p.is_file() and not p.is_symlink() and p.read_bytes()==data,'immutable successor file: '+name)


def main():
    parser=argparse.ArgumentParser(description=__doc__);action=parser.add_mutually_exclusive_group(required=True)
    action.add_argument('--write',action='store_true');action.add_argument('--check',action='store_true');args=parser.parse_args()
    if args.write:require(not DESTINATION.exists() and not DESTINATION.is_symlink(),'fresh successor destination')
    files=projection()
    if args.write:
        DESTINATION.mkdir(parents=True)
        for name,data in files.items():
            with (DESTINATION/name).open('xb') as stream:stream.write(data)
    else:verify(files)
    print(json.dumps(json.loads(files['provenance.json'])['coverage'],sort_keys=True))


if __name__=='__main__':main()
