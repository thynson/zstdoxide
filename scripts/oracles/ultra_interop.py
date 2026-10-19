"""Source-only bounded Ultra public hypotheses over unchanged predecessors."""
from pathlib import Path
from dataclasses import replace
from functools import lru_cache
import collections,hashlib,json
ROOT=Path(__file__).resolve().parents[2]
REVISION='f8745da6ff1ad1e7bab384bd1f9d742439278e99'
MAX_SOURCE=4<<20
MAX_FRAME=5<<20
MAX_TOTAL_SOURCE=256<<20
PINS={'scripts/oracles/optimal_interop.py': 'd20ade78c9b541868ee2480ea4b5458c76c5872ed84dfda1212ddf99a22b330a', 'scripts/oracles/double_fast_interop.py': '05eefcf492ce7dca0741d3348d165aca8a48e402aa63710d1c043775125705f3', 'scripts/oracles/ultra_fresh_recipes.py': '0d76aa3e601bf8563ed374655cf9e22ca8d1cf15902242617a83de390b03ebec', 'scripts/check_reference_admission_v7.py': 'e7d00a7b657bd518cc41d81a5076f6d1f831c511862643e40e92252d4c6df93f', 'scripts/check_lazy_interop_capture_v2.py': '0cb1ea0cef617b732c02537dfda09f5d4ac3080d2f3e2dccab6cb2c399d8410f', 'tests/fixtures/reference-admission-v7/provenance.json': '547009d418dd4b8f50f416aba5193538600f9622cb2f8a6432f2be8c09f21243', 'docs/reference-ultra-interop-protocol.md': '78398829eea12eec5c8caf829b3123ccaec6888cd55e378d753d58c1621ef97b', 'tests/fixtures/reference-optimal-interop-recipes/recipes.json': '91f9c271ebb9cd42aeac7e3bd2a47be0841e610917d994bf589cb217996965ac', 'scripts/oracles/optimal_retained_wide_recipes.py': '1bcc4d2f5e3bc5b840e5c4639ecea939678f03d9315764967a75bcbc2fec37c6'}
def require(ok,why):
    if not ok:raise ValueError(why)
def sha(data):return hashlib.sha256(data).hexdigest()
def checked_inputs():
    for n,h in PINS.items():
        p=ROOT/n;require(p.is_file()and not p.is_symlink()and sha(p.read_bytes())==h,'pinned Ultra public input '+n)

@lru_cache(maxsize=1)
def prepared():
    checked_inputs()
    from oracles import optimal_interop as prior,double_fast_interop as common
    from oracles.ultra_fresh_recipes import recipes as fresh
    from oracles.optimal_retained_wide_recipes import noise
    from check_reference_admission_v7 import classify
    from check_lazy_interop_capture_v2 import presets,resolved_new_request
    table=presets();old=prior.recipes();prior_manifest=json.loads((ROOT/'tests/fixtures/reference-optimal-interop-recipes/recipes.json').read_bytes());tuples=prior_manifest['resolved_tuples']
    require([c.metadata()for c in old]==prior_manifest['recipes'],'exact complete predecessor source/request metadata')
    require(len(old)==len(tuples)==1269,'complete predecessor public inventory')
    result=[];resolved=[];origins=[];equal=[list(g) for g in prior_manifest['expected_rust_frame_equivalence']]
    for c,v in zip(old,tuples,strict=True):
        if v[6]==8:
            independent=resolved_new_request(c,table);require(v==independent,'independent old Ultra tuple');v=independent
        status=classify(v,len(c.source));require(c.expected!='success'or status=='success','preserve previous successes')
        result.append(replace(c,expected=status));resolved.append(v);origins.append('preserved predecessor tuple; Ultra independently reconstructed')
    def add(name,source,request,expected,why):
        c=common.Recipe(name,source,request,expected,why);v=resolved_new_request(c,table)
        require(classify(v,len(source))==expected,'independent new admission '+name)
        result.append(c);resolved.append(v);origins.append('independent pinned-preset reconstruction');return c,v
    seen=set()
    for name,recipe in fresh().items():
        if recipe.request.policy != 2:continue
        r=recipe.request.original;request=(13,r.window_log,r.chain_log,r.hash_log,r.search_log,r.min_match,r.target,8,0)
        key=(r.source,request)
        if key in seen:continue
        seen.add(key);add('ultra-observed-'+name,r.source,request,'success','Complete observed source; public automatic literal policy and known-size tuple resolution.')
    require(len(seen)==20,'distinct observed public source/request pairs')
    aliases=[]
    for size in [64,512,1024,2048]:
        for level in [13,14]:
            for pattern,source in [('uniform',b'a'*size),('periodic',(b'abcXabcYabcZabcd'*((size+14)//15))[:size]),('noise',noise(size))]:
                name=f'ultra-default-{level}-{size}-{pattern}'
                c,v=add(name,source,(level,0,0,0,0,0,0,0,0),'success','Default positive-level bounded Ultra request.')
                if pattern=='uniform':aliases.append((c,v))
    source=(b'abcXabcYabcZabcd'*1093)[:16384]
    for minimum in range(3,8):
        for target in [1,4095]:add(f'ultra-minimum-{minimum}-target-{target}',source,(13,14,6,6,1,minimum,target,8,0),'success','Minimum table geometry and sufficient/horizon target boundary.')
    for c,v in aliases:
        name=c.name+'-explicit';add(name,c.source,(1,*v,0),'success','Equivalent fully explicit resolved tuple with a different requested level.')
        equal.append([c.name,name])
    base=(13,14,12,12,6,4,4095,8,0)
    for size in [0,1,63,16385,32768]:add(f'ultra-resource-source-{size}',b'a'*size,base,'resource','Explicit complete-source resource boundary.')
    for name,index,value in [('window',1,10),('chain',2,13),('hash',3,13),('search',4,7),('target',6,4096)]:
        request=list(base);request[index]=value;add('ultra-resource-'+name,b'a'*16384,tuple(request),'resource','Resolved geometry/target resource boundary.')
    for strategy in [9]:
        request=list(base);request[7]=strategy;add(f'ultra-unsupported-{strategy}',b'a'*64,tuple(request),'unsupported','Unimplemented successor strategy remains distinct from resource limits.')
    for level,size in [(13,4096),(14,4096),(15,64)]:add(f'ultra-resource-default-{level}-{size}',b'a'*size,(level,0,0,0,0,0,0,0,0),'resource','Default resolved geometry exceeds private ordinary envelope.')
    require(len(result)==1345 and len(equal)==16 and len({c.name for c in result})==len(result),'complete unique recipe inventory')
    require(sum(len(c.source)for c in result)<=MAX_TOTAL_SOURCE,'aggregate source bound')
    require(all(len(c.source)<=MAX_SOURCE for c in result),'source cap')
    return tuple(result),resolved,origins,equal

def recipes():return prepared()[0]
def manifest():
    cases,tuples,origins,equal=prepared()
    return dict(schema_version=7,upstream_revision=REVISION,status='Source-only pre-execution public Ultra hypotheses.',source_limit=MAX_SOURCE,frame_limit=MAX_FRAME,aggregate_source_limit=MAX_TOTAL_SOURCE,source_bytes=sum(len(c.source)for c in cases),recipe_count=len(cases),classifications=dict(collections.Counter(c.expected for c in cases)),input_sha256=PINS,request_fields=['level','window','chain','hash','search','minimum','target','strategy','hint'],recipes=[c.metadata()for c in cases],resolved_tuples=tuples,resolution_origins=origins,expected_rust_frame_equivalence=equal,families=dict(historical=1269,observed_sources=20,defaults=24,minimum_target=10,equivalent_aliases=8,rejections=14),qualification='All original predecessor source/request bytes retained; every Ultra and every new tuple independently reconstructed. No native execution, public enabling, applied-C-scalar or C/Rust entropy identity claim.')
